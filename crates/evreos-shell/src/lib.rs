//! The Evreos shell library.
//!
//! Exposes foundational shell modules including [`error`] and [`brand`].

#![forbid(unsafe_code)]

pub mod brand;
pub mod error;

pub use error::{ErrorKind, LogProjection, MemberFacingError, ShellError};
