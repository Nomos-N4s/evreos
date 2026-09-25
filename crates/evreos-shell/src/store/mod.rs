//! The profile store registry.
//!
//! Under T040, this module declares the `history`, `bookmarks`, and `downloads`
//! store modules and nothing else.
//!
//! Session state, site permissions, and site blocking exceptions are NOT store
//! modules and are not declared here: they land separately at `session.rs`,
//! `permissions.rs`, and `blocking_control.rs`.

#![forbid(unsafe_code)]

pub mod bookmarks;
pub mod downloads;
pub mod history;

pub use bookmarks::BookmarkStore;
pub use downloads::DownloadStore;
pub use history::{
    HistoryEntry, HistoryEntryId, HistoryError, HistorySource, HistoryStore, WindowKind,
};

use std::path::{Path, PathBuf};

/// The registry of stores held under a profile root directory.
#[derive(Debug)]
pub struct StoreRegistry {
    root_path: PathBuf,
    history: HistoryStore,
    bookmarks: BookmarkStore,
    downloads: DownloadStore,
}

impl StoreRegistry {
    /// Open or initialise the store registry under `root_path`.
    pub fn open(root_path: impl Into<PathBuf>) -> Self {
        let root = root_path.into();
        Self {
            history: HistoryStore::open(&root),
            bookmarks: BookmarkStore::open(&root),
            downloads: DownloadStore::open(&root),
            root_path: root,
        }
    }

    /// The root filesystem path of the profile this registry belongs to.
    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    /// Reference to the history store.
    pub fn history(&self) -> &HistoryStore {
        &self.history
    }

    /// Mutable reference to the history store.
    pub fn history_mut(&mut self) -> &mut HistoryStore {
        &mut self.history
    }

    /// Reference to the bookmark store.
    pub fn bookmarks(&self) -> &BookmarkStore {
        &self.bookmarks
    }

    /// Mutable reference to the bookmark store.
    pub fn bookmarks_mut(&mut self) -> &mut BookmarkStore {
        &mut self.bookmarks
    }

    /// Reference to the download store.
    pub fn downloads(&self) -> &DownloadStore {
        &self.downloads
    }

    /// Mutable reference to the download store.
    pub fn downloads_mut(&mut self) -> &mut DownloadStore {
        &mut self.downloads
    }
}
