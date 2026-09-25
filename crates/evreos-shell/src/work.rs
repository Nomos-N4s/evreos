//! Shell worker pool for off-UI latency-sensitive tasks.
//!
//! # Architectural Invariants and Latency Bounds
//!
//! Under **SC-006**:
//! - Address-field keystroke and tab-switch input-to-repaint latency must not
//!   exceed **16 ms at the 99th percentile (p99)**.
//! - Nothing the member waits on may execute on the main UI thread. Long-running,
//!   I/O-heavy, or CPU-intensive computations (such as the omnibox suggestion index
//!   under T045, bookmark indexing, and history filtering) must run on worker threads.
//!
//! Under **Thread Affinity**:
//! - Operating-system webviews (WebView2 on Windows, WKWebView delegates on macOS)
//!   and windowing surfaces (`winit`) are strictly thread-affine.
//! - Background workers MUST NOT touch engine state, windows, or DOM elements directly.
//! - Work runs off the UI thread, and all completed results return to the UI thread
//!   that owns the window before touching engine state.
//!
//! Under **Bounded Queue & Saturation Policy**:
//! - An unbounded queue allows runaway backlogs during fast keystroke bursts, causing
//!   stale suggestions and breaching the 16 ms p99 latency cap.
//! - The worker pool enforces a strict, bounded queue capacity ([`DEFAULT_MAX_QUEUE_CAPACITY`]).
//! - When saturated, [`WorkerPool::submit`] immediately refuses work by returning
//!   [`PoolError::Saturated`] rather than growing memory without bound or blocking the UI thread.
//!
//! Under **Panic Safety**:
//! - A panicking job within the worker pool MUST NOT poison the shared queue mutex
//!   nor crash the host process.
//! - Worker threads catch panics, record [`JobOutcome::Panicked`], and continue serving
//!   subsequent requests unimpeded.

#![forbid(unsafe_code)]

use std::fmt;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle, ThreadId};

/// Default number of worker threads allocated for off-UI latency-sensitive tasks.
///
/// Bounded to 4 threads so background indexing and suggestions do not contend
/// with OS webview render processes for system CPU cores.
pub const DEFAULT_WORKER_THREADS: usize = 4;

/// Bound on queued jobs waiting for an available worker thread.
///
/// Under SC-006, address-field keystroke response is capped at 16 ms at the 99th percentile.
/// When the queue fills to this bound, subsequent submissions are refused with
/// [`PoolError::Saturated`] rather than growing without bound or blocking the UI thread.
pub const DEFAULT_MAX_QUEUE_CAPACITY: usize = 32;

/// Unique identifier for a submitted job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JobId(u64);

impl JobId {
    pub const FIRST: JobId = JobId(1);

    pub fn next(self) -> JobId {
        JobId(self.0 + 1)
    }
}

impl fmt::Display for JobId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "job-{}", self.0)
    }
}

static NEXT_JOB_ID: AtomicU64 = AtomicU64::new(1);

fn mint_job_id() -> JobId {
    JobId(NEXT_JOB_ID.fetch_add(1, Ordering::Relaxed))
}

/// Errors returned by the worker pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolError {
    /// The pool is saturated: the bounded queue is full.
    ///
    /// The pool refuses work rather than growing without bound to protect SC-006's
    /// 16 ms keystroke latency cap.
    Saturated,
    /// The worker pool has shut down and is no longer accepting work.
    Terminated,
}

impl fmt::Display for PoolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Saturated => write!(f, "worker pool is saturated: queue capacity reached"),
            Self::Terminated => write!(f, "worker pool has terminated"),
        }
    }
}

impl std::error::Error for PoolError {}

/// The execution outcome of a submitted job.
#[derive(Debug)]
pub enum JobOutcome<T> {
    /// The job completed successfully, returning its computed value.
    Success(T),
    /// The job panicked during execution. The panic was caught and contained.
    Panicked,
}

/// A completed job result delivered back to the UI thread.
#[derive(Debug)]
pub struct JobResult<T> {
    id: JobId,
    outcome: JobOutcome<T>,
    worker_thread: ThreadId,
    delivery_thread: ThreadId,
}

impl<T> JobResult<T> {
    /// The correlation ID of the job.
    pub fn id(&self) -> JobId {
        self.id
    }

    /// Whether the job executed to completion without panicking.
    pub fn is_success(&self) -> bool {
        matches!(self.outcome, JobOutcome::Success(_))
    }

    /// Whether the job panicked.
    pub fn is_panicked(&self) -> bool {
        matches!(self.outcome, JobOutcome::Panicked)
    }

    /// Borrow the computed value if the job succeeded.
    pub fn value(&self) -> Option<&T> {
        match &self.outcome {
            JobOutcome::Success(val) => Some(val),
            JobOutcome::Panicked => None,
        }
    }

    /// Consume the result, returning the computed value if the job succeeded.
    pub fn into_value(self) -> Option<T> {
        match self.outcome {
            JobOutcome::Success(val) => Some(val),
            JobOutcome::Panicked => None,
        }
    }

    /// Reference to the job outcome.
    pub fn outcome(&self) -> &JobOutcome<T> {
        &self.outcome
    }

    /// The ID of the worker thread that executed the job.
    pub fn worker_thread_id(&self) -> ThreadId {
        self.worker_thread
    }

    /// The ID of the UI thread where the result was delivered.
    pub fn delivery_thread_id(&self) -> ThreadId {
        self.delivery_thread
    }
}

struct RawResult<T> {
    id: JobId,
    outcome: JobOutcome<T>,
    worker_thread: ThreadId,
}

struct Job<T> {
    id: JobId,
    task: Box<dyn FnOnce() -> T + Send + 'static>,
}

/// A bounded worker pool running latency-sensitive tasks off the UI thread
/// and delivering results back onto it.
pub struct WorkerPool<T: Send + 'static> {
    ui_thread: ThreadId,
    job_tx: Option<SyncSender<Job<T>>>,
    result_rx: Receiver<RawResult<T>>,
    workers: Vec<JoinHandle<()>>,
    capacity: usize,
    num_threads: usize,
}

impl<T: Send + 'static> WorkerPool<T> {
    /// Create a new worker pool with default thread count and queue bound.
    pub fn default_bounded() -> Self {
        Self::new(DEFAULT_WORKER_THREADS, DEFAULT_MAX_QUEUE_CAPACITY)
    }

    /// Create a new worker pool with the specified thread count and queue bound.
    ///
    /// # Panics
    ///
    /// Panics if `threads` or `queue_capacity` is zero.
    pub fn new(threads: usize, queue_capacity: usize) -> Self {
        assert!(threads > 0, "threads must be greater than zero");
        assert!(
            queue_capacity > 0,
            "queue_capacity must be greater than zero"
        );

        let ui_thread = thread::current().id();
        let (job_tx, job_rx) = mpsc::sync_channel::<Job<T>>(queue_capacity);
        let (result_tx, result_rx) = mpsc::channel::<RawResult<T>>();

        let job_rx = Arc::new(Mutex::new(job_rx));
        let mut workers = Vec::with_capacity(threads);

        for _ in 0..threads {
            let rx = Arc::clone(&job_rx);
            let tx = result_tx.clone();

            let handle = thread::spawn(move || {
                let worker_thread = thread::current().id();
                loop {
                    // Receive next job; exit if all senders dropped
                    let job = {
                        let lock = match rx.lock() {
                            Ok(guard) => guard,
                            Err(poisoned) => poisoned.into_inner(),
                        };
                        match lock.recv() {
                            Ok(job) => job,
                            Err(_) => break, // Channel disconnected; pool shutting down
                        }
                    };

                    // Execute job outside the lock with panic safety
                    let outcome = match panic::catch_unwind(AssertUnwindSafe(job.task)) {
                        Ok(val) => JobOutcome::Success(val),
                        Err(_) => JobOutcome::Panicked,
                    };

                    // Deliver raw result back
                    if tx
                        .send(RawResult {
                            id: job.id,
                            outcome,
                            worker_thread,
                        })
                        .is_err()
                    {
                        break; // Result receiver dropped
                    }
                }
            });

            workers.push(handle);
        }

        Self {
            ui_thread,
            job_tx: Some(job_tx),
            result_rx,
            workers,
            capacity: queue_capacity,
            num_threads: threads,
        }
    }

    /// The thread ID required for result delivery.
    pub fn ui_thread_id(&self) -> ThreadId {
        self.ui_thread
    }

    /// Asserts that the caller is executing on the designated UI thread.
    ///
    /// # Panics
    ///
    /// Panics if called from any thread other than `ui_thread`.
    pub fn assert_ui_thread(&self) {
        assert_eq!(
            thread::current().id(),
            self.ui_thread,
            "UI call off the event-loop thread"
        );
    }

    /// Override the designated UI thread ID for testing thread-affinity assertions.
    pub fn set_ui_thread_for_testing(&mut self, thread_id: ThreadId) {
        self.ui_thread = thread_id;
    }

    /// Maximum number of queued jobs before saturation rejection occurs.
    pub fn queue_bound(&self) -> usize {
        self.capacity
    }

    /// Number of worker threads in the pool.
    pub fn worker_count(&self) -> usize {
        self.num_threads
    }

    /// Submit a job to run off the UI thread on an available worker.
    ///
    /// Returns [`PoolError::Saturated`] immediately if the queue capacity is reached,
    /// upholding SC-006's 16 ms keystroke latency guarantee by refusing work rather
    /// than growing an unbounded backlog.
    pub fn submit<F>(&self, task: F) -> Result<JobId, PoolError>
    where
        F: FnOnce() -> T + Send + 'static,
    {
        let Some(ref tx) = self.job_tx else {
            return Err(PoolError::Terminated);
        };

        let id = mint_job_id();
        let job = Job {
            id,
            task: Box::new(task),
        };

        match tx.try_send(job) {
            Ok(()) => Ok(id),
            Err(TrySendError::Full(_)) => Err(PoolError::Saturated),
            Err(TrySendError::Disconnected(_)) => Err(PoolError::Terminated),
        }
    }

    /// Drain all completed job results onto the UI thread.
    ///
    /// Must be called from the platform UI thread.
    ///
    /// # Panics
    ///
    /// Panics if called off the UI thread per ADR-0002 thread affinity.
    pub fn drain_results(&mut self) -> Vec<JobResult<T>> {
        self.assert_ui_thread();
        let delivery_thread = thread::current().id();
        let mut results = Vec::new();

        while let Ok(raw) = self.result_rx.try_recv() {
            results.push(JobResult {
                id: raw.id,
                outcome: raw.outcome,
                worker_thread: raw.worker_thread,
                delivery_thread,
            });
        }

        results
    }
}

impl<T: Send + 'static> Drop for WorkerPool<T> {
    fn drop(&mut self) {
        // Drop the sender to signal worker threads to exit
        drop(self.job_tx.take());

        // Join worker threads to ensure clean shutdown
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}
