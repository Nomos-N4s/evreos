//! Keyboard action map for the Evreos shell chrome.
//!
//! Under FR-011 and WCAG 2.1.2:
//! - Every action reachable by pointer across the browser chrome MUST be reachable by keyboard.
//! - The chrome command table enumerates every action the chrome exposes.
//! - An escape path out of focus held by page content is guaranteed and does not depend
//!   on `Ctrl` or `Alt` being held (e.g. `F6` to cycle/focus address bar, `Escape` to break
//!   focus traps back to chrome). This is essential because platform web views (such as
//!   WebView2 on Windows) document accelerator key events as raised only when `Ctrl` or `Alt`
//!   is held, so non-accelerator keys (`F6`, `Escape`, `Tab`) need a dedicated escape path.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

/// High-level categories of chrome commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandCategory {
    /// Navigation commands (address bar, back, forward, reload, stop, home).
    Navigation,
    /// Tab lifecycle and selection.
    Tabs,
    /// Window creation and destruction.
    Windows,
    /// Page view, zoom, and display presentation.
    View,
    /// In-page actions (find, print, bookmarking, downloads, history).
    PageActions,
    /// Dialogs, settings, permissions, and extension panels.
    PanesAndDialogs,
    /// Focus navigation and accessibility trap escapes.
    FocusAndAccessibility,
}

/// Commands exposed across the browser chrome.
///
/// Under FR-011, every command that is reachable by pointer (buttons, tabs,
/// menu items, toolbar icons, address bar) MUST carry at least one keyboard binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ChromeCommand {
    // Navigation
    /// Move focus to the address bar / omnibox (FR-003).
    FocusAddressBar,
    /// Navigate back in tab session history (FR-001).
    NavigateBack,
    /// Navigate forward in tab session history (FR-001).
    NavigateForward,
    /// Reload the active tab's current document.
    Reload,
    /// Reload the active tab's current document bypassing local cache.
    ReloadBypassingCache,
    /// Stop loading the current document.
    StopLoading,
    /// Navigate to the member's configured home page or super-app home (FR-016).
    NavigateHome,

    // Tabs
    /// Open a new tab in the active window (FR-001).
    NewTab,
    /// Close the active tab (FR-001).
    CloseTab,
    /// Reopen the most recently closed tab.
    ReopenClosedTab,
    /// Switch to the next tab in the window tab strip.
    NextTab,
    /// Switch to the previous tab in the window tab strip.
    PreviousTab,
    /// Select tab by 1-based index (1 through 8).
    SelectTab(u8),
    /// Select the last tab in the active window.
    SelectLastTab,
    /// Duplicate the active tab.
    DuplicateTab,
    /// Pin or unpin the active tab.
    TogglePinTab,
    /// Mute or unmute audio playback in the active tab.
    ToggleMuteTab,

    // Windows
    /// Open a new standard browsing window.
    NewWindow,
    /// Open a new private browsing window (FR-006).
    NewPrivateWindow,
    /// Close the active window.
    CloseWindow,

    // View & Zoom
    /// Increase interface and page zoom scale (FR-005).
    ZoomIn,
    /// Decrease interface and page zoom scale (FR-005).
    ZoomOut,
    /// Reset zoom scale to 100% default (FR-005).
    ZoomReset,
    /// Toggle window fullscreen presentation.
    ToggleFullscreen,

    // Page Actions
    /// Open find-in-page search field (FR-005).
    FindInPage,
    /// Jump to next find-in-page match (FR-005).
    FindNext,
    /// Jump to previous find-in-page match (FR-005).
    FindPrevious,
    /// Bookmark the active page (FR-003).
    BookmarkPage,
    /// Open the bookmarks management surface or panel (FR-003).
    ShowBookmarks,
    /// Open browsing history view (FR-004).
    ShowHistory,
    /// Open downloads management surface.
    ShowDownloads,
    /// Print the active document or export to PDF (FR-009).
    Print,

    // Panes, Dialogs & Settings
    /// Open the primary shell application menu.
    OpenMenu,
    /// Open member preferences / settings surface.
    OpenSettings,
    /// Open site permissions and security info for the active site (FR-008).
    OpenSitePermissions,
    /// Toggle tracker and advert content blocking for the active site (FR-008).
    ToggleContentBlocking,
    /// Offer or trigger hand-off to the external browser (FR-015a, FR-037).
    HandOffCurrentSite,
    /// Open dialog to clear browsing history, cookies, and cache.
    ClearBrowsingData,
    /// Toggle light / dark presentation theme (FR-010).
    ToggleTheme,

    // Focus & Accessibility
    /// Escape focus held by page content or modal trap back to chrome (FR-011, WCAG 2.1.2).
    EscapeFocusTrap,
    /// Cycle keyboard focus forward through chrome elements and content (FR-011).
    CycleFocusForward,
    /// Cycle keyboard focus backward through chrome elements and content (FR-011).
    CycleFocusBackward,
    /// Transfer keyboard focus from chrome into page content.
    FocusContent,
}

impl ChromeCommand {
    /// Return all commands defined in the chrome's command table.
    pub fn all() -> Vec<Self> {
        let mut commands = vec![
            Self::FocusAddressBar,
            Self::NavigateBack,
            Self::NavigateForward,
            Self::Reload,
            Self::ReloadBypassingCache,
            Self::StopLoading,
            Self::NavigateHome,
            Self::NewTab,
            Self::CloseTab,
            Self::ReopenClosedTab,
            Self::NextTab,
            Self::PreviousTab,
        ];
        for tab_idx in 1..=8 {
            commands.push(Self::SelectTab(tab_idx));
        }
        commands.extend_from_slice(&[
            Self::SelectLastTab,
            Self::DuplicateTab,
            Self::TogglePinTab,
            Self::ToggleMuteTab,
            Self::NewWindow,
            Self::NewPrivateWindow,
            Self::CloseWindow,
            Self::ZoomIn,
            Self::ZoomOut,
            Self::ZoomReset,
            Self::ToggleFullscreen,
            Self::FindInPage,
            Self::FindNext,
            Self::FindPrevious,
            Self::BookmarkPage,
            Self::ShowBookmarks,
            Self::ShowHistory,
            Self::ShowDownloads,
            Self::Print,
            Self::OpenMenu,
            Self::OpenSettings,
            Self::OpenSitePermissions,
            Self::ToggleContentBlocking,
            Self::HandOffCurrentSite,
            Self::ClearBrowsingData,
            Self::ToggleTheme,
            Self::EscapeFocusTrap,
            Self::CycleFocusForward,
            Self::CycleFocusBackward,
            Self::FocusContent,
        ]);
        commands
    }

    /// Whether this action is reachable by pointer interaction (clicks, gestures, menu items).
    ///
    /// Under FR-011: "Every action reachable by pointer MUST be reachable by keyboard."
    /// Every pointer-reachable command MUST carry at least one keyboard shortcut binding.
    pub fn is_pointer_reachable(&self) -> bool {
        // Every command defined in the chrome corresponds to a pointer-reachable control:
        // toolbar buttons, omnibox, tab strip tabs/buttons, context menus, app menu items,
        // page zoom buttons, find-in-page bar, or viewport focus clicks.
        true
    }

    /// Human-readable command name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::FocusAddressBar => "Focus Address Bar",
            Self::NavigateBack => "Back",
            Self::NavigateForward => "Forward",
            Self::Reload => "Reload",
            Self::ReloadBypassingCache => "Reload Bypassing Cache",
            Self::StopLoading => "Stop",
            Self::NavigateHome => "Home",
            Self::NewTab => "New Tab",
            Self::CloseTab => "Close Tab",
            Self::ReopenClosedTab => "Reopen Closed Tab",
            Self::NextTab => "Next Tab",
            Self::PreviousTab => "Previous Tab",
            Self::SelectTab(idx) => match idx {
                1 => "Select Tab 1",
                2 => "Select Tab 2",
                3 => "Select Tab 3",
                4 => "Select Tab 4",
                5 => "Select Tab 5",
                6 => "Select Tab 6",
                7 => "Select Tab 7",
                8 => "Select Tab 8",
                _ => "Select Tab",
            },
            Self::SelectLastTab => "Select Last Tab",
            Self::DuplicateTab => "Duplicate Tab",
            Self::TogglePinTab => "Pin / Unpin Tab",
            Self::ToggleMuteTab => "Mute / Unmute Tab",
            Self::NewWindow => "New Window",
            Self::NewPrivateWindow => "New Private Window",
            Self::CloseWindow => "Close Window",
            Self::ZoomIn => "Zoom In",
            Self::ZoomOut => "Zoom Out",
            Self::ZoomReset => "Reset Zoom",
            Self::ToggleFullscreen => "Toggle Fullscreen",
            Self::FindInPage => "Find in Page",
            Self::FindNext => "Find Next",
            Self::FindPrevious => "Find Previous",
            Self::BookmarkPage => "Bookmark Page",
            Self::ShowBookmarks => "Show Bookmarks",
            Self::ShowHistory => "Show History",
            Self::ShowDownloads => "Show Downloads",
            Self::Print => "Print",
            Self::OpenMenu => "Open Menu",
            Self::OpenSettings => "Settings",
            Self::OpenSitePermissions => "Site Permissions",
            Self::ToggleContentBlocking => "Toggle Content Blocking",
            Self::HandOffCurrentSite => "Hand-off to External Browser",
            Self::ClearBrowsingData => "Clear Browsing Data",
            Self::ToggleTheme => "Toggle Theme",
            Self::EscapeFocusTrap => "Escape Focus Trap",
            Self::CycleFocusForward => "Cycle Focus Forward",
            Self::CycleFocusBackward => "Cycle Focus Backward",
            Self::FocusContent => "Focus Page Content",
        }
    }

    /// Functional category for this command.
    pub fn category(&self) -> CommandCategory {
        match self {
            Self::FocusAddressBar
            | Self::NavigateBack
            | Self::NavigateForward
            | Self::Reload
            | Self::ReloadBypassingCache
            | Self::StopLoading
            | Self::NavigateHome => CommandCategory::Navigation,

            Self::NewTab
            | Self::CloseTab
            | Self::ReopenClosedTab
            | Self::NextTab
            | Self::PreviousTab
            | Self::SelectTab(_)
            | Self::SelectLastTab
            | Self::DuplicateTab
            | Self::TogglePinTab
            | Self::ToggleMuteTab => CommandCategory::Tabs,

            Self::NewWindow | Self::NewPrivateWindow | Self::CloseWindow => {
                CommandCategory::Windows
            }

            Self::ZoomIn | Self::ZoomOut | Self::ZoomReset | Self::ToggleFullscreen => {
                CommandCategory::View
            }

            Self::FindInPage
            | Self::FindNext
            | Self::FindPrevious
            | Self::BookmarkPage
            | Self::ShowBookmarks
            | Self::ShowHistory
            | Self::ShowDownloads
            | Self::Print => CommandCategory::PageActions,

            Self::OpenMenu
            | Self::OpenSettings
            | Self::OpenSitePermissions
            | Self::ToggleContentBlocking
            | Self::HandOffCurrentSite
            | Self::ClearBrowsingData
            | Self::ToggleTheme => CommandCategory::PanesAndDialogs,

            Self::EscapeFocusTrap
            | Self::CycleFocusForward
            | Self::CycleFocusBackward
            | Self::FocusContent => CommandCategory::FocusAndAccessibility,
        }
    }
}

impl fmt::Display for ChromeCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Keyboard key representation independent of platform scan codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Key {
    /// Alphanumeric or punctuation character (normalized to lowercase).
    Char(char),
    /// Function keys F1 through F12.
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    /// Escape key (crucial focus escape key).
    Escape,
    /// Tab key (focus traversal).
    Tab,
    /// Enter / Return key.
    Enter,
    /// Backspace key.
    Backspace,
    /// Delete key.
    Delete,
    /// Insert key.
    Insert,
    /// Home key.
    Home,
    /// End key.
    End,
    /// Page Up key.
    PageUp,
    /// Page Down key.
    PageDown,
    /// Arrow Left key.
    ArrowLeft,
    /// Arrow Right key.
    ArrowRight,
    /// Arrow Up key.
    ArrowUp,
    /// Arrow Down key.
    ArrowDown,
    /// Spacebar key.
    Space,
}

impl Key {
    /// Create a character key, normalizing ASCII characters to lowercase.
    pub fn from_char(c: char) -> Self {
        Self::Char(c.to_ascii_lowercase())
    }

    /// Try to construct a [`Key`] from a `winit::keyboard::Key`.
    pub fn from_winit_key(key: &winit::keyboard::Key) -> Option<Self> {
        use winit::keyboard::{Key as WKey, NamedKey};
        match key {
            WKey::Named(named) => match named {
                NamedKey::Escape => Some(Self::Escape),
                NamedKey::Tab => Some(Self::Tab),
                NamedKey::Enter => Some(Self::Enter),
                NamedKey::Backspace => Some(Self::Backspace),
                NamedKey::Delete => Some(Self::Delete),
                NamedKey::Insert => Some(Self::Insert),
                NamedKey::Home => Some(Self::Home),
                NamedKey::End => Some(Self::End),
                NamedKey::PageUp => Some(Self::PageUp),
                NamedKey::PageDown => Some(Self::PageDown),
                NamedKey::ArrowLeft => Some(Self::ArrowLeft),
                NamedKey::ArrowRight => Some(Self::ArrowRight),
                NamedKey::ArrowUp => Some(Self::ArrowUp),
                NamedKey::ArrowDown => Some(Self::ArrowDown),
                NamedKey::Space => Some(Self::Space),
                NamedKey::F1 => Some(Self::F1),
                NamedKey::F2 => Some(Self::F2),
                NamedKey::F3 => Some(Self::F3),
                NamedKey::F4 => Some(Self::F4),
                NamedKey::F5 => Some(Self::F5),
                NamedKey::F6 => Some(Self::F6),
                NamedKey::F7 => Some(Self::F7),
                NamedKey::F8 => Some(Self::F8),
                NamedKey::F9 => Some(Self::F9),
                NamedKey::F10 => Some(Self::F10),
                NamedKey::F11 => Some(Self::F11),
                NamedKey::F12 => Some(Self::F12),
                _ => None,
            },
            WKey::Character(s) => {
                let mut chars = s.chars();
                let first = chars.next()?;
                if chars.next().is_none() {
                    Some(Self::from_char(first))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Char(c) => write!(f, "{}", c.to_ascii_uppercase()),
            Self::F1 => write!(f, "F1"),
            Self::F2 => write!(f, "F2"),
            Self::F3 => write!(f, "F3"),
            Self::F4 => write!(f, "F4"),
            Self::F5 => write!(f, "F5"),
            Self::F6 => write!(f, "F6"),
            Self::F7 => write!(f, "F7"),
            Self::F8 => write!(f, "F8"),
            Self::F9 => write!(f, "F9"),
            Self::F10 => write!(f, "F10"),
            Self::F11 => write!(f, "F11"),
            Self::F12 => write!(f, "F12"),
            Self::Escape => write!(f, "Escape"),
            Self::Tab => write!(f, "Tab"),
            Self::Enter => write!(f, "Enter"),
            Self::Backspace => write!(f, "Backspace"),
            Self::Delete => write!(f, "Delete"),
            Self::Insert => write!(f, "Insert"),
            Self::Home => write!(f, "Home"),
            Self::End => write!(f, "End"),
            Self::PageUp => write!(f, "PageUp"),
            Self::PageDown => write!(f, "PageDown"),
            Self::ArrowLeft => write!(f, "Left"),
            Self::ArrowRight => write!(f, "Right"),
            Self::ArrowUp => write!(f, "Up"),
            Self::ArrowDown => write!(f, "Down"),
            Self::Space => write!(f, "Space"),
        }
    }
}

/// Modifier keys state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

impl Modifiers {
    pub const NONE: Self = Self {
        ctrl: false,
        alt: false,
        shift: false,
        meta: false,
    };
    pub const CTRL: Self = Self {
        ctrl: true,
        alt: false,
        shift: false,
        meta: false,
    };
    pub const ALT: Self = Self {
        ctrl: false,
        alt: true,
        shift: false,
        meta: false,
    };
    pub const SHIFT: Self = Self {
        ctrl: false,
        alt: false,
        shift: true,
        meta: false,
    };
    pub const META: Self = Self {
        ctrl: false,
        alt: false,
        shift: false,
        meta: true,
    };
    pub const CTRL_SHIFT: Self = Self {
        ctrl: true,
        alt: false,
        shift: true,
        meta: false,
    };
    pub const ALT_SHIFT: Self = Self {
        ctrl: false,
        alt: true,
        shift: true,
        meta: false,
    };
    pub const CTRL_ALT: Self = Self {
        ctrl: true,
        alt: true,
        shift: false,
        meta: false,
    };

    /// Whether this modifier combination requires `Ctrl` or `Alt` to be held.
    #[inline]
    pub const fn requires_ctrl_or_alt(&self) -> bool {
        self.ctrl || self.alt
    }

    /// Construct modifiers from `winit::keyboard::ModifiersState`.
    pub fn from_winit(state: winit::keyboard::ModifiersState) -> Self {
        Self {
            ctrl: state.control_key(),
            alt: state.alt_key(),
            shift: state.shift_key(),
            meta: state.super_key(),
        }
    }
}

/// A specific key combination (modifiers + key).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyShortcut {
    pub modifiers: Modifiers,
    pub key: Key,
}

impl KeyShortcut {
    /// Construct a new shortcut with the given modifiers and key.
    pub const fn new(modifiers: Modifiers, key: Key) -> Self {
        Self { modifiers, key }
    }

    /// Shortcut with no modifiers.
    pub const fn bare(key: Key) -> Self {
        Self {
            modifiers: Modifiers::NONE,
            key,
        }
    }

    /// Shortcut with Ctrl modifier.
    pub const fn ctrl(key: Key) -> Self {
        Self {
            modifiers: Modifiers::CTRL,
            key,
        }
    }

    /// Shortcut with Alt modifier.
    pub const fn alt(key: Key) -> Self {
        Self {
            modifiers: Modifiers::ALT,
            key,
        }
    }

    /// Shortcut with Shift modifier.
    pub const fn shift(key: Key) -> Self {
        Self {
            modifiers: Modifiers::SHIFT,
            key,
        }
    }

    /// Shortcut with Ctrl + Shift modifiers.
    pub const fn ctrl_shift(key: Key) -> Self {
        Self {
            modifiers: Modifiers::CTRL_SHIFT,
            key,
        }
    }

    /// Shortcut with Alt + Shift modifiers.
    pub const fn alt_shift(key: Key) -> Self {
        Self {
            modifiers: Modifiers::ALT_SHIFT,
            key,
        }
    }

    /// Shortcut with Ctrl + Alt modifiers.
    pub const fn ctrl_alt(key: Key) -> Self {
        Self {
            modifiers: Modifiers::CTRL_ALT,
            key,
        }
    }

    /// Whether this shortcut requires `Ctrl` or `Alt` to be held.
    ///
    /// Web view hosts commonly only route accelerator keys when `Ctrl` or `Alt`
    /// is pressed. Shortcuts with `requires_ctrl_or_alt() == false` (such as `F6`,
    /// `Escape`, `Tab`, `Shift+F6`) provide the mandatory escape path out of focus
    /// held by page content traps (research.md §5.4).
    #[inline]
    pub const fn requires_ctrl_or_alt(&self) -> bool {
        self.modifiers.requires_ctrl_or_alt()
    }

    /// Parse a human-readable shortcut string such as `"Ctrl+L"`, `"Ctrl+Shift+T"`, `"F6"`.
    pub fn parse(s: &str) -> Result<Self, KeymapError> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(KeymapError::EmptyShortcutString);
        }

        let parts: Vec<&str> = trimmed.split('+').map(str::trim).collect();
        if parts.is_empty() {
            return Err(KeymapError::EmptyShortcutString);
        }

        let mut modifiers = Modifiers::NONE;
        let key_str = parts.last().copied().unwrap_or("");

        for &part in &parts[..parts.len() - 1] {
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => modifiers.ctrl = true,
                "alt" => modifiers.alt = true,
                "shift" => modifiers.shift = true,
                "meta" | "super" | "cmd" | "command" | "win" => modifiers.meta = true,
                _ => return Err(KeymapError::UnknownKey(part.to_string())),
            }
        }

        let key = match key_str.to_ascii_lowercase().as_str() {
            "escape" | "esc" => Key::Escape,
            "tab" => Key::Tab,
            "enter" | "return" => Key::Enter,
            "backspace" => Key::Backspace,
            "delete" | "del" => Key::Delete,
            "insert" | "ins" => Key::Insert,
            "home" => Key::Home,
            "end" => Key::End,
            "pageup" | "pgup" => Key::PageUp,
            "pagedown" | "pgdn" => Key::PageDown,
            "left" => Key::ArrowLeft,
            "right" => Key::ArrowRight,
            "up" => Key::ArrowUp,
            "down" => Key::ArrowDown,
            "space" => Key::Space,
            "f1" => Key::F1,
            "f2" => Key::F2,
            "f3" => Key::F3,
            "f4" => Key::F4,
            "f5" => Key::F5,
            "f6" => Key::F6,
            "f7" => Key::F7,
            "f8" => Key::F8,
            "f9" => Key::F9,
            "f10" => Key::F10,
            "f11" => Key::F11,
            "f12" => Key::F12,
            other => {
                let mut chars = other.chars();
                if let Some(c) = chars.next() {
                    if chars.next().is_none() {
                        Key::from_char(c)
                    } else {
                        return Err(KeymapError::UnknownKey(key_str.to_string()));
                    }
                } else {
                    return Err(KeymapError::MissingKey);
                }
            }
        };

        Ok(Self::new(modifiers, key))
    }

    /// Try to construct a [`KeyShortcut`] from a `winit::keyboard::Key` and modifiers.
    pub fn from_winit(
        key: &winit::keyboard::Key,
        modifiers: winit::keyboard::ModifiersState,
    ) -> Option<Self> {
        let key = Key::from_winit_key(key)?;
        let modifiers = Modifiers::from_winit(modifiers);
        Some(Self::new(modifiers, key))
    }
}

impl FromStr for KeyShortcut {
    type Err = KeymapError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl fmt::Display for KeyShortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.modifiers.ctrl {
            write!(f, "Ctrl+")?;
        }
        if self.modifiers.alt {
            write!(f, "Alt+")?;
        }
        if self.modifiers.shift {
            write!(f, "Shift+")?;
        }
        if self.modifiers.meta {
            write!(f, "Super+")?;
        }
        write!(f, "{}", self.key)
    }
}

/// Errors related to keymap management and shortcut parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeymapError {
    /// Empty shortcut string provided.
    EmptyShortcutString,
    /// Shortcut string did not contain a key component.
    MissingKey,
    /// Unrecognized key or modifier name.
    UnknownKey(String),
    /// Shortcut already bound to a different command.
    ConflictingBinding {
        shortcut: KeyShortcut,
        existing: ChromeCommand,
        attempted: ChromeCommand,
    },
}

impl fmt::Display for KeymapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyShortcutString => write!(f, "shortcut string cannot be empty"),
            Self::MissingKey => write!(f, "missing key component in shortcut"),
            Self::UnknownKey(k) => write!(f, "unknown key or modifier: '{k}'"),
            Self::ConflictingBinding {
                shortcut,
                existing,
                attempted,
            } => write!(
                f,
                "shortcut '{shortcut}' already bound to '{existing}'; cannot bind to '{attempted}'"
            ),
        }
    }
}

impl std::error::Error for KeymapError {}

/// Action map binding keyboard shortcuts to browser chrome commands.
///
/// Under FR-011:
/// - Maps every pointer-reachable chrome command to at least one keyboard shortcut.
/// - Guarantees non-Ctrl and non-Alt focus escape paths out of page content traps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keymap {
    shortcut_to_command: HashMap<KeyShortcut, ChromeCommand>,
    command_to_shortcuts: HashMap<ChromeCommand, Vec<KeyShortcut>>,
}

impl Default for Keymap {
    fn default() -> Self {
        Self::default_keymap()
    }
}

impl Keymap {
    /// Create a new, empty keymap.
    pub fn new() -> Self {
        Self {
            shortcut_to_command: HashMap::new(),
            command_to_shortcuts: HashMap::new(),
        }
    }

    /// Construct the standard default browser chrome keymap.
    ///
    /// Every pointer-reachable command is mapped to its established browser shortcut,
    /// and non-Ctrl/non-Alt focus escape paths are guaranteed.
    pub fn default_keymap() -> Self {
        let mut keymap = Self::new();

        // ---------------------------------------------------------------------
        // Navigation (FR-001, FR-003)
        // ---------------------------------------------------------------------
        // Focus address bar: Ctrl+L, Alt+D, and bare F6 (focus escape path!)
        keymap.bind(
            KeyShortcut::ctrl(Key::from_char('l')),
            ChromeCommand::FocusAddressBar,
        );
        keymap.bind(
            KeyShortcut::alt(Key::from_char('d')),
            ChromeCommand::FocusAddressBar,
        );
        keymap.bind(KeyShortcut::bare(Key::F6), ChromeCommand::FocusAddressBar);

        // Back: Alt+Left, Backspace
        keymap.bind(
            KeyShortcut::alt(Key::ArrowLeft),
            ChromeCommand::NavigateBack,
        );
        keymap.bind(
            KeyShortcut::bare(Key::Backspace),
            ChromeCommand::NavigateBack,
        );

        // Forward: Alt+Right, Shift+Backspace
        keymap.bind(
            KeyShortcut::alt(Key::ArrowRight),
            ChromeCommand::NavigateForward,
        );
        keymap.bind(
            KeyShortcut::shift(Key::Backspace),
            ChromeCommand::NavigateForward,
        );

        // Reload: Ctrl+R, F5
        keymap.bind(
            KeyShortcut::ctrl(Key::from_char('r')),
            ChromeCommand::Reload,
        );
        keymap.bind(KeyShortcut::bare(Key::F5), ChromeCommand::Reload);

        // Reload bypassing cache: Ctrl+Shift+R, Ctrl+F5
        keymap.bind(
            KeyShortcut::ctrl_shift(Key::from_char('r')),
            ChromeCommand::ReloadBypassingCache,
        );
        keymap.bind(
            KeyShortcut::ctrl(Key::F5),
            ChromeCommand::ReloadBypassingCache,
        );

        // Stop loading: Ctrl+. and Alt+Shift+S
        keymap.bind(
            KeyShortcut::ctrl(Key::Char('.')),
            ChromeCommand::StopLoading,
        );
        keymap.bind(
            KeyShortcut::alt_shift(Key::from_char('s')),
            ChromeCommand::StopLoading,
        );

        // Home: Alt+Home
        keymap.bind(KeyShortcut::alt(Key::Home), ChromeCommand::NavigateHome);

        // ---------------------------------------------------------------------
        // Tabs & Windows (FR-001, FR-006)
        // ---------------------------------------------------------------------
        // New tab: Ctrl+T
        keymap.bind(
            KeyShortcut::ctrl(Key::from_char('t')),
            ChromeCommand::NewTab,
        );

        // Close tab: Ctrl+W, Ctrl+F4
        keymap.bind(
            KeyShortcut::ctrl(Key::from_char('w')),
            ChromeCommand::CloseTab,
        );
        keymap.bind(KeyShortcut::ctrl(Key::F4), ChromeCommand::CloseTab);

        // Reopen closed tab: Ctrl+Shift+T
        keymap.bind(
            KeyShortcut::ctrl_shift(Key::from_char('t')),
            ChromeCommand::ReopenClosedTab,
        );

        // Next tab: Ctrl+Tab, Ctrl+PageDown
        keymap.bind(KeyShortcut::ctrl(Key::Tab), ChromeCommand::NextTab);
        keymap.bind(KeyShortcut::ctrl(Key::PageDown), ChromeCommand::NextTab);

        // Previous tab: Ctrl+Shift+Tab, Ctrl+PageUp
        keymap.bind(
            KeyShortcut::ctrl_shift(Key::Tab),
            ChromeCommand::PreviousTab,
        );
        keymap.bind(KeyShortcut::ctrl(Key::PageUp), ChromeCommand::PreviousTab);

        // Select tab 1 through 8: Ctrl+1 through Ctrl+8
        for i in 1..=8 {
            let digit_char = char::from_digit(i as u32, 10).unwrap_or('1');
            keymap.bind(
                KeyShortcut::ctrl(Key::Char(digit_char)),
                ChromeCommand::SelectTab(i),
            );
        }

        // Select last tab: Ctrl+9
        keymap.bind(
            KeyShortcut::ctrl(Key::Char('9')),
            ChromeCommand::SelectLastTab,
        );

        // Duplicate tab: Ctrl+Shift+D
        keymap.bind(
            KeyShortcut::ctrl_shift(Key::from_char('d')),
            ChromeCommand::DuplicateTab,
        );

        // Toggle pin tab: Ctrl+Shift+P
        keymap.bind(
            KeyShortcut::ctrl_shift(Key::from_char('p')),
            ChromeCommand::TogglePinTab,
        );

        // Toggle mute tab: Ctrl+M
        keymap.bind(
            KeyShortcut::ctrl(Key::from_char('m')),
            ChromeCommand::ToggleMuteTab,
        );

        // New window: Ctrl+N
        keymap.bind(
            KeyShortcut::ctrl(Key::from_char('n')),
            ChromeCommand::NewWindow,
        );

        // New private window: Ctrl+Shift+N
        keymap.bind(
            KeyShortcut::ctrl_shift(Key::from_char('n')),
            ChromeCommand::NewPrivateWindow,
        );

        // Close window: Ctrl+Shift+W, Alt+F4
        keymap.bind(
            KeyShortcut::ctrl_shift(Key::from_char('w')),
            ChromeCommand::CloseWindow,
        );
        keymap.bind(KeyShortcut::alt(Key::F4), ChromeCommand::CloseWindow);

        // ---------------------------------------------------------------------
        // View & Zoom (FR-005)
        // ---------------------------------------------------------------------
        // Zoom in: Ctrl+=, Ctrl++
        keymap.bind(KeyShortcut::ctrl(Key::Char('=')), ChromeCommand::ZoomIn);
        keymap.bind(KeyShortcut::ctrl(Key::Char('+')), ChromeCommand::ZoomIn);

        // Zoom out: Ctrl+-
        keymap.bind(KeyShortcut::ctrl(Key::Char('-')), ChromeCommand::ZoomOut);

        // Reset zoom: Ctrl+0
        keymap.bind(KeyShortcut::ctrl(Key::Char('0')), ChromeCommand::ZoomReset);

        // Fullscreen: F11
        keymap.bind(KeyShortcut::bare(Key::F11), ChromeCommand::ToggleFullscreen);

        // ---------------------------------------------------------------------
        // Page Actions (FR-003, FR-004, FR-005, FR-009)
        // ---------------------------------------------------------------------
        // Find in page: Ctrl+F, F3
        keymap.bind(
            KeyShortcut::ctrl(Key::from_char('f')),
            ChromeCommand::FindInPage,
        );
        keymap.bind(KeyShortcut::bare(Key::F3), ChromeCommand::FindInPage);

        // Find next: Ctrl+G
        keymap.bind(
            KeyShortcut::ctrl(Key::from_char('g')),
            ChromeCommand::FindNext,
        );

        // Find previous: Ctrl+Shift+G
        keymap.bind(
            KeyShortcut::ctrl_shift(Key::from_char('g')),
            ChromeCommand::FindPrevious,
        );

        // Bookmark page: Ctrl+D
        keymap.bind(
            KeyShortcut::ctrl(Key::from_char('d')),
            ChromeCommand::BookmarkPage,
        );

        // Show bookmarks: Ctrl+Shift+O, Ctrl+B
        keymap.bind(
            KeyShortcut::ctrl_shift(Key::from_char('o')),
            ChromeCommand::ShowBookmarks,
        );
        keymap.bind(
            KeyShortcut::ctrl(Key::from_char('b')),
            ChromeCommand::ShowBookmarks,
        );

        // Show history: Ctrl+H
        keymap.bind(
            KeyShortcut::ctrl(Key::from_char('h')),
            ChromeCommand::ShowHistory,
        );

        // Show downloads: Ctrl+J
        keymap.bind(
            KeyShortcut::ctrl(Key::from_char('j')),
            ChromeCommand::ShowDownloads,
        );

        // Print: Ctrl+P
        keymap.bind(KeyShortcut::ctrl(Key::from_char('p')), ChromeCommand::Print);

        // ---------------------------------------------------------------------
        // Shell Controls & Dialogs (FR-008, FR-010, FR-015a, FR-037)
        // ---------------------------------------------------------------------
        // Open menu: Alt+F, Alt+E, F10
        keymap.bind(
            KeyShortcut::alt(Key::from_char('f')),
            ChromeCommand::OpenMenu,
        );
        keymap.bind(
            KeyShortcut::alt(Key::from_char('e')),
            ChromeCommand::OpenMenu,
        );
        keymap.bind(KeyShortcut::bare(Key::F10), ChromeCommand::OpenMenu);

        // Settings: Ctrl+,
        keymap.bind(
            KeyShortcut::ctrl(Key::Char(',')),
            ChromeCommand::OpenSettings,
        );

        // Site permissions: Ctrl+Shift+I
        keymap.bind(
            KeyShortcut::ctrl_shift(Key::from_char('i')),
            ChromeCommand::OpenSitePermissions,
        );

        // Toggle content blocking: Alt+Shift+B
        keymap.bind(
            KeyShortcut::alt_shift(Key::from_char('b')),
            ChromeCommand::ToggleContentBlocking,
        );

        // Hand-off: Alt+Shift+H
        keymap.bind(
            KeyShortcut::alt_shift(Key::from_char('h')),
            ChromeCommand::HandOffCurrentSite,
        );

        // Clear browsing data: Ctrl+Shift+Delete
        keymap.bind(
            KeyShortcut::ctrl_shift(Key::Delete),
            ChromeCommand::ClearBrowsingData,
        );

        // Toggle theme: Alt+Shift+T
        keymap.bind(
            KeyShortcut::alt_shift(Key::from_char('t')),
            ChromeCommand::ToggleTheme,
        );

        // ---------------------------------------------------------------------
        // Focus & Accessibility Trap Escape (FR-011, WCAG 2.1.2, research.md §5.4)
        // ---------------------------------------------------------------------
        // Escape focus trap: bare Escape key (crucial escape path without Ctrl/Alt!)
        keymap.bind(
            KeyShortcut::bare(Key::Escape),
            ChromeCommand::EscapeFocusTrap,
        );

        // Cycle focus forward: bare Tab key
        keymap.bind(
            KeyShortcut::bare(Key::Tab),
            ChromeCommand::CycleFocusForward,
        );

        // Cycle focus backward: Shift+Tab, Shift+F6 (no Ctrl/Alt required!)
        keymap.bind(
            KeyShortcut::shift(Key::Tab),
            ChromeCommand::CycleFocusBackward,
        );
        keymap.bind(
            KeyShortcut::shift(Key::F6),
            ChromeCommand::CycleFocusBackward,
        );

        // Focus content: Ctrl+Shift+C
        keymap.bind(
            KeyShortcut::ctrl_shift(Key::from_char('c')),
            ChromeCommand::FocusContent,
        );

        keymap
    }

    /// Bind a shortcut to a command, replacing any existing binding for that shortcut.
    pub fn bind(&mut self, shortcut: KeyShortcut, command: ChromeCommand) {
        if let Some(old_cmd) = self.shortcut_to_command.insert(shortcut, command) {
            if let Some(shortcuts) = self.command_to_shortcuts.get_mut(&old_cmd) {
                shortcuts.retain(|&s| s != shortcut);
            }
        }
        let list = self.command_to_shortcuts.entry(command).or_default();
        if !list.contains(&shortcut) {
            list.push(shortcut);
        }
    }

    /// Try to bind a shortcut to a command, returning an error if already bound to another command.
    pub fn try_bind(
        &mut self,
        shortcut: KeyShortcut,
        command: ChromeCommand,
    ) -> Result<(), KeymapError> {
        if let Some(&existing) = self.shortcut_to_command.get(&shortcut) {
            if existing != command {
                return Err(KeymapError::ConflictingBinding {
                    shortcut,
                    existing,
                    attempted: command,
                });
            }
        }
        self.bind(shortcut, command);
        Ok(())
    }

    /// Unbind a shortcut if present.
    pub fn unbind(&mut self, shortcut: &KeyShortcut) -> Option<ChromeCommand> {
        let cmd = self.shortcut_to_command.remove(shortcut)?;
        if let Some(shortcuts) = self.command_to_shortcuts.get_mut(&cmd) {
            shortcuts.retain(|s| s != shortcut);
        }
        Some(cmd)
    }

    /// Look up the command triggered by the specified shortcut.
    pub fn command_for_shortcut(&self, shortcut: &KeyShortcut) -> Option<ChromeCommand> {
        self.shortcut_to_command.get(shortcut).copied()
    }

    /// Look up all shortcuts mapped to a given command.
    pub fn shortcuts_for_command(&self, command: ChromeCommand) -> &[KeyShortcut] {
        self.command_to_shortcuts
            .get(&command)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Whether the specified command has at least one keyboard binding.
    pub fn has_binding_for(&self, command: ChromeCommand) -> bool {
        !self.shortcuts_for_command(command).is_empty()
    }

    /// Return all commands that are reachable by pointer but carry NO keyboard binding.
    ///
    /// Under FR-011, this list MUST be empty for the default keymap.
    pub fn unbound_pointer_commands(&self) -> Vec<ChromeCommand> {
        ChromeCommand::all()
            .into_iter()
            .filter(|cmd| cmd.is_pointer_reachable() && !self.has_binding_for(*cmd))
            .collect()
    }

    /// Return all shortcuts that provide an escape path out of focus held by page content
    /// without requiring `Ctrl` or `Alt` to be held.
    ///
    /// Per research.md §5.4 and tasks.md T163, WebView2 on Windows documents `AcceleratorKeyPressed`
    /// as raised only when `Ctrl` or `Alt` is held, so web content holding focus can trap keyboard
    /// input unless a dedicated non-Ctrl/non-Alt escape path exists.
    pub fn focus_escape_shortcuts(&self) -> Vec<KeyShortcut> {
        let mut escapes = Vec::new();
        for &cmd in &[
            ChromeCommand::EscapeFocusTrap,
            ChromeCommand::FocusAddressBar,
            ChromeCommand::CycleFocusForward,
            ChromeCommand::CycleFocusBackward,
        ] {
            for &shortcut in self.shortcuts_for_command(cmd) {
                if !shortcut.requires_ctrl_or_alt() && !escapes.contains(&shortcut) {
                    escapes.push(shortcut);
                }
            }
        }
        escapes
    }

    /// Whether an escape path out of page content focus exists without holding `Ctrl` or `Alt`.
    ///
    /// Verifies that both `F6` and `Escape` are bound without `Ctrl` or `Alt`.
    pub fn has_non_modifier_focus_escape_path(&self) -> bool {
        let escapes = self.focus_escape_shortcuts();
        escapes.contains(&KeyShortcut::bare(Key::F6))
            && escapes.contains(&KeyShortcut::bare(Key::Escape))
    }

    /// Match a raw `winit` key and modifiers event against the keymap.
    pub fn match_event(
        &self,
        key: &winit::keyboard::Key,
        modifiers: winit::keyboard::ModifiersState,
    ) -> Option<ChromeCommand> {
        let shortcut = KeyShortcut::from_winit(key, modifiers)?;
        self.command_for_shortcut(&shortcut)
    }

    /// Return all registered `(shortcut, command)` pairs.
    pub fn all_bindings(&self) -> Vec<(KeyShortcut, ChromeCommand)> {
        self.shortcut_to_command
            .iter()
            .map(|(&s, &c)| (s, c))
            .collect()
    }

    /// Total count of shortcut bindings.
    pub fn len(&self) -> usize {
        self.shortcut_to_command.len()
    }

    /// Whether the keymap has zero bindings.
    pub fn is_empty(&self) -> bool {
        self.shortcut_to_command.is_empty()
    }
}
