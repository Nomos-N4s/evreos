//! Download store stub.
//!
//! Under T040 this module declares the store type's signature with no behaviour,
//! allowing the profile store registry to compile. Full download implementation
//! lands in T043.

#![forbid(unsafe_code)]

use std::path::Path;

/// The download store.
#[derive(Debug, Default)]
pub struct DownloadStore;

impl DownloadStore {
    /// Open the download store under the profile root.
    pub fn open(_root: &Path) -> Self {
        Self
    }
}
