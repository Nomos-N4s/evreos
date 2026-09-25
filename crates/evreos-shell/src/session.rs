//! Tab session store for the Evreos shell.
//!
//! Under FR-001, FR-007, SC-002, and SC-004:
//! - Persists the open tabs, ordering, and active tab across browser restarts.
//! - Written atomically as a temporary file plus rename so crashes cannot truncate it into partial loss.
//! - Restores tab identity, order, and address as [`TabLifecycle::RestoredNotLoaded`],
//!   deferring page loading until first activation.
//! - Enforces write-path exclusion: a private window is NEVER written to the session file
//!   at any point in its lifetime (FR-007).
//! - Holds NO account state (FR-021 account sessions are separate entities).

#![forbid(unsafe_code)]

use std::fmt;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::app::mint_window_id;
use crate::store::WindowKind;
use crate::tabs::WindowTabs;

/// An error occurring during session serialization, saving, or loading.
#[derive(Debug)]
pub enum SessionError {
    /// File I/O failure.
    Io(io::Error),
    /// Session file parsing or deserialization failure.
    ParseError(String),
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "session I/O error: {err}"),
            Self::ParseError(msg) => write!(f, "session parse error: {msg}"),
        }
    }
}

impl std::error::Error for SessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::ParseError(_) => None,
        }
    }
}

impl From<io::Error> for SessionError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

/// A serialized tab entry within a stored session window.
///
/// Under FR-001 and data-model §1.5: holds address, title, position, and active state.
/// Holds no account state, authentication tokens, or credentials (FR-021, FR-023).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionTab {
    /// The displayed address of the tab.
    pub address: String,
    /// Page title of the tab.
    pub title: String,
    /// Tab position within the window's tab bar.
    pub position: usize,
    /// Whether this tab was the active tab in its window.
    pub active: bool,
}

impl SessionTab {
    /// Create a new session tab entry.
    pub fn new(
        address: impl Into<String>,
        title: impl Into<String>,
        position: usize,
        active: bool,
    ) -> Self {
        Self {
            address: address.into(),
            title: title.into(),
            position,
            active,
        }
    }
}

/// A serialized window entry within a stored session.
///
/// Normal windows only (FR-007).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionWindow {
    /// Ordered list of tabs in this window.
    pub tabs: Vec<SessionTab>,
}

impl SessionWindow {
    /// Create a new session window containing `tabs`.
    pub fn new(mut tabs: Vec<SessionTab>) -> Self {
        tabs.sort_by_key(|t| t.position);
        Self { tabs }
    }
}

/// Complete in-memory snapshot of open windows and tabs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSnapshot {
    /// Unix timestamp in milliseconds when this session was saved.
    pub saved_at_epoch_ms: u64,
    /// Open normal windows in the session.
    pub windows: Vec<SessionWindow>,
}

impl SessionSnapshot {
    /// Create a new session snapshot.
    pub fn new(windows: Vec<SessionWindow>) -> Self {
        let saved_at_epoch_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        Self {
            saved_at_epoch_ms,
            windows,
        }
    }
}

/// Serialize a [`SessionSnapshot`] into TOML format.
pub fn serialize_session(snapshot: &SessionSnapshot) -> String {
    let mut out = String::new();
    out.push_str("# Evreos Tab Session Store (FR-001)\n");
    out.push_str(&format!(
        "saved_at_epoch_ms = {}\n\n",
        snapshot.saved_at_epoch_ms
    ));

    for window in &snapshot.windows {
        out.push_str("[[windows]]\n");
        for tab in &window.tabs {
            out.push_str("  [[windows.tabs]]\n");
            out.push_str(&format!("  address = {:?}\n", tab.address));
            out.push_str(&format!("  title = {:?}\n", tab.title));
            out.push_str(&format!("  position = {}\n", tab.position));
            out.push_str(&format!("  active = {}\n\n", tab.active));
        }
    }

    out
}

/// Parse a serialized TOML session string into a [`SessionSnapshot`].
pub fn parse_session_file(content: &str) -> Result<SessionSnapshot, SessionError> {
    let mut saved_at_epoch_ms = 0;
    let mut windows: Vec<SessionWindow> = Vec::new();
    let mut current_window_tabs: Option<Vec<SessionTab>> = None;
    let mut current_tab_builder: Option<TabBuilder> = None;

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line == "[[windows]]" {
            if let Some(tb) = current_tab_builder.take() {
                if let Some(tab) = tb.build() {
                    current_window_tabs.get_or_insert_with(Vec::new).push(tab);
                }
            }
            if let Some(tabs) = current_window_tabs.take() {
                windows.push(SessionWindow::new(tabs));
            }
            current_window_tabs = Some(Vec::new());
            continue;
        }

        if line == "[[windows.tabs]]" {
            if let Some(tb) = current_tab_builder.take() {
                if let Some(tab) = tb.build() {
                    current_window_tabs.get_or_insert_with(Vec::new).push(tab);
                }
            }
            current_tab_builder = Some(TabBuilder::default());
            continue;
        }

        if let Some((key, val)) = line.split_once('=') {
            let k = key.trim();
            let v = val.trim();

            if k == "saved_at_epoch_ms" {
                saved_at_epoch_ms = v.parse::<u64>().unwrap_or(0);
            } else if let Some(ref mut tb) = current_tab_builder {
                match k {
                    "address" => {
                        tb.address = Some(unescape_toml_str(v));
                    }
                    "title" => {
                        tb.title = Some(unescape_toml_str(v));
                    }
                    "position" => {
                        tb.position = v.parse::<usize>().ok();
                    }
                    "active" => {
                        tb.active = v.parse::<bool>().ok();
                    }
                    _ => {}
                }
            }
        }
    }

    if let Some(tb) = current_tab_builder.take() {
        if let Some(tab) = tb.build() {
            current_window_tabs.get_or_insert_with(Vec::new).push(tab);
        }
    }
    if let Some(tabs) = current_window_tabs.take() {
        windows.push(SessionWindow::new(tabs));
    }

    Ok(SessionSnapshot {
        saved_at_epoch_ms,
        windows,
    })
}

/// Manages the persistent tab session file under the profile root.
///
/// Invariants:
/// - Atomic write via temp file + rename to prevent corruption or partial loss on crash.
/// - Write-path exclusion of private windows (FR-007): private windows and tabs are never
///   written to disk under any circumstances.
/// - Deferral of page loads on restoration: restored tabs are given [`TabLifecycle::RestoredNotLoaded`].
/// - No account state is ever held or written (FR-021, FR-023).
#[derive(Debug, Clone)]
pub struct SessionStore {
    root_path: PathBuf,
}

impl SessionStore {
    /// Open the session store under the given profile root directory.
    pub fn open(profile_root: impl Into<PathBuf>) -> Self {
        Self {
            root_path: profile_root.into(),
        }
    }

    /// Return the session file path (`<root>/session.toml`).
    pub fn file_path(&self) -> PathBuf {
        self.root_path.join("session.toml")
    }

    /// Save the open window tab states atomically to disk.
    ///
    /// # Write-Path Private Window Exclusion (FR-007)
    /// Windows of kind [`WindowKind::Private`] are filtered out and completely omitted
    /// from serialization. No trace of private windows or tabs enters the written session.
    pub fn save(&self, windows: &[&WindowTabs]) -> Result<(), SessionError> {
        let mut session_windows = Vec::new();

        for win in windows {
            // Write-path exclusion: Private windows MUST NEVER appear in this store
            if win.kind() != WindowKind::Normal {
                continue;
            }

            let mut tabs = Vec::new();
            for tab in win.tabs() {
                let active = win.active_tab_id() == Some(tab.id());
                tabs.push(SessionTab::new(
                    tab.displayed_address(),
                    tab.title(),
                    tab.position(),
                    active,
                ));
            }

            session_windows.push(SessionWindow::new(tabs));
        }

        let snapshot = SessionSnapshot::new(session_windows);
        self.save_snapshot(&snapshot)
    }

    /// Atomically write a [`SessionSnapshot`] to disk.
    pub fn save_snapshot(&self, snapshot: &SessionSnapshot) -> Result<(), SessionError> {
        if !self.root_path.exists() {
            fs::create_dir_all(&self.root_path)?;
        }

        let serialized = serialize_session(snapshot);
        let target_path = self.file_path();
        static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);
        let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let tmp_path = self
            .root_path
            .join(format!(".session.toml.{pid}_{seq}.tmp"));

        fs::write(&tmp_path, serialized.as_bytes())?;
        fs::rename(&tmp_path, &target_path)?;

        Ok(())
    }

    /// Load the stored session snapshot from disk, if present.
    pub fn load(&self) -> Result<Option<SessionSnapshot>, SessionError> {
        let path = self.file_path();
        if !path.is_file() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path)?;
        let snapshot = parse_session_file(&content)?;
        Ok(Some(snapshot))
    }

    /// Restore a [`SessionSnapshot`] into a vector of [`WindowTabs`].
    ///
    /// Every restored tab is placed in [`TabLifecycle::RestoredNotLoaded`],
    /// with page loading deferred until first activation (FR-001, SC-002).
    pub fn restore_into(&self, snapshot: &SessionSnapshot) -> Vec<WindowTabs> {
        let mut result = Vec::new();

        for win in &snapshot.windows {
            let win_id = mint_window_id();
            let mut window_tabs = WindowTabs::new(win_id, WindowKind::Normal);

            for tab in &win.tabs {
                window_tabs.restore_tab(&tab.address, &tab.title, tab.position, tab.active);
            }

            result.push(window_tabs);
        }

        result
    }

    /// Clear the stored session file from disk.
    pub fn clear(&self) -> Result<(), SessionError> {
        let path = self.file_path();
        if path.is_file() {
            fs::remove_file(path)?;
        }
        Ok(())
    }
}

#[derive(Default)]
struct TabBuilder {
    address: Option<String>,
    title: Option<String>,
    position: Option<usize>,
    active: Option<bool>,
}

impl TabBuilder {
    fn build(self) -> Option<SessionTab> {
        let address = self.address?;
        let title = self.title.unwrap_or_default();
        let position = self.position.unwrap_or(0);
        let active = self.active.unwrap_or(false);
        Some(SessionTab {
            address,
            title,
            position,
            active,
        })
    }
}

fn unescape_toml_str(val: &str) -> String {
    let s = val.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        let inner = &s[1..s.len() - 1];
        inner
            .replace("\\\\", "\\")
            .replace("\\\"", "\"")
            .replace("\\n", "\n")
            .replace("\\r", "\r")
            .replace("\\t", "\t")
    } else {
        s.to_string()
    }
}
