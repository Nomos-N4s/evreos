//! Bookmark store stub.
//!
//! Under T040 this module declares the store type's signature with no behaviour,
//! allowing the profile store registry to compile. Full bookmark implementation
//! lands in T042.

#![forbid(unsafe_code)]

use std::path::Path;

/// The bookmark store.
#[derive(Debug, Default)]
pub struct BookmarkStore;

impl BookmarkStore {
    /// Open the bookmark store under the profile root.
    pub fn open(_root: &Path) -> Self {
        Self
    }
}
