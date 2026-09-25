//! Persistent browsing history store.
//!
//! # Architecture and Invariants
//!
//! - **Local Residence (FR-004, FR-007a, Invariant A)**:
//!   Browsing history is strictly local. It is never transmitted, synchronised,
//!   or retained off the machine in whole or derived form.
//! - **Private Window Isolation (FR-007, FR-007a)**:
//!   Navigations in a [`WindowKind::Private`] window produce no [`HistoryEntry`]
//!   and leave no trace on disk or in memory.
//! - **Cascade Deletion without Undo or Journals (FR-004, Invariant A)**:
//!   Deleting a single entry or a chosen time range erases the rows from the
//!   store and every derived index in the same operation. No undo log,
//!   append-only journal, or secondary index with an independent lifetime is
//!   maintained.

#![forbid(unsafe_code)]

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static TMP_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Unique identifier for a history entry within the local store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HistoryEntryId(pub u64);

impl HistoryEntryId {
    /// Create a new history entry identifier.
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// The raw 64-bit integer representation.
    pub const fn as_u64(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for HistoryEntryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for HistoryEntryId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

/// The origin or provenance of a history entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistorySource {
    /// Navigated directly by the member during standard browsing.
    Navigated,
    /// Imported from an external browser profile (FR-012).
    Imported {
        /// Identifier or name of the source browser application.
        browser: String,
    },
}

impl HistorySource {
    /// Construct an imported history source.
    pub fn imported(browser: impl Into<String>) -> Self {
        Self::Imported {
            browser: browser.into(),
        }
    }
}

/// Window browsing mode.
///
/// Under FR-007, private window navigations must produce no history entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowKind {
    /// Standard browsing window whose navigations record to the history store.
    #[default]
    Normal,
    /// Private browsing window leaving no trace after closure.
    Private,
}

/// A single recorded browsing history entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    /// The unique local identifier.
    pub entry_id: HistoryEntryId,
    /// The navigated address / URL.
    pub address: String,
    /// The title of the page at navigation time.
    pub title: String,
    /// The timestamp when the navigation occurred.
    pub visited_at: SystemTime,
    /// The origin of this entry.
    pub source: HistorySource,
}

impl HistoryEntry {
    /// Construct a new history entry.
    pub fn new(
        entry_id: HistoryEntryId,
        address: impl Into<String>,
        title: impl Into<String>,
        visited_at: SystemTime,
        source: HistorySource,
    ) -> Self {
        Self {
            entry_id,
            address: address.into(),
            title: title.into(),
            visited_at,
            source,
        }
    }

    /// The unique local entry identifier.
    pub fn entry_id(&self) -> HistoryEntryId {
        self.entry_id
    }

    /// The navigated address.
    pub fn address(&self) -> &str {
        &self.address
    }

    /// The recorded page title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The timestamp of the visit.
    pub fn visited_at(&self) -> SystemTime {
        self.visited_at
    }

    /// The provenance of this entry.
    pub fn source(&self) -> &HistorySource {
        &self.source
    }
}

/// Errors occurring during history store operations or persistence.
#[derive(Debug)]
pub enum HistoryError {
    /// Filesystem I/O failure.
    Io(io::Error),
    /// Format or parsing error in the history file.
    InvalidFormat(String),
}

impl fmt::Display for HistoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "history I/O error: {err}"),
            Self::InvalidFormat(msg) => write!(f, "invalid history file format: {msg}"),
        }
    }
}

impl std::error::Error for HistoryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::InvalidFormat(_) => None,
        }
    }
}

impl From<io::Error> for HistoryError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

/// The persistent local history store.
///
/// Stores entries locally under the profile directory without any secondary
/// log, journal, or remote egress.
#[derive(Debug, Clone)]
pub struct HistoryStore {
    root: PathBuf,
    entries: Vec<HistoryEntry>,
    next_id: u64,
}

impl Default for HistoryStore {
    fn default() -> Self {
        Self {
            root: PathBuf::new(),
            entries: Vec::new(),
            next_id: 1,
        }
    }
}

impl HistoryStore {
    const FILE_NAME: &'static str = "history.toml";

    /// Open or initialise the history store under the given profile root.
    ///
    /// If the history file does not exist, an empty store is created. If opening
    /// fails due to I/O or format error, an empty store is returned.
    pub fn open(root: &Path) -> Self {
        Self::try_open(root).unwrap_or_else(|_| Self::new_empty(root))
    }

    /// Attempt to open the history store under `root`, returning any error encountered.
    pub fn try_open(root: &Path) -> Result<Self, HistoryError> {
        let file_path = root.join(Self::FILE_NAME);
        if !file_path.exists() {
            return Ok(Self::new_empty(root));
        }

        let content = fs::read_to_string(&file_path)?;
        let entries = Self::deserialize(&content)?;
        let max_id = entries
            .iter()
            .map(|e| e.entry_id.as_u64())
            .max()
            .unwrap_or(0);

        Ok(Self {
            root: root.to_path_buf(),
            entries,
            next_id: max_id + 1,
        })
    }

    fn new_empty(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            entries: Vec::new(),
            next_id: 1,
        }
    }

    /// The filesystem path to the history file under the profile root.
    pub fn file_path(&self) -> PathBuf {
        self.root.join(Self::FILE_NAME)
    }

    /// Record a navigation into the history store.
    ///
    /// If `window_kind` is [`WindowKind::Private`], no entry is produced,
    /// nothing is persisted, and `Ok(None)` is returned (FR-007).
    pub fn record(
        &mut self,
        address: &str,
        title: &str,
        window_kind: WindowKind,
    ) -> Result<Option<HistoryEntryId>, HistoryError> {
        self.record_with_details(
            address,
            title,
            SystemTime::now(),
            HistorySource::Navigated,
            window_kind,
        )
    }

    /// Record a navigation with full details.
    ///
    /// If `window_kind` is [`WindowKind::Private`], this is an immediate no-op returning `Ok(None)`.
    pub fn record_with_details(
        &mut self,
        address: impl Into<String>,
        title: impl Into<String>,
        visited_at: SystemTime,
        source: HistorySource,
        window_kind: WindowKind,
    ) -> Result<Option<HistoryEntryId>, HistoryError> {
        if window_kind == WindowKind::Private {
            return Ok(None);
        }

        let id = HistoryEntryId::new(self.next_id);
        self.next_id += 1;

        let entry = HistoryEntry::new(id, address, title, visited_at, source);
        self.entries.push(entry);

        self.save_to_disk()?;
        Ok(Some(id))
    }

    /// Review browsing history entries in reverse chronological order (newest first).
    pub fn review(&self) -> Vec<HistoryEntry> {
        let mut list = self.entries.clone();
        list.sort_by_key(|b| {
            (
                std::cmp::Reverse(b.visited_at),
                std::cmp::Reverse(b.entry_id),
            )
        });
        list
    }

    /// Access all history entries as an unsorted slice.
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// Search history entries matching `query` in either address or title (case-insensitive).
    ///
    /// Returns matching entries ordered by `visited_at` descending (newest first).
    pub fn search(&self, query: &str) -> Vec<HistoryEntry> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }

        let mut matches: Vec<HistoryEntry> = self
            .entries
            .iter()
            .filter(|e| {
                e.address.to_lowercase().contains(&q) || e.title.to_lowercase().contains(&q)
            })
            .cloned()
            .collect();

        matches.sort_by_key(|b| {
            (
                std::cmp::Reverse(b.visited_at),
                std::cmp::Reverse(b.entry_id),
            )
        });
        matches
    }

    /// Look up a single entry by its unique local identifier.
    pub fn get(&self, id: HistoryEntryId) -> Option<&HistoryEntry> {
        self.entries.iter().find(|e| e.entry_id == id)
    }

    /// Delete a single entry by identifier.
    ///
    /// Erases the entry from the store and persists immediately to disk in the same
    /// operation. No undo log or journal is produced (FR-004).
    /// Returns `true` if the entry was found and removed, `false` otherwise.
    pub fn delete_entry(&mut self, id: HistoryEntryId) -> Result<bool, HistoryError> {
        let initial_len = self.entries.len();
        self.entries.retain(|e| e.entry_id != id);

        if self.entries.len() != initial_len {
            self.save_to_disk()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Delete entries within a chosen time range `[from, to]` inclusive.
    ///
    /// Erases all matching entries immediately from the store and writes the updated
    /// store to disk in the same operation with no undo log or journal (FR-004).
    /// Returns the number of entries erased.
    pub fn delete_range(
        &mut self,
        from: SystemTime,
        to: SystemTime,
    ) -> Result<usize, HistoryError> {
        let (min_t, max_t) = if from <= to { (from, to) } else { (to, from) };

        let initial_len = self.entries.len();
        self.entries
            .retain(|e| e.visited_at < min_t || e.visited_at > max_t);

        let removed = initial_len - self.entries.len();
        if removed > 0 {
            self.save_to_disk()?;
        }
        Ok(removed)
    }

    /// Clear all history entries completely.
    pub fn clear(&mut self) -> Result<usize, HistoryError> {
        let count = self.entries.len();
        self.entries.clear();
        self.save_to_disk()?;
        Ok(count)
    }

    /// Total count of entries currently held.
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Whether the history store holds no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Atomically write all current entries to disk.
    ///
    /// Overwrites the target file directly without retaining secondary journals,
    /// undo logs, or intermediate state files.
    fn save_to_disk(&self) -> Result<(), HistoryError> {
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

        // On Windows and Unix, replace atomic target.
        if let Err(e) = fs::rename(&tmp_path, &target_path) {
            let _ = fs::remove_file(&tmp_path);
            return Err(HistoryError::Io(e));
        }

        Ok(())
    }

    /// Serialize entries into toml table entries.
    fn serialize(&self) -> String {
        let mut out = String::new();
        out.push_str("# Local Browsing History Store\n");
        out.push_str("# Format version 1. No secondary log, journal, or cache.\n\n");

        for entry in &self.entries {
            out.push_str("[[entry]]\n");
            out.push_str(&format!("id = {}\n", entry.entry_id.as_u64()));
            out.push_str(&format!("address = {}\n", escape_string(&entry.address)));
            out.push_str(&format!("title = {}\n", escape_string(&entry.title)));

            let ms = entry
                .visited_at
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            out.push_str(&format!("visited_at_ms = {ms}\n"));

            match &entry.source {
                HistorySource::Navigated => {
                    out.push_str("source = \"navigated\"\n");
                }
                HistorySource::Imported { browser } => {
                    out.push_str("source = \"imported\"\n");
                    out.push_str(&format!("imported_browser = {}\n", escape_string(browser)));
                }
            }
            out.push('\n');
        }

        out
    }

    /// Deserialize entries from toml content.
    fn deserialize(content: &str) -> Result<Vec<HistoryEntry>, HistoryError> {
        let mut entries = Vec::new();

        let mut current_id = None;
        let mut current_address = None;
        let mut current_title = None;
        let mut current_visited_ms = None;
        let mut current_source_str = None;
        let mut current_browser = None;

        let flush_entry = |entries: &mut Vec<HistoryEntry>,
                           id: &mut Option<u64>,
                           addr: &mut Option<String>,
                           title: &mut Option<String>,
                           v_ms: &mut Option<u64>,
                           src_str: &mut Option<String>,
                           browser: &mut Option<String>|
         -> Result<(), HistoryError> {
            if let Some(entry_id) = id.take() {
                let address = addr
                    .take()
                    .ok_or_else(|| HistoryError::InvalidFormat("entry missing address".into()))?;
                let t = title.take().unwrap_or_default();
                let visited_at = match v_ms.take() {
                    Some(ms) => UNIX_EPOCH + Duration::from_millis(ms),
                    None => SystemTime::now(),
                };
                let source = match src_str.take().as_deref() {
                    Some("imported") => HistorySource::Imported {
                        browser: browser.take().unwrap_or_default(),
                    },
                    _ => HistorySource::Navigated,
                };
                *browser = None;

                entries.push(HistoryEntry {
                    entry_id: HistoryEntryId::new(entry_id),
                    address,
                    title: t,
                    visited_at,
                    source,
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

            if line == "[[entry]]" {
                flush_entry(
                    &mut entries,
                    &mut current_id,
                    &mut current_address,
                    &mut current_title,
                    &mut current_visited_ms,
                    &mut current_source_str,
                    &mut current_browser,
                )?;
                continue;
            }

            let Some((k, v)) = line.split_once('=') else {
                return Err(HistoryError::InvalidFormat(format!(
                    "line {line_num}: missing '=' separator"
                )));
            };

            let key = k.trim();
            let val = v.trim();

            match key {
                "id" => {
                    let id_val: u64 = val.parse().map_err(|_| {
                        HistoryError::InvalidFormat(format!("line {line_num}: invalid id integer"))
                    })?;
                    current_id = Some(id_val);
                }
                "address" => {
                    current_address = Some(unescape_string(val, line_num)?);
                }
                "title" => {
                    current_title = Some(unescape_string(val, line_num)?);
                }
                "visited_at_ms" => {
                    let ms: u64 = val.parse().map_err(|_| {
                        HistoryError::InvalidFormat(format!(
                            "line {line_num}: invalid visited_at_ms integer"
                        ))
                    })?;
                    current_visited_ms = Some(ms);
                }
                "source" => {
                    current_source_str = Some(unescape_string(val, line_num)?);
                }
                "imported_browser" => {
                    current_browser = Some(unescape_string(val, line_num)?);
                }
                _ => {
                    // Ignore unrecognized keys for forward compatibility.
                }
            }
        }

        flush_entry(
            &mut entries,
            &mut current_id,
            &mut current_address,
            &mut current_title,
            &mut current_visited_ms,
            &mut current_source_str,
            &mut current_browser,
        )?;

        Ok(entries)
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

fn unescape_string(s: &str, line_num: usize) -> Result<String, HistoryError> {
    let s = s.trim();
    if !s.starts_with('"') || !s.ends_with('"') || s.len() < 2 {
        return Err(HistoryError::InvalidFormat(format!(
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
                    return Err(HistoryError::InvalidFormat(format!(
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
