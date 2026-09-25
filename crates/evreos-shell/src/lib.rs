//! The Evreos shell library.
//!
//! Exposes foundational shell modules including [`error`] and [`brand`].

#![forbid(unsafe_code)]

pub mod brand;
pub mod error;
pub mod log;

pub use error::{ErrorKind, LogProjection, MemberFacingError, ShellError};
pub use log::{
    Address, Credential, EventKind, Field, FieldValue, Level, LogSink, LogValue, MemoryLogSink,
    PageTitle, Record, RecordBuilder, SearchTerm, Sensitive, Token, emit, log_directory,
    log_file_path,
};
