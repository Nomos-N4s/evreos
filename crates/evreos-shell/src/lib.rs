//! The Evreos shell library.
//!
//! Exposes foundational shell modules including [`error`] and [`brand`].

#![forbid(unsafe_code)]

pub mod app;
pub mod brand;
pub mod error;
pub mod error_presentation;
pub mod handoff;
pub mod log;
pub mod omnibox;
pub mod permissions;
pub mod private;
pub mod profile;
pub mod search_provider;
pub mod session;
pub mod site_key;
pub mod store;
pub mod suggest;
pub mod tabs;
pub mod work;

pub use app::{App, AppWindow, AppWindowId, mint_window_id};
pub use error::{ErrorKind, LogProjection, MemberFacingError, ShellError};
pub use error_presentation::{
    ErrorPresentation, TimeoutPresentation, render_error, render_load_error, render_timeout,
    timeout_as_shell_error,
};
pub use handoff::{
    HandOffBrowser, HandOffError, HandOffExecutor, HandOffOffer, HandOffReason,
    MockHandOffExecutor, detect_password_input,
};
pub use log::{
    Address, Credential, EventKind, Field, FieldValue, Level, LogSink, LogValue, MemoryLogSink,
    PageTitle, Record, RecordBuilder, SearchTerm, Sensitive, Token, emit, log_directory,
    log_file_path,
};
pub use omnibox::{Omnibox, OmniboxAction, SubmittedSearch};
pub use permissions::{
    Capability, PermissionDecision, PermissionError, PermissionPromptRequest, PermissionStore,
    PromptOutcome, PromptResponse, PromptResult, SitePermission, WindowScope,
};
pub use private::{PrivateSession, PrivateWindow, record_history_safely};
pub use profile::{Profile, ProfileError, SearchProviderSetting, ThemePreference};
pub use session::{
    SessionError, SessionSnapshot, SessionStore, SessionTab, SessionWindow, parse_session_file,
    serialize_session,
};
pub use site_key::{SiteKey, SiteKeyError};
pub use store::{
    Bookmark, BookmarkError, BookmarkFolder, BookmarkId, BookmarkSource, BookmarkStore,
    DownloadEntry, DownloadError, DownloadId, DownloadState, DownloadStore, FolderId, HistoryEntry,
    HistoryEntryId, HistoryError, HistorySource, HistoryStore, StoreRegistry, WindowKind,
};
pub use suggest::{OpenTab, Suggestion, SuggestionIndex, SuggestionSource};
pub use tabs::{
    Clock, DEFAULT_NAVIGATION_TIMEOUT, MockClock, NavigationState, NavigationTracker, SystemClock,
    Tab, TabError, TabId, TabLifecycle, WindowTabs, mint_tab_id,
};
pub use work::{
    DEFAULT_MAX_QUEUE_CAPACITY, DEFAULT_WORKER_THREADS, JobId, JobOutcome, JobResult, PoolError,
    WorkerPool,
};
