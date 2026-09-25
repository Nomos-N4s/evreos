//! Persistent download store.
//!
//! # Architecture and Invariants
//!
//! - **Local Residence (FR-004, FR-007a, Invariant A)**:
//!   Download records are strictly local. They are never transmitted, synchronised,
//!   or retained off the machine.
//! - **Destination Preservation (FR-004)**:
//!   Every download entry carries its destination path on disk.
//! - **Member Ownership (FR-004)**:
//!   Removing an entry from the download list deletes the record from the store
//!   and never deletes the file the member saved on disk.
//! - **Single Operation Persistence without Undo Logs (FR-004, Invariant A)**:
//!   State changes and removals are written atomically with no undo logs,
//!   journals, or secondary index files with independent lifetimes.

#![forbid(unsafe_code)]

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub use evreos_engine::DownloadId;

/// The lifecycle state of a download.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DownloadState {
    /// Transfer actively in progress.
    InProgress,
    /// Transfer successfully completed.
    Completed,
    /// Transfer cancelled by member action or browser policy.
    Cancelled,
    /// Transfer failed due to network, disk, or runtime error.
    Failed,
}

impl DownloadState {
    /// Parse a state identifier from its serialized string representation.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "in_progress" => Some(Self::InProgress),
            "completed" => Some(Self::Completed),
            "cancelled" => Some(Self::Cancelled),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }

    /// The string identifier for serialization.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }
}

impl fmt::Display for DownloadState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// An entry in the download store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadEntry {
    pub(crate) id: DownloadId,
    pub(crate) source_address: String,
    pub(crate) destination_path: PathBuf,
    pub(crate) bytes_total: Option<u64>,
    pub(crate) bytes_received: u64,
    pub(crate) state: DownloadState,
    pub(crate) started_at: SystemTime,
    pub(crate) finished_at: Option<SystemTime>,
}

impl DownloadEntry {
    /// Construct a new download entry in the `InProgress` state.
    pub fn new_in_progress(
        id: DownloadId,
        source_address: impl Into<String>,
        destination_path: impl Into<PathBuf>,
        bytes_total: Option<u64>,
        started_at: SystemTime,
    ) -> Self {
        Self {
            id,
            source_address: source_address.into(),
            destination_path: destination_path.into(),
            bytes_total,
            bytes_received: 0,
            state: DownloadState::InProgress,
            started_at,
            finished_at: None,
        }
    }

    /// The unique local identifier assigned to this download.
    pub fn id(&self) -> DownloadId {
        self.id
    }

    /// The origin address or URL from which the download was requested.
    pub fn source_address(&self) -> &str {
        &self.source_address
    }

    /// The absolute destination path on disk where the file is being or was written.
    pub fn destination_path(&self) -> &Path {
        &self.destination_path
    }

    /// The expected total size of the file in bytes, if known by the server.
    pub fn bytes_total(&self) -> Option<u64> {
        self.bytes_total
    }

    /// The number of bytes received so far.
    pub fn bytes_received(&self) -> u64 {
        self.bytes_received
    }

    /// Current lifecycle state of the download.
    pub fn state(&self) -> DownloadState {
        self.state
    }

    /// The timestamp when the download was started.
    pub fn started_at(&self) -> SystemTime {
        self.started_at
    }

    /// The timestamp when the download completed, was cancelled, or failed.
    pub fn finished_at(&self) -> Option<SystemTime> {
        self.finished_at
    }

    /// Whether the download is currently active.
    pub fn is_in_progress(&self) -> bool {
        self.state == DownloadState::InProgress
    }

    /// Whether the download finished successfully.
    pub fn is_completed(&self) -> bool {
        self.state == DownloadState::Completed
    }

    /// Whether the download was cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.state == DownloadState::Cancelled
    }

    /// Whether the download failed.
    pub fn is_failed(&self) -> bool {
        self.state == DownloadState::Failed
    }
}

/// Errors that may occur when interacting with the download store.
#[derive(Debug)]
pub enum DownloadError {
    /// The specified download identifier was not found in the store.
    NotFound(DownloadId),
    /// A download with the specified identifier already exists.
    AlreadyExists(DownloadId),
    /// The download has already completed, failed, or been cancelled.
    AlreadyFinished(DownloadId),
    /// An illegal lifecycle transition was attempted.
    InvalidStateTransition {
        /// The affected download identifier.
        id: DownloadId,
        /// Current state.
        current: DownloadState,
        /// Attempted invalid state.
        attempted: DownloadState,
    },
    /// An underlying filesystem I/O error occurred.
    Io(io::Error),
    /// The persisted file could not be parsed.
    InvalidFormat(String),
}

impl fmt::Display for DownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "download record not found: {id}"),
            Self::AlreadyExists(id) => write!(f, "download already exists: {id}"),
            Self::AlreadyFinished(id) => write!(f, "download has already finished: {id}"),
            Self::InvalidStateTransition {
                id,
                current,
                attempted,
            } => {
                write!(
                    f,
                    "invalid state transition for download {id}: cannot transition from {current} to {attempted}"
                )
            }
            Self::Io(err) => write!(f, "download store I/O error: {err}"),
            Self::InvalidFormat(msg) => write!(f, "invalid download store format: {msg}"),
        }
    }
}

impl std::error::Error for DownloadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for DownloadError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

/// The download store.
#[derive(Debug, Default)]
pub struct DownloadStore;

impl DownloadStore {
    /// Open the download store under the profile root.
    pub fn open(_root: &Path) -> Self {
        Self
    }
}
