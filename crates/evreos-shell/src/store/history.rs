//! History store stub.
//!
//! Under T040 this module declares the store type's signature with no behaviour,
//! allowing the profile store registry to compile. Full history implementation
//! lands in T041.

#![forbid(unsafe_code)]

use std::path::Path;

/// The history store.
#[derive(Debug, Default)]
pub struct HistoryStore;

impl HistoryStore {
    /// Open the history store under the profile root.
    pub fn open(_root: &Path) -> Self {
        Self
    }
}
