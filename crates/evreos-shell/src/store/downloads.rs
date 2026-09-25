//! Persistent download store.
//!
//! # Architecture and Invariants
//!
//! - **Local Residence (FR-004, FR-007a, Invariant A)**:
//!   Download records are strictly local. They are never transmitted, synchronised,
//!   or retained off the machine.
//! - **Destination Preservation (FR-004)**:
//!   Every download entry carries its destination path on disk.
//! - **Member Ownership (FR-004)**:
//!   Removing an entry from the download list deletes the record from the store
//!   and never deletes the file the member saved on disk.
//! - **Single Operation Persistence without Undo Logs (FR-004, Invariant A)**:
//!   State changes and removals are written atomically with no undo logs,
//!   journals, or secondary index files with independent lifetimes.

#![forbid(unsafe_code)]

use std::cmp::Reverse;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub use evreos_engine::DownloadId;

static TMP_COUNTER: AtomicU64 = AtomicU64::new(1);

/// The lifecycle state of a download.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DownloadState {
    /// Transfer actively in progress.
    InProgress,
    /// Transfer successfully completed.
    Completed,
    /// Transfer cancelled by member action or browser policy.
    Cancelled,
    /// Transfer failed due to network, disk, or runtime error.
    Failed,
}

impl DownloadState {
    /// Parse a state identifier from its serialized string representation.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "in_progress" => Some(Self::InProgress),
            "completed" => Some(Self::Completed),
            "cancelled" => Some(Self::Cancelled),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }

    /// The string identifier for serialization.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }
}

impl fmt::Display for DownloadState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// An entry in the download store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadEntry {
    pub(crate) id: DownloadId,
    pub(crate) source_address: String,
    pub(crate) destination_path: PathBuf,
    pub(crate) bytes_total: Option<u64>,
    pub(crate) bytes_received: u64,
    pub(crate) state: DownloadState,
    pub(crate) started_at: SystemTime,
    pub(crate) finished_at: Option<SystemTime>,
}

impl DownloadEntry {
    /// Construct a new download entry in the `InProgress` state.
    pub fn new_in_progress(
        id: DownloadId,
        source_address: impl Into<String>,
        destination_path: impl Into<PathBuf>,
        bytes_total: Option<u64>,
        started_at: SystemTime,
    ) -> Self {
        Self {
            id,
            source_address: source_address.into(),
            destination_path: destination_path.into(),
            bytes_total,
            bytes_received: 0,
            state: DownloadState::InProgress,
            started_at,
            finished_at: None,
        }
    }

    /// The unique local identifier assigned to this download.
    pub fn id(&self) -> DownloadId {
        self.id
    }

    /// The origin address or URL from which the download was requested.
    pub fn source_address(&self) -> &str {
        &self.source_address
    }

    /// The destination path on disk where the file is being or was written.
    pub fn destination_path(&self) -> &Path {
        &self.destination_path
    }

    /// The expected total size of the file in bytes, if known by the server.
    pub fn bytes_total(&self) -> Option<u64> {
        self.bytes_total
    }

    /// The number of bytes received so far.
    pub fn bytes_received(&self) -> u64 {
        self.bytes_received
    }

    /// Current lifecycle state of the download.
    pub fn state(&self) -> DownloadState {
        self.state
    }

    /// The timestamp when the download was started.
    pub fn started_at(&self) -> SystemTime {
        self.started_at
    }

    /// The timestamp when the download completed, was cancelled, or failed.
    pub fn finished_at(&self) -> Option<SystemTime> {
        self.finished_at
    }

    /// Whether the download is currently active.
    pub fn is_in_progress(&self) -> bool {
        self.state == DownloadState::InProgress
    }

    /// Whether the download finished successfully.
    pub fn is_completed(&self) -> bool {
        self.state == DownloadState::Completed
    }

    /// Whether the download was cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.state == DownloadState::Cancelled
    }

    /// Whether the download failed.
    pub fn is_failed(&self) -> bool {
        self.state == DownloadState::Failed
    }
}

/// Errors that may occur when interacting with the download store.
#[derive(Debug)]
pub enum DownloadError {
    /// The specified download identifier was not found in the store.
    NotFound(DownloadId),
    /// A download with the specified identifier already exists.
    AlreadyExists(DownloadId),
    /// The download has already completed, failed, or been cancelled.
    AlreadyFinished(DownloadId),
    /// An illegal lifecycle transition was attempted.
    InvalidStateTransition {
        /// The affected download identifier.
        id: DownloadId,
        /// Current state.
        current: DownloadState,
        /// Attempted invalid state.
        attempted: DownloadState,
    },
    /// An underlying filesystem I/O error occurred.
    Io(io::Error),
    /// The persisted file could not be parsed.
    InvalidFormat(String),
}

impl fmt::Display for DownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "download record not found: {id}"),
            Self::AlreadyExists(id) => write!(f, "download already exists: {id}"),
            Self::AlreadyFinished(id) => write!(f, "download has already finished: {id}"),
            Self::InvalidStateTransition {
                id,
                current,
                attempted,
            } => {
                write!(
                    f,
                    "invalid state transition for download {id}: cannot transition from {current} to {attempted}"
                )
            }
            Self::Io(err) => write!(f, "download store I/O error: {err}"),
            Self::InvalidFormat(msg) => write!(f, "invalid download store format: {msg}"),
        }
    }
}

impl std::error::Error for DownloadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for DownloadError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

/// Persistent store tracking member downloads.
#[derive(Debug, Clone)]
pub struct DownloadStore {
    root: PathBuf,
    entries: Vec<DownloadEntry>,
    next_id: u64,
}

impl DownloadStore {
    /// The canonical file name on disk.
    pub const FILE_NAME: &'static str = "downloads.toml";

    /// Open or initialise the download store under the given profile root.
    pub fn open(profile_root: &Path) -> Self {
        let root = profile_root.to_path_buf();
        let target_path = root.join(Self::FILE_NAME);

        let entries = if target_path.exists() {
            match fs::read_to_string(&target_path) {
                Ok(content) => match Self::deserialize(&content) {
                    Ok(parsed) => parsed,
                    Err(e) => {
                        eprintln!("evreos: warning: corrupt downloads.toml ({e}); starting empty");
                        Vec::new()
                    }
                },
                Err(e) => {
                    eprintln!(
                        "evreos: warning: could not read downloads.toml ({e}); starting empty"
                    );
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

        let max_id = entries.iter().map(|e| e.id.as_u64()).max().unwrap_or(0);

        Self {
            root,
            entries,
            next_id: max_id.saturating_add(1),
        }
    }

    /// Construct an in-memory download store with no disk persistence.
    pub fn in_memory() -> Self {
        Self {
            root: PathBuf::new(),
            entries: Vec::new(),
            next_id: 1,
        }
    }

    /// Allocate the next available sequential download identifier.
    pub fn allocate_id(&mut self) -> DownloadId {
        let id = DownloadId::new(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// Record a newly initiated download in the `InProgress` state.
    ///
    /// The entry is immediately persisted to `downloads.toml` with no undo log.
    pub fn start_download(
        &mut self,
        id: DownloadId,
        source_address: impl Into<String>,
        destination_path: impl Into<PathBuf>,
        bytes_total: Option<u64>,
    ) -> Result<&DownloadEntry, DownloadError> {
        if self.entries.iter().any(|e| e.id == id) {
            return Err(DownloadError::AlreadyExists(id));
        }

        if id.as_u64() >= self.next_id {
            self.next_id = id.as_u64().saturating_add(1);
        }

        let entry = DownloadEntry::new_in_progress(
            id,
            source_address,
            destination_path,
            bytes_total,
            SystemTime::now(),
        );

        self.entries.push(entry);
        self.save_to_disk()?;

        Ok(self.entries.last().expect("just pushed"))
    }

    /// Update the byte transfer progress for an active download.
    pub fn update_progress(
        &mut self,
        id: DownloadId,
        bytes_received: u64,
    ) -> Result<(), DownloadError> {
        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or(DownloadError::NotFound(id))?;

        if !entry.is_in_progress() {
            return Err(DownloadError::InvalidStateTransition {
                id,
                current: entry.state,
                attempted: DownloadState::InProgress,
            });
        }

        entry.bytes_received = bytes_received;
        self.save_to_disk()?;
        Ok(())
    }

    /// Mark an active download as successfully completed.
    pub fn complete_download(&mut self, id: DownloadId) -> Result<(), DownloadError> {
        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or(DownloadError::NotFound(id))?;

        if !entry.is_in_progress() {
            return Err(DownloadError::InvalidStateTransition {
                id,
                current: entry.state,
                attempted: DownloadState::Completed,
            });
        }

        entry.state = DownloadState::Completed;
        entry.finished_at = Some(SystemTime::now());
        if let Some(total) = entry.bytes_total {
            if entry.bytes_received < total {
                entry.bytes_received = total;
            }
        }

        self.save_to_disk()?;
        Ok(())
    }

    /// Cancel an in-progress download by member action.
    ///
    /// If the download has already completed, failed, or been cancelled, returns
    /// `DownloadError::AlreadyFinished`.
    pub fn cancel_download(&mut self, id: DownloadId) -> Result<(), DownloadError> {
        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or(DownloadError::NotFound(id))?;

        if !entry.is_in_progress() {
            return Err(DownloadError::AlreadyFinished(id));
        }

        entry.state = DownloadState::Cancelled;
        entry.finished_at = Some(SystemTime::now());

        self.save_to_disk()?;
        Ok(())
    }

    /// Mark an in-progress download as failed.
    pub fn fail_download(&mut self, id: DownloadId) -> Result<(), DownloadError> {
        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or(DownloadError::NotFound(id))?;

        if !entry.is_in_progress() {
            return Err(DownloadError::AlreadyFinished(id));
        }

        entry.state = DownloadState::Failed;
        entry.finished_at = Some(SystemTime::now());

        self.save_to_disk()?;
        Ok(())
    }

    /// Remove a download record from the list.
    ///
    /// **Important (FR-004)**: This deletes ONLY the metadata record from the store
    /// and NEVER deletes the file the member saved on disk.
    pub fn remove_from_list(&mut self, id: DownloadId) -> Result<bool, DownloadError> {
        let initial_len = self.entries.len();
        self.entries.retain(|e| e.id != id);

        if self.entries.len() != initial_len {
            self.save_to_disk()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Remove all finished downloads (completed, cancelled, or failed) from the list.
    ///
    /// Does not delete any saved files from disk.
    pub fn clear_finished(&mut self) -> Result<usize, DownloadError> {
        let initial_len = self.entries.len();
        self.entries.retain(|e| e.is_in_progress());

        let removed = initial_len - self.entries.len();
        if removed > 0 {
            self.save_to_disk()?;
        }
        Ok(removed)
    }

    /// Retrieve a download entry by its identifier.
    pub fn get(&self, id: DownloadId) -> Option<&DownloadEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// All download entries currently in the store.
    pub fn entries(&self) -> &[DownloadEntry] {
        &self.entries
    }

    /// Entries currently in progress.
    pub fn in_progress(&self) -> Vec<&DownloadEntry> {
        self.entries.iter().filter(|e| e.is_in_progress()).collect()
    }

    /// Entries that completed successfully.
    pub fn completed(&self) -> Vec<&DownloadEntry> {
        self.entries.iter().filter(|e| e.is_completed()).collect()
    }

    /// Returns all download entries sorted reverse-chronologically (newest first).
    pub fn list_ordered(&self) -> Vec<DownloadEntry> {
        let mut list = self.entries.clone();
        list.sort_by_key(|e| (Reverse(e.started_at), Reverse(e.id.as_u64())));
        list
    }

    /// Count of entries held in the store.
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Whether the store holds zero download entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The filesystem path to the canonical `downloads.toml` file.
    pub fn file_path(&self) -> PathBuf {
        self.root.join(Self::FILE_NAME)
    }

    /// Atomically persist current entries to disk.
    fn save_to_disk(&self) -> Result<(), DownloadError> {
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
            return Err(DownloadError::Io(e));
        }

        Ok(())
    }

    /// Serialize all downloads into TOML format.
    fn serialize(&self) -> String {
        let mut out = String::new();
        out.push_str("# Local Download Store\n");
        out.push_str("# Format version 1. No secondary log, journal, or cache.\n\n");

        for entry in &self.entries {
            out.push_str("[[download]]\n");
            out.push_str(&format!("id = {}\n", entry.id.as_u64()));
            out.push_str(&format!(
                "source_address = {}\n",
                escape_string(&entry.source_address)
            ));
            out.push_str(&format!(
                "destination_path = {}\n",
                escape_string(&entry.destination_path.to_string_lossy())
            ));

            if let Some(total) = entry.bytes_total {
                out.push_str(&format!("bytes_total = {total}\n"));
            }

            out.push_str(&format!("bytes_received = {}\n", entry.bytes_received));
            out.push_str(&format!("state = \"{}\"\n", entry.state.as_str()));

            let started_ms = entry
                .started_at
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            out.push_str(&format!("started_at_ms = {started_ms}\n"));

            if let Some(fin) = entry.finished_at {
                let finished_ms = fin
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                out.push_str(&format!("finished_at_ms = {finished_ms}\n"));
            }

            out.push('\n');
        }

        out
    }

    /// Deserialize downloads from TOML format.
    fn deserialize(content: &str) -> Result<Vec<DownloadEntry>, DownloadError> {
        let mut entries = Vec::new();

        let mut current_id = None;
        let mut current_source = None;
        let mut current_dest = None;
        let mut current_total = None;
        let mut current_received = None;
        let mut current_state = None;
        let mut current_started_ms = None;
        let mut current_finished_ms = None;

        let flush_entry = |entries: &mut Vec<DownloadEntry>,
                           id: &mut Option<u64>,
                           source: &mut Option<String>,
                           dest: &mut Option<String>,
                           total: &mut Option<u64>,
                           received: &mut Option<u64>,
                           state: &mut Option<DownloadState>,
                           started_ms: &mut Option<u64>,
                           finished_ms: &mut Option<u64>|
         -> Result<(), DownloadError> {
            if let Some(raw_id) = id.take() {
                let src = source
                    .take()
                    .ok_or_else(|| DownloadError::InvalidFormat("missing source_address".into()))?;
                let dst = dest.take().ok_or_else(|| {
                    DownloadError::InvalidFormat("missing destination_path".into())
                })?;
                let st = state.take().unwrap_or(DownloadState::InProgress);
                let rec = received.take().unwrap_or(0);
                let tot = total.take();

                let started_at = match started_ms.take() {
                    Some(ms) => UNIX_EPOCH + Duration::from_millis(ms),
                    None => SystemTime::now(),
                };

                let finished_at = finished_ms
                    .take()
                    .map(|ms| UNIX_EPOCH + Duration::from_millis(ms));

                entries.push(DownloadEntry {
                    id: DownloadId::new(raw_id),
                    source_address: src,
                    destination_path: PathBuf::from(dst),
                    bytes_total: tot,
                    bytes_received: rec,
                    state: st,
                    started_at,
                    finished_at,
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

            if line == "[[download]]" {
                flush_entry(
                    &mut entries,
                    &mut current_id,
                    &mut current_source,
                    &mut current_dest,
                    &mut current_total,
                    &mut current_received,
                    &mut current_state,
                    &mut current_started_ms,
                    &mut current_finished_ms,
                )?;
                continue;
            }

            let Some((key, val)) = line.split_once('=') else {
                continue;
            };

            let key = key.trim();
            let val = val.trim();

            match key {
                "id" => {
                    let parsed: u64 = val.parse().map_err(|_| {
                        DownloadError::InvalidFormat(format!("line {line_num}: invalid id"))
                    })?;
                    current_id = Some(parsed);
                }
                "source_address" => {
                    current_source = Some(unescape_string(val, line_num)?);
                }
                "destination_path" => {
                    current_dest = Some(unescape_string(val, line_num)?);
                }
                "bytes_total" => {
                    let total: u64 = val.parse().map_err(|_| {
                        DownloadError::InvalidFormat(format!(
                            "line {line_num}: invalid bytes_total"
                        ))
                    })?;
                    current_total = Some(total);
                }
                "bytes_received" => {
                    let received: u64 = val.parse().map_err(|_| {
                        DownloadError::InvalidFormat(format!(
                            "line {line_num}: invalid bytes_received"
                        ))
                    })?;
                    current_received = Some(received);
                }
                "state" => {
                    let unescaped = unescape_string(val, line_num)?;
                    let st = DownloadState::parse(&unescaped).ok_or_else(|| {
                        DownloadError::InvalidFormat(format!(
                            "line {line_num}: unknown download state {unescaped}"
                        ))
                    })?;
                    current_state = Some(st);
                }
                "started_at_ms" => {
                    let ms: u64 = val.parse().map_err(|_| {
                        DownloadError::InvalidFormat(format!(
                            "line {line_num}: invalid started_at_ms"
                        ))
                    })?;
                    current_started_ms = Some(ms);
                }
                "finished_at_ms" => {
                    let ms: u64 = val.parse().map_err(|_| {
                        DownloadError::InvalidFormat(format!(
                            "line {line_num}: invalid finished_at_ms"
                        ))
                    })?;
                    current_finished_ms = Some(ms);
                }
                _ => {}
            }
        }

        flush_entry(
            &mut entries,
            &mut current_id,
            &mut current_source,
            &mut current_dest,
            &mut current_total,
            &mut current_received,
            &mut current_state,
            &mut current_started_ms,
            &mut current_finished_ms,
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

fn unescape_string(s: &str, line_num: usize) -> Result<String, DownloadError> {
    if !s.starts_with('"') || !s.ends_with('"') || s.len() < 2 {
        return Err(DownloadError::InvalidFormat(format!(
            "line {line_num}: string value must be quoted"
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
                    return Err(DownloadError::InvalidFormat(format!(
                        "line {line_num}: unsupported escape sequence \\{other}"
                    )));
                }
                None => {
                    return Err(DownloadError::InvalidFormat(format!(
                        "line {line_num}: dangling escape backslash"
                    )));
                }
            }
        } else {
            out.push(ch);
        }
    }

    Ok(out)
}
