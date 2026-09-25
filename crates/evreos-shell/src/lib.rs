//! The Evreos shell library.
//!
//! Exposes foundational shell modules including [`error`] and [`brand`].

#![forbid(unsafe_code)]

pub mod app;
pub mod brand;
pub mod error;
pub mod log;
pub mod profile;
pub mod store;
pub mod work;

pub use app::{App, AppWindow, AppWindowId};
pub use error::{ErrorKind, LogProjection, MemberFacingError, ShellError};
pub use log::{
    Address, Credential, EventKind, Field, FieldValue, Level, LogSink, LogValue, MemoryLogSink,
    PageTitle, Record, RecordBuilder, SearchTerm, Sensitive, Token, emit, log_directory,
    log_file_path,
};
pub use profile::{HandOffBrowser, Profile, ProfileError, SearchProviderSetting, ThemePreference};
pub use store::{
    BookmarkStore, DownloadStore, HistoryEntry, HistoryEntryId, HistoryError, HistorySource,
    HistoryStore, StoreRegistry, WindowKind,
};
pub use work::{
    DEFAULT_MAX_QUEUE_CAPACITY, DEFAULT_WORKER_THREADS, JobId, JobOutcome, JobResult, PoolError,
    WorkerPool,
};
