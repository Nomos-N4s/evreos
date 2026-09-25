//! Tests for the shell worker pool in crates/evreos-shell/src/work.rs.
//!
//! Under T039:
//! - Asserts that a submitted job runs off the UI thread.
//! - Asserts that its result is delivered on the UI thread.
//! - Asserts that the pool refuses work rather than growing without bound when saturated.
//! - Asserts that a panicking job neither poisons the pool nor takes the process down.

use std::panic;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use evreos_shell::work::{PoolError, WorkerPool};

#[test]
fn job_runs_off_ui_thread_and_result_delivered_on_ui_thread() {
    let mut pool = WorkerPool::<thread::ThreadId>::new(2, 8);
    let ui_thread = pool.ui_thread_id();
    assert_eq!(ui_thread, thread::current().id());

    let job_id = pool
        .submit(|| {
            // Executing on a worker thread
            thread::current().id()
        })
        .expect("submission must succeed");

    // Wait until result is available
    let mut completed = None;
    for _ in 0..100 {
        let results = pool.drain_results();
        if let Some(res) = results.into_iter().find(|r| r.id() == job_id) {
            completed = Some(res);
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }

    let result = completed.expect("job must complete and be delivered");

    assert!(result.is_success());
    assert!(!result.is_panicked());

    let worker_thread = *result.value().expect("must carry computed thread id");
    // Assert job ran off the UI thread
    assert_ne!(
        worker_thread, ui_thread,
        "submitted job must run off the UI thread"
    );
    assert_eq!(
        result.worker_thread_id(),
        worker_thread,
        "worker_thread_id must match actual execution thread"
    );

    // Assert result is delivered on the UI thread
    assert_eq!(
        result.delivery_thread_id(),
        ui_thread,
        "result must be delivered on the UI thread"
    );
}

#[test]
fn draining_results_asserts_caller_is_on_ui_thread() {
    let mut pool = WorkerPool::<()>::new(1, 4);

    let fake_other_thread = thread::spawn(|| thread::current().id()).join().unwrap();
    pool.set_ui_thread_for_testing(fake_other_thread);

    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        pool.drain_results();
    }));

    assert!(
        result.is_err(),
        "draining results off the UI thread must fail thread assertion"
    );
}

#[test]
fn pool_refuses_work_when_saturated_rather_than_growing_without_bound() {
    // 1 worker thread, queue capacity 2
    let (block_tx, block_rx) = mpsc::channel::<()>();
    let (started_tx, started_rx) = mpsc::channel::<()>();

    let mut pool = WorkerPool::<()>::new(1, 2);
    assert_eq!(pool.queue_bound(), 2);
    assert_eq!(pool.worker_count(), 1);

    // 1. Submit a job that blocks, occupying the single worker thread
    let _job1 = pool
        .submit(move || {
            let _ = started_tx.send(());
            let _ = block_rx.recv();
        })
        .expect("job 1 must be accepted");

    // Wait until worker starts executing job 1
    started_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("worker must pick up job 1");

    // 2. Submit job 2 (fills queue slot 1 of 2)
    let _job2 = pool.submit(|| ()).expect("job 2 must fit in bounded queue");

    // 3. Submit job 3 (fills queue slot 2 of 2)
    let _job3 = pool.submit(|| ()).expect("job 3 must fit in bounded queue");

    // 4. Now the queue is completely saturated (capacity 2 full)
    // Submitting job 4 must be REFUSED immediately with PoolError::Saturated
    let overflow = pool.submit(|| ());
    assert_eq!(
        overflow,
        Err(PoolError::Saturated),
        "pool must refuse work when saturated rather than growing without bound"
    );

    // Unblock the worker so all queued jobs complete
    let _ = block_tx.send(());

    // Wait for queued jobs to finish
    let mut finished_count = 0;
    for _ in 0..100 {
        finished_count += pool.drain_results().len();
        if finished_count >= 3 {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(finished_count, 3);

    // With queue drained, new work is accepted again
    let new_job = pool.submit(|| ());
    assert!(
        new_job.is_ok(),
        "pool must accept work again once queue is no longer saturated"
    );
}

#[test]
fn panicking_job_neither_poisons_pool_nor_takes_process_down() {
    let mut pool = WorkerPool::<u32>::new(2, 4);

    let ran_after_panic = Arc::new(AtomicBool::new(false));
    let ran_after_panic_clone = Arc::clone(&ran_after_panic);

    // Submit a job that explicitly panics
    let panic_id = pool
        .submit(|| {
            panic!("deliberate panic in worker pool test");
        })
        .expect("panicking job must be accepted");

    // Submit a subsequent job immediately
    let normal_id = pool
        .submit(move || {
            ran_after_panic_clone.store(true, Ordering::SeqCst);
            100_u32
        })
        .expect("normal job submitted after panicking job must be accepted");

    // Drain results until both jobs report back
    let mut panic_res = None;
    let mut normal_res = None;

    for _ in 0..100 {
        for res in pool.drain_results() {
            if res.id() == panic_id {
                panic_res = Some(res);
            } else if res.id() == normal_id {
                normal_res = Some(res);
            }
        }
        if panic_res.is_some() && normal_res.is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }

    let panic_res = panic_res.expect("panic job result must be returned");
    assert!(
        panic_res.is_panicked(),
        "job that panicked must report is_panicked"
    );
    assert!(!panic_res.is_success());
    assert_eq!(panic_res.value(), None);

    let normal_res = normal_res.expect("normal job result must be returned");
    assert!(
        normal_res.is_success(),
        "normal job must succeed despite prior panic"
    );
    assert_eq!(normal_res.value(), Some(&100));
    assert!(ran_after_panic.load(Ordering::SeqCst));

    // Submit a third job to prove the pool remains fully functional and unpoisoned
    let third_id = pool
        .submit(|| 200_u32)
        .expect("pool must accept new work after catching panic");

    let mut third_res = None;
    for _ in 0..100 {
        for res in pool.drain_results() {
            if res.id() == third_id {
                third_res = Some(res);
                break;
            }
        }
        if third_res.is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }

    let third_res = third_res.expect("third job must complete cleanly");
    assert!(third_res.is_success());
    assert_eq!(third_res.value(), Some(&200));
}
