//! FR-012 import: bookmarks and history from Chrome, Firefox and Edge.
//!
//! # What an import is
//!
//! An [`ImportJob`] reads one profile of one of the three browsers the
//! specification's Assumptions name, and writes what it read into the
//! member's own stores as **ordinary rows marked as imported**: every history
//! row carries [`HistorySource::Imported`] and every bookmark
//! [`BookmarkSource::Imported`], each naming the browser, and from the moment
//! they exist they are the member's history in exactly the sense FR-007a
//! defines — class L, local, never transmitted, deleted by the same deletion
//! as any other row (data-model §1.13). The job's state, scope and counts are
//! that section's fields.
//!
//! # What an import never touches
//!
//! **Site credentials** — FR-015a forbids Evreos holding them, so what may
//! not be held cannot be imported (Q-E5). This is structural rather than a
//! filter: the readers open exactly the files [`SourceBrowser::store_files`]
//! lists, and no credential store — Chromium's `Login Data`, Firefox's
//! `logins.json`, `logins.db` or `key4.db` — is among them. The one place a
//! credential can hide inside a store that *is* read is an address carrying a
//! user name and password (`https://user:secret@host/`); [`clean_address`]
//! removes that part from every imported address before a row is written.
//!
//! **The network** — reading another browser's files is the local computation
//! FR-007a permits. Nothing in this module or its submodules references
//! `evreos-net` or any part of this crate but the stores, which
//! `tests/import.rs` asserts.
//!
//! # How a running browser's store is read
//!
//! All three browsers keep their stores under an exclusive SQLite lock while
//! they run, so the store is copied into memory and read from the copy, and
//! the copy is verified rather than trusted — [`snapshot`] states the
//! protocol, and `docs/measurements/import-profile-read.md` records the
//! measurement it rests on. The copy is never written to disk.
//!
//! # Threads
//!
//! Reading a store of tens of megabytes is work SC-006 forbids on the UI
//! thread, so the job splits in two: [`ImportJob::start_reading`] hands out a
//! [`ReadRequest`] that is `Send` and runs on the shell worker pool, and
//! [`ImportJob::finish`] writes its result into the stores on the thread that
//! owns them. [`ImportJob::run`] does both in sequence for a caller with no
//! UI thread to protect.

#![forbid(unsafe_code)]

mod chromium;
mod firefox;
pub mod json;
pub mod snapshot;
pub mod sqlite;

use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use evreos_i18n::{Language, catalogue};

use crate::store::{
    BookmarkError, BookmarkSource, BookmarkStore, FolderId, HistoryError, HistorySource,
    NewHistoryEntry, StoreRegistry, WindowKind,
};
use snapshot::{Disk, FileSource, Snapshot, SnapshotError, SnapshotPolicy, StoreFiles};

/// The browsers an import reads, closed as the specification's Assumptions
/// close them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceBrowser {
    /// Google Chrome.
    Chrome,
    /// Microsoft Edge, which keeps Chromium's store formats.
    Edge,
    /// Mozilla Firefox.
    Firefox,
}

impl SourceBrowser {
    /// Every source browser.
    pub const ALL: [SourceBrowser; 3] = [Self::Chrome, Self::Edge, Self::Firefox];

    /// The browser's name as imported rows record it. A product name is not
    /// interface text: it enters the folder name as a catalogue argument, the
    /// way FR-042 has brand names enter every message.
    pub fn name(self) -> &'static str {
        match self {
            Self::Chrome => "Chrome",
            Self::Edge => "Edge",
            Self::Firefox => "Firefox",
        }
    }

    /// The files, relative to a profile directory, that an import of this
    /// browser reads — every one of them, closed. No credential store is in
    /// this list, which is what "importing no site credentials" rests on.
    pub fn store_files(self) -> &'static [&'static str] {
        match self {
            Self::Chrome | Self::Edge => chromium::STORE_FILES,
            Self::Firefox => firefox::STORE_FILES,
        }
    }
}

/// One profile of one source browser, found on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceProfile {
    /// Which browser it belongs to.
    pub browser: SourceBrowser,
    /// The name the browser shows for it, or its directory name.
    pub name: String,
    /// The profile directory.
    pub path: PathBuf,
}

impl SourceProfile {
    /// A profile at an explicit path.
    pub fn new(browser: SourceBrowser, name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            browser,
            name: name.into(),
            path: path.into(),
        }
    }
}

/// Where each browser keeps its profiles on this machine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileLocations {
    /// Chrome's `User Data` directory.
    pub chrome: Option<PathBuf>,
    /// Edge's `User Data` directory.
    pub edge: Option<PathBuf>,
    /// The directory holding Firefox's `profiles.ini`.
    pub firefox: Option<PathBuf>,
}

impl ProfileLocations {
    /// Each browser's default location on this platform, from the
    /// environment. Nothing here calls a platform service: the locations are
    /// the documented directories under the user's own profile.
    pub fn from_environment() -> Self {
        let var = |name: &str| std::env::var_os(name).map(PathBuf::from);
        if cfg!(windows) {
            let local = var("LOCALAPPDATA");
            Self {
                chrome: local.as_ref().map(|d| d.join("Google/Chrome/User Data")),
                edge: local.as_ref().map(|d| d.join("Microsoft/Edge/User Data")),
                firefox: var("APPDATA").map(|d| d.join("Mozilla/Firefox")),
            }
        } else if cfg!(target_os = "macos") {
            let support = var("HOME").map(|d| d.join("Library/Application Support"));
            Self {
                chrome: support.as_ref().map(|d| d.join("Google/Chrome")),
                edge: support.as_ref().map(|d| d.join("Microsoft Edge")),
                firefox: support.as_ref().map(|d| d.join("Firefox")),
            }
        } else {
            let home = var("HOME");
            Self {
                chrome: home.as_ref().map(|d| d.join(".config/google-chrome")),
                edge: home.as_ref().map(|d| d.join(".config/microsoft-edge")),
                firefox: home.as_ref().map(|d| d.join(".mozilla/firefox")),
            }
        }
    }
}

/// Every profile of every source browser found under `locations`, Chrome's
/// first, then Edge's, then Firefox's.
pub fn discover(locations: &ProfileLocations) -> Vec<SourceProfile> {
    let mut found = Vec::new();
    if let Some(dir) = &locations.chrome {
        found.extend(chromium::discover(SourceBrowser::Chrome, dir));
    }
    if let Some(dir) = &locations.edge {
        found.extend(chromium::discover(SourceBrowser::Edge, dir));
    }
    if let Some(dir) = &locations.firefox {
        found.extend(firefox::discover(dir));
    }
    found
}

/// What the member chose to import. Site credentials are not a field: they
/// are never imported (Q-E5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportScope {
    /// Import bookmarks.
    pub bookmarks: bool,
    /// Import history.
    pub history: bool,
}

impl ImportScope {
    /// Bookmarks and history both.
    pub const ALL: Self = Self {
        bookmarks: true,
        history: true,
    };
}

/// Rows written by one job.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportCounts {
    /// Bookmark rows written.
    pub bookmarks_imported: usize,
    /// History rows written.
    pub history_imported: usize,
}

/// Why a job failed, as its state records it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFailure {
    /// The profile directory is gone.
    ProfileMissing,
    /// The browser kept writing through every attempt to copy a store.
    SourceBusy,
    /// A store was held mid-write at every attempt without changing: the
    /// browser left it so, most likely by stopping mid-write.
    SourceInterrupted,
    /// A store is not in a format this reader understands.
    Unreadable,
    /// Evreos's own stores could not be written; nothing was imported.
    WriteFailed,
}

/// Where a job is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportState {
    /// Created, not started.
    Pending,
    /// Reading the source profile.
    Reading,
    /// Every row written.
    Written,
    /// Stopped, having written nothing.
    Failed(ImportFailure),
}

/// Why an import failed. No variant carries an address, a title or any other
/// value read from the source profile, so an error can be logged as it is.
#[derive(Debug)]
pub enum ImportError {
    /// The profile directory does not exist.
    ProfileMissing,
    /// The named store never held still long enough to copy; the member can
    /// close that browser and try again.
    SourceBusy {
        /// The store's file name.
        store: &'static str,
        /// Attempts made.
        attempts: u32,
    },
    /// The named store had a write open at every attempt and never changed,
    /// which is what a browser that stopped mid-write leaves behind; opening
    /// that browser and closing it again completes or undoes the write.
    SourceInterrupted {
        /// The store's file name.
        store: &'static str,
        /// Attempts made.
        attempts: u32,
    },
    /// The named store could not be parsed.
    Unreadable {
        /// The store's file name.
        store: &'static str,
        /// The parser's reason, which names structure and never content.
        reason: String,
    },
    /// The named store could not be read from disk.
    Io {
        /// The store's file name.
        store: &'static str,
        /// The underlying error.
        error: io::Error,
    },
    /// The bookmark store refused the write.
    Bookmarks(BookmarkError),
    /// The history store refused the write.
    History(HistoryError),
    /// A folder name did not resolve from the catalogue.
    Catalogue(String),
    /// The history store refused the write, and removing the bookmarks this
    /// import had already written failed as well: they remain.
    RollbackFailed {
        /// Why the history write failed.
        history: HistoryError,
        /// Why the bookmarks could not be removed.
        bookmarks: BookmarkError,
    },
}

impl ImportError {
    /// The failure a job's state records for this error.
    pub fn failure(&self) -> ImportFailure {
        match self {
            Self::ProfileMissing => ImportFailure::ProfileMissing,
            Self::SourceBusy { .. } => ImportFailure::SourceBusy,
            Self::SourceInterrupted { .. } => ImportFailure::SourceInterrupted,
            Self::Unreadable { .. } | Self::Io { .. } => ImportFailure::Unreadable,
            Self::Bookmarks(_)
            | Self::History(_)
            | Self::Catalogue(_)
            | Self::RollbackFailed { .. } => ImportFailure::WriteFailed,
        }
    }

    fn from_snapshot(store: &'static str, error: SnapshotError) -> Self {
        match error {
            SnapshotError::Busy { attempts } => Self::SourceBusy { store, attempts },
            SnapshotError::Interrupted { attempts } => Self::SourceInterrupted { store, attempts },
            SnapshotError::Io(error) => Self::Io { store, error },
            // Absent stores are handled by the callers, which read them as empty.
            SnapshotError::Absent => Self::Unreadable {
                store,
                reason: "the store is absent".into(),
            },
        }
    }

    fn unreadable(store: &'static str, reason: impl fmt::Display) -> Self {
        Self::Unreadable {
            store,
            reason: reason.to_string(),
        }
    }
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProfileMissing => write!(f, "the source profile does not exist"),
            Self::SourceBusy { store, attempts } => write!(
                f,
                "{store} changed during each of {attempts} attempts to copy it"
            ),
            Self::SourceInterrupted { store, attempts } => write!(
                f,
                "{store} held an unfinished write through each of {attempts} attempts \
                 to copy it"
            ),
            Self::Unreadable { store, reason } => write!(f, "{store} is unreadable: {reason}"),
            Self::Io { store, error } => write!(f, "{store} could not be read: {error}"),
            Self::Bookmarks(error) => write!(f, "{error}"),
            Self::History(error) => write!(f, "{error}"),
            Self::Catalogue(key) => write!(f, "catalogue key {key} did not resolve"),
            Self::RollbackFailed { history, bookmarks } => write!(
                f,
                "{history}; the bookmarks already imported could not be removed: {bookmarks}"
            ),
        }
    }
}

impl std::error::Error for ImportError {}

/// The four top-level containers the source browsers file bookmarks under.
/// Each becomes a folder named from the catalogue in the member's language,
/// since the source browsers store these names untranslated or in their own
/// interface language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RootKind {
    /// Chromium's bookmark bar, Firefox's bookmarks toolbar.
    Toolbar,
    /// Firefox's bookmarks menu. Chromium has none.
    Menu,
    /// Chromium's "other bookmarks", Firefox's unfiled bookmarks.
    Other,
    /// Bookmarks synced from a phone, in either browser.
    Mobile,
}

impl RootKind {
    /// The order the root folders are written in.
    pub const ALL: [RootKind; 4] = [Self::Toolbar, Self::Menu, Self::Other, Self::Mobile];

    fn catalogue_key(self) -> &'static str {
        match self {
            Self::Toolbar => "import.root.toolbar",
            Self::Menu => "import.root.menu",
            Self::Other => "import.root.other",
            Self::Mobile => "import.root.mobile",
        }
    }
}

/// A bookmark or folder read from a source profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportedNode {
    /// A folder and what it holds, in the source's order.
    Folder {
        /// Its name in the source.
        title: String,
        /// Its contents.
        children: Vec<ImportedNode>,
    },
    /// A bookmark.
    Bookmark {
        /// Its title, or its address when the source gave none.
        title: String,
        /// Its address, cleaned by [`clean_address`].
        address: String,
        /// When the source says it was added.
        added_at: Option<SystemTime>,
    },
}

/// One source root and what it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedRoot {
    /// Which root.
    pub kind: RootKind,
    /// Its contents, in the source's order.
    pub children: Vec<ImportedNode>,
}

/// One history row read from a source profile: an address and its most
/// recent visit, which is what both Chromium's and Firefox's own importers
/// carry across rather than every visit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedVisit {
    /// The address, cleaned by [`clean_address`].
    pub address: String,
    /// The page title the source recorded.
    pub title: String,
    /// The most recent visit.
    pub visited_at: SystemTime,
}

/// Everything read from one profile. Class L: it is the member's history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedData {
    /// The browser it was read from.
    pub browser: SourceBrowser,
    /// The bookmark roots holding at least one node, in [`RootKind::ALL`]
    /// order.
    pub roots: Vec<ImportedRoot>,
    /// History rows.
    pub history: Vec<ImportedVisit>,
}

impl ImportedData {
    /// The number of bookmarks, not counting folders.
    pub fn bookmark_count(&self) -> usize {
        fn count(nodes: &[ImportedNode]) -> usize {
            nodes
                .iter()
                .map(|node| match node {
                    ImportedNode::Folder { children, .. } => count(children),
                    ImportedNode::Bookmark { .. } => 1,
                })
                .sum()
        }
        self.roots.iter().map(|root| count(&root.children)).sum()
    }
}

/// Read one profile, off the UI thread.
#[derive(Debug, Clone)]
pub struct ReadRequest {
    profile: SourceProfile,
    scope: ImportScope,
    policy: SnapshotPolicy,
}

impl ReadRequest {
    /// Read the profile. This is the half of an import that may take
    /// seconds, and the half that belongs on the worker pool.
    pub fn execute(self) -> Result<ImportedData, ImportError> {
        read_profile_with(&self.profile, self.scope, self.policy, &mut Disk)
    }
}

/// Read `profile` with the default copy policy.
pub fn read_profile(
    profile: &SourceProfile,
    scope: ImportScope,
) -> Result<ImportedData, ImportError> {
    read_profile_with(profile, scope, SnapshotPolicy::default(), &mut Disk)
}

/// Read `profile` through `source` under `policy`.
pub fn read_profile_with(
    profile: &SourceProfile,
    scope: ImportScope,
    policy: SnapshotPolicy,
    source: &mut impl FileSource,
) -> Result<ImportedData, ImportError> {
    if !profile.path.is_dir() {
        return Err(ImportError::ProfileMissing);
    }
    let (roots, history) = match profile.browser {
        SourceBrowser::Chrome | SourceBrowser::Edge => {
            chromium::read(&profile.path, scope, policy, source)?
        }
        SourceBrowser::Firefox => firefox::read(&profile.path, scope, policy, source)?,
    };
    let roots = RootKind::ALL
        .iter()
        .filter_map(|kind| roots.iter().find(|root| root.kind == *kind))
        .filter(|root| !root.children.is_empty())
        .cloned()
        .collect();
    Ok(ImportedData {
        browser: profile.browser,
        roots,
        history,
    })
}

/// Take a verified copy of one store, reading an absent store as `None`.
fn copy_store(
    source: &mut impl FileSource,
    files: StoreFiles,
    store: &'static str,
    policy: SnapshotPolicy,
) -> Result<Option<Snapshot>, ImportError> {
    match snapshot::take(source, &files, policy) {
        Ok(copy) => Ok(Some(copy)),
        Err(SnapshotError::Absent) => Ok(None),
        Err(error) => Err(ImportError::from_snapshot(store, error)),
    }
}

/// One import of one profile, with the state data-model §1.13 gives it.
#[derive(Debug, Clone)]
pub struct ImportJob {
    profile: SourceProfile,
    scope: ImportScope,
    policy: SnapshotPolicy,
    state: ImportState,
    counts: ImportCounts,
}

impl ImportJob {
    /// A pending job.
    pub fn new(profile: SourceProfile, scope: ImportScope) -> Self {
        Self {
            profile,
            scope,
            policy: SnapshotPolicy::default(),
            state: ImportState::Pending,
            counts: ImportCounts::default(),
        }
    }

    /// Replace the copy policy.
    pub fn with_policy(mut self, policy: SnapshotPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// The browser being imported from.
    pub fn source_browser(&self) -> SourceBrowser {
        self.profile.browser
    }

    /// What is being imported.
    pub fn scope(&self) -> ImportScope {
        self.scope
    }

    /// Where the job is.
    pub fn state(&self) -> ImportState {
        self.state
    }

    /// Rows written; zero until the job is [`ImportState::Written`].
    pub fn counts(&self) -> ImportCounts {
        self.counts
    }

    /// Mark the job reading and hand out the read, for the worker pool.
    pub fn start_reading(&mut self) -> ReadRequest {
        self.state = ImportState::Reading;
        ReadRequest {
            profile: self.profile.clone(),
            scope: self.scope,
            policy: self.policy,
        }
    }

    /// Write a finished read into `stores`, on the thread that owns them.
    ///
    /// Folder names resolve in `language`; a bookmark with no date of its
    /// own is dated `now`, and a history row with no visit time was never
    /// read. The guarantee on a failed write is [`write_imported`]'s.
    pub fn finish(
        &mut self,
        read: Result<ImportedData, ImportError>,
        stores: &mut StoreRegistry,
        language: Language,
        now: SystemTime,
    ) -> Result<ImportCounts, ImportError> {
        let result = read.and_then(|data| write_imported(&data, stores, language, now));
        match &result {
            Ok(counts) => {
                self.counts = *counts;
                self.state = ImportState::Written;
            }
            Err(error) => self.state = ImportState::Failed(error.failure()),
        }
        result
    }

    /// Read and write in sequence, on this thread.
    pub fn run(
        &mut self,
        stores: &mut StoreRegistry,
        language: Language,
    ) -> Result<ImportCounts, ImportError> {
        let read = self.start_reading().execute();
        self.finish(read, stores, language, SystemTime::now())
    }
}

/// Write `data` into `stores` as rows marked imported.
///
/// Bookmarks go into one new folder at the top of the member's tree, named
/// for the browser they came from, holding one folder per non-empty source
/// root; a second import of the same profile makes a second such folder, as
/// the source browsers' own importers do. History rows already present — the
/// same address at the same instant, as a repeated import produces — are not
/// written twice.
///
/// Each store takes its rows in one write or none of them. The bookmarks are
/// written first; when the history write then fails, the folder this import
/// created is removed again, so the failure leaves no rows behind. Two cases
/// escape that, and are stated rather than hidden: if the removal itself
/// cannot be saved, [`ImportError::RollbackFailed`] reports that the
/// bookmarks remain; and the process ending between the two writes leaves
/// the bookmarks without the history, which a second import completes.
pub fn write_imported(
    data: &ImportedData,
    stores: &mut StoreRegistry,
    language: Language,
    now: SystemTime,
) -> Result<ImportCounts, ImportError> {
    let browser = data.browser.name();
    let messages = catalogue(language);
    let resolve = |key: &str, arguments: &[(&str, &str)]| {
        messages
            .resolve(key, arguments)
            .map_err(|_| ImportError::Catalogue(key.to_string()))
    };

    let mut top_folder = None;
    if !data.roots.is_empty() {
        let top_name = resolve("import.folder", &[("browser", browser)])?;
        let mut root_names = Vec::new();
        for root in &data.roots {
            root_names.push(resolve(root.kind.catalogue_key(), &[])?);
        }
        let folder = stores
            .bookmarks_mut()
            .batch(|store| {
                let top = store.create_folder(FolderId::ROOT, top_name)?;
                for (root, name) in data.roots.iter().zip(root_names) {
                    let folder = store.create_folder(top, name)?;
                    write_nodes(store, folder, &root.children, browser, now)?;
                }
                Ok(top)
            })
            .map_err(ImportError::Bookmarks)?;
        top_folder = Some(folder);
    }

    let history = stores.history_mut();
    let mut seen: HashSet<(String, u128)> = history
        .entries()
        .iter()
        .map(|entry| (entry.address.clone(), millis(entry.visited_at)))
        .collect();
    let fresh: Vec<NewHistoryEntry> = data
        .history
        .iter()
        .filter(|visit| seen.insert((visit.address.clone(), millis(visit.visited_at))))
        .map(|visit| NewHistoryEntry {
            address: visit.address.clone(),
            title: visit.title.clone(),
            visited_at: visit.visited_at,
            source: HistorySource::imported(browser),
        })
        .collect();
    let written = match history.record_batch(fresh, WindowKind::Normal) {
        Ok(ids) => ids.len(),
        Err(error) => {
            // Undo the bookmark half, and say so if that fails too.
            if let Some(folder) = top_folder {
                // Inside a batch, so that a removal whose save fails leaves
                // the folder in memory too, agreeing with what is on disk.
                let removal = stores
                    .bookmarks_mut()
                    .batch(|store| store.delete_folder(folder));
                if let Err(bookmarks) = removal {
                    return Err(ImportError::RollbackFailed {
                        history: error,
                        bookmarks,
                    });
                }
            }
            return Err(ImportError::History(error));
        }
    };

    Ok(ImportCounts {
        bookmarks_imported: data.bookmark_count(),
        history_imported: written,
    })
}

fn write_nodes(
    store: &mut BookmarkStore,
    parent: FolderId,
    nodes: &[ImportedNode],
    browser: &str,
    now: SystemTime,
) -> Result<(), BookmarkError> {
    for node in nodes {
        match node {
            ImportedNode::Folder { title, children } => {
                let folder = store.create_folder(parent, title.clone())?;
                write_nodes(store, folder, children, browser, now)?;
            }
            ImportedNode::Bookmark {
                title,
                address,
                added_at,
            } => {
                store.create_bookmark_with_details(
                    parent,
                    title.clone(),
                    address.clone(),
                    added_at.unwrap_or(now),
                    BookmarkSource::imported(browser),
                )?;
            }
        }
    }
    Ok(())
}

fn millis(time: SystemTime) -> u128 {
    time.duration_since(UNIX_EPOCH)
        .map(|since| since.as_millis())
        .unwrap_or(0)
}

/// The address to import for `raw`, or `None` if it is not one to import.
///
/// Only addresses a page load can use are imported — `http`, `https` and
/// `file`. Browser-internal pages (`chrome:`, `edge:`, `about:`), Firefox's
/// `place:` queries, extension pages, script (`javascript:`) and inline data
/// (`data:`, `blob:`) are not addresses of anywhere the member went, and are
/// skipped. A user name and password carried in the address are removed:
/// they are a site credential, which an import never carries (Q-E5).
pub fn clean_address(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() || raw.chars().any(|ch| ch.is_control() || ch == ' ') {
        return None;
    }
    let colon = raw.find(':')?;
    let scheme = raw[..colon].to_ascii_lowercase();
    match scheme.as_str() {
        "http" | "https" | "file" => {
            let after = &raw[colon + 1..];
            let Some(rest) = after.strip_prefix("//") else {
                // Only `file:/path` is a path with no authority, so no
                // credential. A browser reads `\\` and `/\` as the start
                // of an authority, where a user name and password can sit,
                // and any other shape is not an address it loads.
                let plain_path = scheme == "file"
                    && after.starts_with('/')
                    && !after[1..].starts_with(['/', '\\']);
                return plain_path.then(|| format!("file:{after}"));
            };
            // A browser ends the authority at a backslash too, for these
            // schemes; ending it there keeps the host the one it loaded.
            let end = rest.find(['/', '?', '#', '\\']).unwrap_or(rest.len());
            let authority = &rest[..end];
            let host = authority
                .rsplit_once('@')
                .map_or(authority, |(_, host)| host);
            // A web address needs a host; a local file's may be empty.
            if host.is_empty() && scheme != "file" {
                return None;
            }
            Some(format!("{scheme}://{host}{}", &rest[end..]))
        }
        _ => None,
    }
}

/// Chromium's timestamps: microseconds since 1601-01-01 UTC.
fn chromium_time(micros: i64) -> Option<SystemTime> {
    const UNIX_OFFSET_MICROS: i64 = 11_644_473_600_000_000;
    unix_micros(micros.checked_sub(UNIX_OFFSET_MICROS)?)
}

/// Firefox's timestamps (PRTime): microseconds since the Unix epoch.
fn firefox_time(micros: i64) -> Option<SystemTime> {
    unix_micros(micros)
}

fn unix_micros(micros: i64) -> Option<SystemTime> {
    let micros = u64::try_from(micros).ok().filter(|m| *m > 0)?;
    UNIX_EPOCH.checked_add(Duration::from_micros(micros))
}

/// Whether a directory entry is a directory, following no further than
/// `fs::metadata` does.
fn is_dir(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|meta| meta.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_lose_credentials_and_keep_everything_else() {
        assert_eq!(
            clean_address("https://alice:hunter2@example.com/a?b#c").as_deref(),
            Some("https://example.com/a?b#c")
        );
        assert_eq!(
            clean_address("HTTP://user@host:8080").as_deref(),
            Some("http://host:8080")
        );
        assert_eq!(
            clean_address("https://example.com/path@not-userinfo").as_deref(),
            Some("https://example.com/path@not-userinfo")
        );
        assert_eq!(
            clean_address("file:///home/a/b.html").as_deref(),
            Some("file:///home/a/b.html")
        );
        assert_eq!(
            clean_address("file://user:secret@server/share/x").as_deref(),
            Some("file://server/share/x")
        );
        assert_eq!(
            clean_address("https://u:p@host\\p@x").as_deref(),
            Some("https://host\\p@x")
        );
        assert_eq!(
            clean_address("file:/home/a/b.html").as_deref(),
            Some("file:/home/a/b.html")
        );
    }

    #[test]
    fn addresses_that_are_not_pages_are_skipped() {
        for skipped in [
            "javascript:alert(1)",
            "data:text/html,hi",
            "chrome://settings",
            "edge://favorites",
            "about:blank",
            "place:sort=8",
            "moz-extension://x/y",
            "https://",
            "https://user:pw@/",
            "no scheme",
            "",
            "https://exa\nmple.com",
            "file:/\\u:secret@server/x",
            "file:\\\\u:secret@server\\x",
            "file:u:secret@server/x",
        ] {
            assert_eq!(clean_address(skipped), None, "{skipped:?}");
        }
    }

    #[test]
    fn timestamps_convert_from_both_epochs() {
        let unix = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        assert_eq!(chromium_time(13_344_473_600_000_000), Some(unix));
        assert_eq!(firefox_time(1_700_000_000_000_000), Some(unix));
        assert_eq!(chromium_time(0), None);
        assert_eq!(chromium_time(i64::MIN), None);
        assert_eq!(firefox_time(-5), None);
    }
}
