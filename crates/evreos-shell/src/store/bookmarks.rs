//! Local bookmark and folder store.
//!
//! # Architecture and Invariants
//!
//! - **Local Residence (FR-004, FR-007a, Invariant A)**:
//!   Bookmarks are strictly local to the device. They are never transmitted,
//!   synchronised, or retained off the machine in whole or derived form.
//! - **Tree Invariant (data-model §1.8)**:
//!   The folder graph is strictly a tree: exactly one root folder, no cycles,
//!   and every bookmark is reachable from the root.
//! - **Cascade Deletion without Undo or Journals (FR-004, Invariant A)**:
//!   Deleting a bookmark or folder erases the record and all child items in the
//!   same operation. No undo log or journal with an independent lifetime is
//!   maintained.

#![forbid(unsafe_code)]

use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static TMP_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Unique identifier for a bookmark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BookmarkId(pub u64);

impl BookmarkId {
    /// Create a new bookmark identifier.
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// The raw 64-bit integer representation.
    pub const fn as_u64(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for BookmarkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for BookmarkId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

/// Unique identifier for a bookmark folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FolderId(pub u64);

impl FolderId {
    /// The root folder identifier that anchors the bookmark tree.
    pub const ROOT: Self = Self(0);

    /// Create a new folder identifier.
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// The raw 64-bit integer representation.
    pub const fn as_u64(&self) -> u64 {
        self.0
    }

    /// Whether this is the root folder.
    pub const fn is_root(&self) -> bool {
        self.0 == 0
    }
}

impl fmt::Display for FolderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for FolderId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

/// The origin or provenance of a bookmark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BookmarkSource {
    /// Created directly by the member.
    Created,
    /// Imported from an external browser profile (FR-012).
    Imported {
        /// Identifier or name of the source browser application.
        browser: String,
    },
}

impl BookmarkSource {
    /// Construct an imported bookmark source.
    pub fn imported(browser: impl Into<String>) -> Self {
        Self::Imported {
            browser: browser.into(),
        }
    }
}

/// A single bookmark record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bookmark {
    /// Unique local bookmark identifier.
    pub bookmark_id: BookmarkId,
    /// Reference to the parent folder containing this bookmark.
    pub parent_folder: FolderId,
    /// Display title for the bookmark.
    pub title: String,
    /// The bookmarked address / URL.
    pub address: String,
    /// Timestamp when this bookmark was created.
    pub created_at: SystemTime,
    /// Origin of this bookmark.
    pub source: BookmarkSource,
    /// Position within its parent folder.
    pub position: u32,
}

impl Bookmark {
    /// Construct a new bookmark record.
    pub fn new(
        bookmark_id: BookmarkId,
        parent_folder: FolderId,
        title: impl Into<String>,
        address: impl Into<String>,
        created_at: SystemTime,
        source: BookmarkSource,
        position: u32,
    ) -> Self {
        Self {
            bookmark_id,
            parent_folder,
            title: title.into(),
            address: address.into(),
            created_at,
            source,
            position,
        }
    }

    /// The bookmark's unique identifier.
    pub fn id(&self) -> BookmarkId {
        self.bookmark_id
    }

    /// The folder holding this bookmark.
    pub fn parent(&self) -> FolderId {
        self.parent_folder
    }

    /// The bookmark's title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The bookmarked address.
    pub fn address(&self) -> &str {
        &self.address
    }

    /// When the bookmark was created.
    pub fn created_at(&self) -> SystemTime {
        self.created_at
    }

    /// Provenance of the bookmark.
    pub fn source(&self) -> &BookmarkSource {
        &self.source
    }

    /// Display order position within its parent folder.
    pub fn position(&self) -> u32 {
        self.position
    }
}

/// A bookmark folder node in the folder tree hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookmarkFolder {
    /// Unique folder identifier.
    pub folder_id: FolderId,
    /// Parent folder reference, or `None` if this is the root folder.
    pub parent_folder: Option<FolderId>,
    /// Human-readable folder name.
    pub name: String,
    /// Ordering position among siblings in the parent folder.
    pub position: u32,
}

impl BookmarkFolder {
    /// Create the root folder node.
    pub fn root() -> Self {
        Self {
            folder_id: FolderId::ROOT,
            parent_folder: None,
            name: "Bookmarks".to_string(),
            position: 0,
        }
    }

    /// Construct a new folder with an explicit parent.
    pub fn new(
        folder_id: FolderId,
        parent_folder: FolderId,
        name: impl Into<String>,
        position: u32,
    ) -> Self {
        Self {
            folder_id,
            parent_folder: Some(parent_folder),
            name: name.into(),
            position,
        }
    }

    /// The unique folder identifier.
    pub fn id(&self) -> FolderId {
        self.folder_id
    }

    /// The parent folder ID, or `None` if this is the root folder.
    pub fn parent(&self) -> Option<FolderId> {
        self.parent_folder
    }

    /// The folder's display name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Ordering position among siblings.
    pub fn position(&self) -> u32 {
        self.position
    }

    /// Whether this is the root folder.
    pub fn is_root(&self) -> bool {
        self.folder_id.is_root()
    }
}

/// Errors occurring during bookmark store operations or validation.
#[derive(Debug)]
pub enum BookmarkError {
    /// Filesystem I/O failure.
    Io(io::Error),
    /// Format or parsing error in the bookmark persistence file.
    InvalidFormat(String),
    /// Referenced bookmark was not found.
    BookmarkNotFound(BookmarkId),
    /// Referenced folder was not found.
    FolderNotFound(FolderId),
    /// Moving a folder would create a cycle in the folder tree.
    CycleDetected(FolderId),
    /// Root folder cannot be deleted or assigned a parent.
    CannotModifyRoot,
    /// Tree invariant violation.
    TreeInvariantViolation(String),
}

impl fmt::Display for BookmarkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "bookmark I/O error: {err}"),
            Self::InvalidFormat(msg) => write!(f, "invalid bookmark file format: {msg}"),
            Self::BookmarkNotFound(id) => write!(f, "bookmark {id} not found"),
            Self::FolderNotFound(id) => write!(f, "bookmark folder {id} not found"),
            Self::CycleDetected(id) => {
                write!(
                    f,
                    "cycle detected: folder {id} cannot be moved into its own subtree"
                )
            }
            Self::CannotModifyRoot => {
                write!(f, "root bookmark folder cannot be deleted or reparented")
            }
            Self::TreeInvariantViolation(msg) => {
                write!(f, "bookmark tree invariant violation: {msg}")
            }
        }
    }
}

impl std::error::Error for BookmarkError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for BookmarkError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

/// The persistent bookmark and folder store.
#[derive(Debug, Clone)]
pub struct BookmarkStore {
    root: PathBuf,
    folders: Vec<BookmarkFolder>,
    bookmarks: Vec<Bookmark>,
    next_folder_id: u64,
    next_bookmark_id: u64,
}

impl Default for BookmarkStore {
    fn default() -> Self {
        Self {
            root: PathBuf::new(),
            folders: vec![BookmarkFolder::root()],
            bookmarks: Vec::new(),
            next_folder_id: 1,
            next_bookmark_id: 1,
        }
    }
}

impl BookmarkStore {
    const FILE_NAME: &'static str = "bookmarks.toml";

    /// Open or initialise the bookmark store under the given profile root.
    pub fn open(root: &Path) -> Self {
        Self::try_open(root).unwrap_or_else(|_| Self::new_empty(root))
    }

    /// Attempt to open the bookmark store, returning any parsing or I/O error.
    pub fn try_open(root: &Path) -> Result<Self, BookmarkError> {
        let file_path = root.join(Self::FILE_NAME);
        if !file_path.exists() {
            return Ok(Self::new_empty(root));
        }

        let content = fs::read_to_string(&file_path)?;
        let (folders, bookmarks) = Self::deserialize(&content)?;

        let store = Self {
            root: root.to_path_buf(),
            next_folder_id: folders
                .iter()
                .map(|f| f.folder_id.as_u64())
                .max()
                .unwrap_or(0)
                + 1,
            next_bookmark_id: bookmarks
                .iter()
                .map(|b| b.bookmark_id.as_u64())
                .max()
                .unwrap_or(0)
                + 1,
            folders,
            bookmarks,
        };

        store.validate_tree()?;
        Ok(store)
    }

    fn new_empty(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            folders: vec![BookmarkFolder::root()],
            bookmarks: Vec::new(),
            next_folder_id: 1,
            next_bookmark_id: 1,
        }
    }

    /// The path to `bookmarks.toml` under the profile root.
    pub fn file_path(&self) -> PathBuf {
        self.root.join(Self::FILE_NAME)
    }

    /// Access all folders in the store.
    pub fn folders(&self) -> &[BookmarkFolder] {
        &self.folders
    }

    /// Access all bookmarks in the store.
    pub fn bookmarks(&self) -> &[Bookmark] {
        &self.bookmarks
    }

    /// Reference to the root bookmark folder.
    pub fn root_folder(&self) -> &BookmarkFolder {
        self.folders
            .iter()
            .find(|f| f.folder_id.is_root())
            .expect("root folder must always exist")
    }

    /// Look up a folder by ID.
    pub fn get_folder(&self, id: FolderId) -> Option<&BookmarkFolder> {
        self.folders.iter().find(|f| f.folder_id == id)
    }

    /// Look up a bookmark by ID.
    pub fn get_bookmark(&self, id: BookmarkId) -> Option<&Bookmark> {
        self.bookmarks.iter().find(|b| b.bookmark_id == id)
    }

    /// List immediate child folders of `parent`.
    pub fn subfolders(&self, parent: FolderId) -> Vec<&BookmarkFolder> {
        let mut list: Vec<&BookmarkFolder> = self
            .folders
            .iter()
            .filter(|f| f.parent_folder == Some(parent))
            .collect();
        list.sort_by_key(|f| f.position);
        list
    }

    /// List bookmarks directly contained in `parent`.
    pub fn bookmarks_in_folder(&self, parent: FolderId) -> Vec<&Bookmark> {
        let mut list: Vec<&Bookmark> = self
            .bookmarks
            .iter()
            .filter(|b| b.parent_folder == parent)
            .collect();
        list.sort_by_key(|b| b.position);
        list
    }

    /// Create a new folder under `parent`.
    pub fn create_folder(
        &mut self,
        parent: FolderId,
        name: impl Into<String>,
    ) -> Result<FolderId, BookmarkError> {
        if !self.folders.iter().any(|f| f.folder_id == parent) {
            return Err(BookmarkError::FolderNotFound(parent));
        }

        let folder_id = FolderId::new(self.next_folder_id);
        self.next_folder_id += 1;

        let position = self.subfolders(parent).len() as u32;
        let folder = BookmarkFolder::new(folder_id, parent, name, position);
        self.folders.push(folder);

        self.save_to_disk()?;
        Ok(folder_id)
    }

    /// Create a new bookmark under `parent`.
    pub fn create_bookmark(
        &mut self,
        parent: FolderId,
        title: impl Into<String>,
        address: impl Into<String>,
    ) -> Result<BookmarkId, BookmarkError> {
        self.create_bookmark_with_details(
            parent,
            title,
            address,
            SystemTime::now(),
            BookmarkSource::Created,
        )
    }

    /// Create a bookmark with explicit metadata (for imports or deterministic testing).
    pub fn create_bookmark_with_details(
        &mut self,
        parent: FolderId,
        title: impl Into<String>,
        address: impl Into<String>,
        created_at: SystemTime,
        source: BookmarkSource,
    ) -> Result<BookmarkId, BookmarkError> {
        if !self.folders.iter().any(|f| f.folder_id == parent) {
            return Err(BookmarkError::FolderNotFound(parent));
        }

        let bookmark_id = BookmarkId::new(self.next_bookmark_id);
        self.next_bookmark_id += 1;

        let position = self.bookmarks_in_folder(parent).len() as u32;
        let bookmark = Bookmark::new(
            bookmark_id,
            parent,
            title,
            address,
            created_at,
            source,
            position,
        );
        self.bookmarks.push(bookmark);

        self.save_to_disk()?;
        Ok(bookmark_id)
    }

    /// Rename an existing folder.
    pub fn rename_folder(
        &mut self,
        folder_id: FolderId,
        new_name: impl Into<String>,
    ) -> Result<(), BookmarkError> {
        let folder = self
            .folders
            .iter_mut()
            .find(|f| f.folder_id == folder_id)
            .ok_or(BookmarkError::FolderNotFound(folder_id))?;

        folder.name = new_name.into();
        self.save_to_disk()
    }

    /// Rename an existing bookmark.
    pub fn rename_bookmark(
        &mut self,
        bookmark_id: BookmarkId,
        new_title: impl Into<String>,
    ) -> Result<(), BookmarkError> {
        let bookmark = self
            .bookmarks
            .iter_mut()
            .find(|b| b.bookmark_id == bookmark_id)
            .ok_or(BookmarkError::BookmarkNotFound(bookmark_id))?;

        bookmark.title = new_title.into();
        self.save_to_disk()
    }

    /// Move a folder to a new parent folder, strictly verifying cycle prevention.
    pub fn move_folder(
        &mut self,
        folder_id: FolderId,
        new_parent: FolderId,
    ) -> Result<(), BookmarkError> {
        if folder_id.is_root() {
            return Err(BookmarkError::CannotModifyRoot);
        }

        if folder_id == new_parent {
            return Err(BookmarkError::CycleDetected(folder_id));
        }

        if !self.folders.iter().any(|f| f.folder_id == new_parent) {
            return Err(BookmarkError::FolderNotFound(new_parent));
        }

        // Verify new_parent is not inside folder_id's subtree (cycle check)
        let mut current = Some(new_parent);
        while let Some(ancestor) = current {
            if ancestor == folder_id {
                return Err(BookmarkError::CycleDetected(folder_id));
            }
            current = self
                .folders
                .iter()
                .find(|f| f.folder_id == ancestor)
                .and_then(|f| f.parent_folder);
        }

        let new_position = self.subfolders(new_parent).len() as u32;

        let folder = self
            .folders
            .iter_mut()
            .find(|f| f.folder_id == folder_id)
            .ok_or(BookmarkError::FolderNotFound(folder_id))?;

        folder.parent_folder = Some(new_parent);
        folder.position = new_position;

        self.save_to_disk()
    }

    /// Move a bookmark to a new parent folder.
    pub fn move_bookmark(
        &mut self,
        bookmark_id: BookmarkId,
        new_parent: FolderId,
    ) -> Result<(), BookmarkError> {
        if !self.folders.iter().any(|f| f.folder_id == new_parent) {
            return Err(BookmarkError::FolderNotFound(new_parent));
        }

        let new_position = self.bookmarks_in_folder(new_parent).len() as u32;

        let bookmark = self
            .bookmarks
            .iter_mut()
            .find(|b| b.bookmark_id == bookmark_id)
            .ok_or(BookmarkError::BookmarkNotFound(bookmark_id))?;

        bookmark.parent_folder = new_parent;
        bookmark.position = new_position;

        self.save_to_disk()
    }

    /// Delete a single bookmark.
    ///
    /// Persists immediately to disk with no undo log or journal.
    pub fn delete_bookmark(&mut self, bookmark_id: BookmarkId) -> Result<bool, BookmarkError> {
        let initial_len = self.bookmarks.len();
        self.bookmarks.retain(|b| b.bookmark_id != bookmark_id);

        if self.bookmarks.len() != initial_len {
            self.save_to_disk()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Delete a folder and cascade-delete its entire subtree of folders and bookmarks.
    ///
    /// Root folder cannot be deleted.
    /// Deletion erases the folder and all descendants from the store in the same operation.
    pub fn delete_folder(&mut self, folder_id: FolderId) -> Result<usize, BookmarkError> {
        if folder_id.is_root() {
            return Err(BookmarkError::CannotModifyRoot);
        }

        if !self.folders.iter().any(|f| f.folder_id == folder_id) {
            return Err(BookmarkError::FolderNotFound(folder_id));
        }

        // Collect all folders in the subtree
        let mut to_delete_folders = HashSet::new();
        to_delete_folders.insert(folder_id);

        let mut changed = true;
        while changed {
            changed = false;
            for f in &self.folders {
                if let Some(parent) = f.parent_folder {
                    if to_delete_folders.contains(&parent)
                        && !to_delete_folders.contains(&f.folder_id)
                    {
                        to_delete_folders.insert(f.folder_id);
                        changed = true;
                    }
                }
            }
        }

        let initial_bm_count = self.bookmarks.len();
        self.bookmarks
            .retain(|b| !to_delete_folders.contains(&b.parent_folder));
        let deleted_bookmarks = initial_bm_count - self.bookmarks.len();

        let initial_folder_count = self.folders.len();
        self.folders
            .retain(|f| !to_delete_folders.contains(&f.folder_id));
        let deleted_folders = initial_folder_count - self.folders.len();

        let total_deleted = deleted_bookmarks + deleted_folders;
        self.save_to_disk()?;
        Ok(total_deleted)
    }

    /// Search bookmarks by title or address (case-insensitive substring match).
    pub fn search(&self, query: &str) -> Vec<Bookmark> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }

        self.bookmarks
            .iter()
            .filter(|b| {
                b.title.to_lowercase().contains(&q) || b.address.to_lowercase().contains(&q)
            })
            .cloned()
            .collect()
    }

    /// Validate the tree invariant:
    /// - Exactly one root folder exists with `parent_folder == None`.
    /// - Every non-root folder references an existing parent folder.
    /// - No cycles exist in the folder graph.
    /// - Every bookmark references an existing folder reachable from the root.
    pub fn validate_tree(&self) -> Result<(), BookmarkError> {
        let roots: Vec<&BookmarkFolder> = self.folders.iter().filter(|f| f.is_root()).collect();
        if roots.len() != 1 {
            return Err(BookmarkError::TreeInvariantViolation(format!(
                "expected exactly 1 root folder, found {}",
                roots.len()
            )));
        }

        let root = roots[0];
        if root.parent_folder.is_some() {
            return Err(BookmarkError::TreeInvariantViolation(
                "root folder must not have a parent".into(),
            ));
        }

        let folder_ids: HashSet<FolderId> = self.folders.iter().map(|f| f.folder_id).collect();

        // Check folder parents and cycle freedom
        for folder in &self.folders {
            if folder.is_root() {
                continue;
            }

            let Some(parent) = folder.parent_folder else {
                return Err(BookmarkError::TreeInvariantViolation(format!(
                    "non-root folder {} has no parent",
                    folder.folder_id
                )));
            };

            if !folder_ids.contains(&parent) {
                return Err(BookmarkError::TreeInvariantViolation(format!(
                    "folder {} references non-existent parent {parent}",
                    folder.folder_id
                )));
            }

            // Cycle check
            let mut visited = HashSet::new();
            visited.insert(folder.folder_id);
            let mut current = parent;
            loop {
                if current.is_root() {
                    break;
                }
                if visited.contains(&current) {
                    return Err(BookmarkError::CycleDetected(current));
                }
                visited.insert(current);
                let parent_of_current = self
                    .folders
                    .iter()
                    .find(|f| f.folder_id == current)
                    .and_then(|f| f.parent_folder);

                match parent_of_current {
                    Some(next) => current = next,
                    None => {
                        return Err(BookmarkError::TreeInvariantViolation(format!(
                            "folder {current} is disconnected from root"
                        )));
                    }
                }
            }
        }

        // Check bookmarks
        for bookmark in &self.bookmarks {
            if !folder_ids.contains(&bookmark.parent_folder) {
                return Err(BookmarkError::TreeInvariantViolation(format!(
                    "bookmark {} references non-existent folder {}",
                    bookmark.bookmark_id, bookmark.parent_folder
                )));
            }
        }

        Ok(())
    }

    /// Save the bookmark tree atomically to disk.
    fn save_to_disk(&self) -> Result<(), BookmarkError> {
        self.validate_tree()?;

        if self.root.as_os_str().is_empty() {
            return Ok(());
        }

        fs::create_dir_all(&self.root)?;

        let serialized = self.serialize();
        let target_path = self.file_path();

        let counter = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let tmp_name = format!("{}.tmp.{}.{}", Self::FILE_NAME, pid, counter);
        let tmp_path = self.root.join(tmp_name);

        fs::write(&tmp_path, serialized.as_bytes())?;

        if let Err(e) = fs::rename(&tmp_path, &target_path) {
            let _ = fs::remove_file(&tmp_path);
            return Err(BookmarkError::Io(e));
        }

        Ok(())
    }

    fn serialize(&self) -> String {
        let mut out = String::new();
        out.push_str("# Local Bookmark Store\n");
        out.push_str("# Format version 1. No secondary log, journal, or cache.\n\n");

        for folder in &self.folders {
            out.push_str("[[folder]]\n");
            out.push_str(&format!("id = {}\n", folder.folder_id.as_u64()));
            if let Some(parent) = folder.parent_folder {
                out.push_str(&format!("parent = {}\n", parent.as_u64()));
            }
            out.push_str(&format!("name = {}\n", escape_string(&folder.name)));
            out.push_str(&format!("position = {}\n\n", folder.position));
        }

        for bookmark in &self.bookmarks {
            out.push_str("[[bookmark]]\n");
            out.push_str(&format!("id = {}\n", bookmark.bookmark_id.as_u64()));
            out.push_str(&format!("parent = {}\n", bookmark.parent_folder.as_u64()));
            out.push_str(&format!("title = {}\n", escape_string(&bookmark.title)));
            out.push_str(&format!("address = {}\n", escape_string(&bookmark.address)));

            let ms = bookmark
                .created_at
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            out.push_str(&format!("created_at_ms = {ms}\n"));
            out.push_str(&format!("position = {}\n", bookmark.position));

            match &bookmark.source {
                BookmarkSource::Created => {
                    out.push_str("source = \"created\"\n");
                }
                BookmarkSource::Imported { browser } => {
                    out.push_str("source = \"imported\"\n");
                    out.push_str(&format!("imported_browser = {}\n", escape_string(browser)));
                }
            }
            out.push('\n');
        }

        out
    }

    fn deserialize(content: &str) -> Result<(Vec<BookmarkFolder>, Vec<Bookmark>), BookmarkError> {
        let mut folders = Vec::new();
        let mut bookmarks = Vec::new();

        enum Section {
            None,
            Folder,
            Bookmark,
        }

        let mut section = Section::None;

        let mut f_id = None;
        let mut f_parent = None;
        let mut f_name = None;
        let mut f_pos = None;

        let mut b_id = None;
        let mut b_parent = None;
        let mut b_title = None;
        let mut b_address = None;
        let mut b_created_ms = None;
        let mut b_source_str = None;
        let mut b_browser = None;
        let mut b_pos = None;

        let flush_folder = |folders: &mut Vec<BookmarkFolder>,
                            id: &mut Option<u64>,
                            parent: &mut Option<u64>,
                            name: &mut Option<String>,
                            pos: &mut Option<u32>|
         -> Result<(), BookmarkError> {
            if let Some(fid) = id.take() {
                let n = name
                    .take()
                    .ok_or_else(|| BookmarkError::InvalidFormat("folder missing name".into()))?;
                let position = pos.take().unwrap_or(0);
                let folder_id = FolderId::new(fid);
                let parent_folder = parent.take().map(FolderId::new);

                folders.push(BookmarkFolder {
                    folder_id,
                    parent_folder,
                    name: n,
                    position,
                });
            }
            Ok(())
        };

        let flush_bookmark = |bookmarks: &mut Vec<Bookmark>,
                              id: &mut Option<u64>,
                              parent: &mut Option<u64>,
                              title: &mut Option<String>,
                              address: &mut Option<String>,
                              created_ms: &mut Option<u64>,
                              src_str: &mut Option<String>,
                              browser: &mut Option<String>,
                              pos: &mut Option<u32>|
         -> Result<(), BookmarkError> {
            if let Some(bid) = id.take() {
                let pid = parent.take().ok_or_else(|| {
                    BookmarkError::InvalidFormat("bookmark missing parent".into())
                })?;
                let t = title.take().unwrap_or_default();
                let addr = address.take().ok_or_else(|| {
                    BookmarkError::InvalidFormat("bookmark missing address".into())
                })?;
                let created_at = match created_ms.take() {
                    Some(ms) => UNIX_EPOCH + Duration::from_millis(ms),
                    None => SystemTime::now(),
                };
                let source = match src_str.take().as_deref() {
                    Some("imported") => BookmarkSource::Imported {
                        browser: browser.take().unwrap_or_default(),
                    },
                    _ => BookmarkSource::Created,
                };
                *browser = None;
                let position = pos.take().unwrap_or(0);

                bookmarks.push(Bookmark {
                    bookmark_id: BookmarkId::new(bid),
                    parent_folder: FolderId::new(pid),
                    title: t,
                    address: addr,
                    created_at,
                    source,
                    position,
                });
            }
            Ok(())
        };

        for (line_idx, raw_line) in content.lines().enumerate() {
            let line_num = line_idx + 1;
            let line = raw_line.trim();

            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if line == "[[folder]]" {
                match section {
                    Section::Folder => {
                        flush_folder(
                            &mut folders,
                            &mut f_id,
                            &mut f_parent,
                            &mut f_name,
                            &mut f_pos,
                        )?;
                    }
                    Section::Bookmark => {
                        flush_bookmark(
                            &mut bookmarks,
                            &mut b_id,
                            &mut b_parent,
                            &mut b_title,
                            &mut b_address,
                            &mut b_created_ms,
                            &mut b_source_str,
                            &mut b_browser,
                            &mut b_pos,
                        )?;
                    }
                    Section::None => {}
                }
                section = Section::Folder;
                continue;
            }

            if line == "[[bookmark]]" {
                match section {
                    Section::Folder => {
                        flush_folder(
                            &mut folders,
                            &mut f_id,
                            &mut f_parent,
                            &mut f_name,
                            &mut f_pos,
                        )?;
                    }
                    Section::Bookmark => {
                        flush_bookmark(
                            &mut bookmarks,
                            &mut b_id,
                            &mut b_parent,
                            &mut b_title,
                            &mut b_address,
                            &mut b_created_ms,
                            &mut b_source_str,
                            &mut b_browser,
                            &mut b_pos,
                        )?;
                    }
                    Section::None => {}
                }
                section = Section::Bookmark;
                continue;
            }

            let Some((k, v)) = line.split_once('=') else {
                return Err(BookmarkError::InvalidFormat(format!(
                    "line {line_num}: missing '=' separator"
                )));
            };

            let key = k.trim();
            let val = v.trim();

            match section {
                Section::Folder => match key {
                    "id" => {
                        let id_val: u64 = val.parse().map_err(|_| {
                            BookmarkError::InvalidFormat(format!(
                                "line {line_num}: invalid folder id"
                            ))
                        })?;
                        f_id = Some(id_val);
                    }
                    "parent" => {
                        let p_val: u64 = val.parse().map_err(|_| {
                            BookmarkError::InvalidFormat(format!(
                                "line {line_num}: invalid folder parent"
                            ))
                        })?;
                        f_parent = Some(p_val);
                    }
                    "name" => {
                        f_name = Some(unescape_string(val, line_num)?);
                    }
                    "position" => {
                        let pos_val: u32 = val.parse().map_err(|_| {
                            BookmarkError::InvalidFormat(format!(
                                "line {line_num}: invalid folder position"
                            ))
                        })?;
                        f_pos = Some(pos_val);
                    }
                    _ => {}
                },
                Section::Bookmark => match key {
                    "id" => {
                        let id_val: u64 = val.parse().map_err(|_| {
                            BookmarkError::InvalidFormat(format!(
                                "line {line_num}: invalid bookmark id"
                            ))
                        })?;
                        b_id = Some(id_val);
                    }
                    "parent" => {
                        let p_val: u64 = val.parse().map_err(|_| {
                            BookmarkError::InvalidFormat(format!(
                                "line {line_num}: invalid bookmark parent"
                            ))
                        })?;
                        b_parent = Some(p_val);
                    }
                    "title" => {
                        b_title = Some(unescape_string(val, line_num)?);
                    }
                    "address" => {
                        b_address = Some(unescape_string(val, line_num)?);
                    }
                    "created_at_ms" => {
                        let ms: u64 = val.parse().map_err(|_| {
                            BookmarkError::InvalidFormat(format!(
                                "line {line_num}: invalid created_at_ms"
                            ))
                        })?;
                        b_created_ms = Some(ms);
                    }
                    "position" => {
                        let pos_val: u32 = val.parse().map_err(|_| {
                            BookmarkError::InvalidFormat(format!(
                                "line {line_num}: invalid bookmark position"
                            ))
                        })?;
                        b_pos = Some(pos_val);
                    }
                    "source" => {
                        b_source_str = Some(unescape_string(val, line_num)?);
                    }
                    "imported_browser" => {
                        b_browser = Some(unescape_string(val, line_num)?);
                    }
                    _ => {}
                },
                Section::None => {}
            }
        }

        match section {
            Section::Folder => {
                flush_folder(
                    &mut folders,
                    &mut f_id,
                    &mut f_parent,
                    &mut f_name,
                    &mut f_pos,
                )?;
            }
            Section::Bookmark => {
                flush_bookmark(
                    &mut bookmarks,
                    &mut b_id,
                    &mut b_parent,
                    &mut b_title,
                    &mut b_address,
                    &mut b_created_ms,
                    &mut b_source_str,
                    &mut b_browser,
                    &mut b_pos,
                )?;
            }
            Section::None => {}
        }

        // Ensure at least root folder exists
        if !folders.iter().any(|f| f.is_root()) {
            folders.insert(0, BookmarkFolder::root());
        }

        Ok((folders, bookmarks))
    }
}

fn escape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn unescape_string(s: &str, line_num: usize) -> Result<String, BookmarkError> {
    let s = s.trim();
    if !s.starts_with('"') || !s.ends_with('"') || s.len() < 2 {
        return Err(BookmarkError::InvalidFormat(format!(
            "line {line_num}: expected quoted string, found: {s}"
        )));
    }

    let inner = &s[1..s.len() - 1];
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('\\') => out.push('\\'),
                Some('"') => out.push('"'),
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => {
                    return Err(BookmarkError::InvalidFormat(format!(
                        "line {line_num}: dangling escape character"
                    )));
                }
            }
        } else {
            out.push(ch);
        }
    }

    Ok(out)
}
