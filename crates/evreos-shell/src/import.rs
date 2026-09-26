//! FR-012 import: bookmarks and history from Chrome, Firefox and Edge.
//!
//! # What is read
//!
//! [`read_profile`] reads one profile of a browser the specification's
//! Assumptions name into [`ImportedData`]: its bookmark trees and its history
//! rows, and nothing else. Writing them into the member's stores is the
//! import's next step, which this module does not take yet.
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

#![forbid(unsafe_code)]

mod chromium;
pub mod json;
pub mod snapshot;
pub mod sqlite;

use std::fmt;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use snapshot::{Disk, FileSource, Snapshot, SnapshotError, SnapshotPolicy, StoreFiles};

/// The browsers an import reads, closed as the specification's Assumptions
/// close them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceBrowser {
    /// Google Chrome.
    Chrome,
    /// Microsoft Edge, which keeps Chromium's store formats.
    Edge,
}

impl SourceBrowser {
    /// Every source browser.
    pub const ALL: [SourceBrowser; 2] = [Self::Chrome, Self::Edge];

    /// The browser's name as imported rows record it. A product name is not
    /// interface text: it enters the folder name as a catalogue argument, the
    /// way FR-042 has brand names enter every message.
    pub fn name(self) -> &'static str {
        match self {
            Self::Chrome => "Chrome",
            Self::Edge => "Edge",
        }
    }

    /// The files, relative to a profile directory, that an import of this
    /// browser reads — every one of them, closed. No credential store is in
    /// this list, which is what "importing no site credentials" rests on.
    pub fn store_files(self) -> &'static [&'static str] {
        match self {
            Self::Chrome | Self::Edge => chromium::STORE_FILES,
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
}

impl ImportError {
    fn from_snapshot(store: &'static str, error: SnapshotError) -> Self {
        match error {
            SnapshotError::Busy { attempts } => Self::SourceBusy { store, attempts },
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
            Self::Unreadable { store, reason } => write!(f, "{store} is unreadable: {reason}"),
            Self::Io { store, error } => write!(f, "{store} could not be read: {error}"),
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
        "http" | "https" => {
            let rest = raw[colon + 1..].strip_prefix("//")?;
            let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
            let authority = &rest[..end];
            let host = authority
                .rsplit_once('@')
                .map_or(authority, |(_, host)| host);
            if host.is_empty() {
                return None;
            }
            Some(format!("{scheme}://{host}{}", &rest[end..]))
        }
        "file" => Some(format!("file:{}", &raw[colon + 1..])),
        _ => None,
    }
}

/// Chromium's timestamps: microseconds since 1601-01-01 UTC.
fn chromium_time(micros: i64) -> Option<SystemTime> {
    const UNIX_OFFSET_MICROS: i64 = 11_644_473_600_000_000;
    unix_micros(micros.checked_sub(UNIX_OFFSET_MICROS)?)
}

fn unix_micros(micros: i64) -> Option<SystemTime> {
    let micros = u64::try_from(micros).ok().filter(|m| *m > 0)?;
    UNIX_EPOCH.checked_add(Duration::from_micros(micros))
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
        ] {
            assert_eq!(clean_address(skipped), None, "{skipped:?}");
        }
    }
}
