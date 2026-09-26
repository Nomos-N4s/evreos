//! Integration tests for FR-011 keyboard action map.
//!
//! Under FR-011 and WCAG 2.1.2:
//! - Every action reachable by pointer MUST be reachable by keyboard.
//! - Enumerates the chrome's command table and asserts that every pointer-reachable
//!   command carries at least one valid keyboard shortcut.
//! - Verifies an escape path out of focus held by page content that does not depend
//!   on `Ctrl` or `Alt` being held (e.g. `F6`, `Escape`, `Shift+F6`, `Tab`).

#![forbid(unsafe_code)]

use evreos_shell::keymap::{ChromeCommand, CommandCategory, Key, KeyShortcut, Keymap, KeymapError};
use winit::keyboard::{Key as WKey, ModifiersState, NamedKey, SmolStr};

#[test]
fn test_every_pointer_reachable_command_has_keyboard_binding() {
    let keymap = Keymap::default_keymap();

    // Enumerate the chrome's full command table
    let all_commands = ChromeCommand::all();
    assert!(
        !all_commands.is_empty(),
        "Chrome command table must not be empty"
    );

    // Filter to every command that is reachable by pointer
    let pointer_commands: Vec<ChromeCommand> = all_commands
        .iter()
        .copied()
        .filter(|cmd| cmd.is_pointer_reachable())
        .collect();

    assert!(
        !pointer_commands.is_empty(),
        "Pointer-reachable commands must be present"
    );

    // Assert that no pointer-reachable command is left unbound
    let unbound = keymap.unbound_pointer_commands();
    assert!(
        unbound.is_empty(),
        "FR-011 violation: the following pointer-reachable chrome commands carry no keyboard binding: {:?}",
        unbound
    );

    // Verify each pointer-reachable command individually
    for cmd in pointer_commands {
        let shortcuts = keymap.shortcuts_for_command(cmd);
        assert!(
            !shortcuts.is_empty(),
            "Command '{cmd}' ({:?}) is reachable by pointer but has zero keyboard shortcuts",
            cmd.name()
        );
        assert!(
            keymap.has_binding_for(cmd),
            "keymap.has_binding_for({cmd}) must be true"
        );
    }
}

#[test]
fn test_focus_escape_path_without_ctrl_or_alt() {
    let keymap = Keymap::default_keymap();

    // Verifies that a non-Ctrl and non-Alt escape path is established
    assert!(
        keymap.has_non_modifier_focus_escape_path(),
        "An escape path out of content focus that does not depend on Ctrl or Alt must exist"
    );

    let escape_shortcuts = keymap.focus_escape_shortcuts();
    assert!(
        !escape_shortcuts.is_empty(),
        "At least one focus escape shortcut must exist"
    );

    // Every returned focus escape shortcut must strictly require neither Ctrl nor Alt
    for shortcut in &escape_shortcuts {
        assert!(
            !shortcut.requires_ctrl_or_alt(),
            "Focus escape shortcut '{shortcut}' must not require Ctrl or Alt"
        );
        assert!(
            !shortcut.modifiers.ctrl,
            "Focus escape shortcut '{shortcut}' must not have Ctrl set"
        );
        assert!(
            !shortcut.modifiers.alt,
            "Focus escape shortcut '{shortcut}' must not have Alt set"
        );
    }

    // Bare F6 must be bound to a focus command (FocusAddressBar or CycleFocus)
    let f6_shortcut = KeyShortcut::bare(Key::F6);
    let f6_cmd = keymap.command_for_shortcut(&f6_shortcut);
    assert!(
        f6_cmd.is_some(),
        "Bare F6 must be bound to escape content focus"
    );
    let f6_cmd = f6_cmd.unwrap();
    assert!(
        matches!(
            f6_cmd,
            ChromeCommand::FocusAddressBar | ChromeCommand::CycleFocusForward
        ),
        "F6 must focus the address bar or cycle focus, got '{f6_cmd}'"
    );

    // Bare Escape must be bound to escape focus trap / return to chrome
    let esc_shortcut = KeyShortcut::bare(Key::Escape);
    let esc_cmd = keymap.command_for_shortcut(&esc_shortcut);
    assert!(
        esc_cmd.is_some(),
        "Bare Escape must be bound to escape focus trap"
    );
    assert_eq!(
        esc_cmd.unwrap(),
        ChromeCommand::EscapeFocusTrap,
        "Escape key must map to EscapeFocusTrap"
    );

    // Shift+F6 must provide backward cycle without Ctrl or Alt
    let shift_f6 = KeyShortcut::shift(Key::F6);
    let shift_f6_cmd = keymap.command_for_shortcut(&shift_f6);
    assert!(
        shift_f6_cmd.is_some(),
        "Shift+F6 must be bound for focus navigation"
    );
    assert_eq!(
        shift_f6_cmd.unwrap(),
        ChromeCommand::CycleFocusBackward,
        "Shift+F6 must map to CycleFocusBackward"
    );
}

#[test]
fn test_default_keymap_standard_browser_shortcuts() {
    let keymap = Keymap::default_keymap();

    // Address bar focus
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::ctrl(Key::from_char('l'))),
        Some(ChromeCommand::FocusAddressBar)
    );
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::alt(Key::from_char('d'))),
        Some(ChromeCommand::FocusAddressBar)
    );

    // Navigation
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::alt(Key::ArrowLeft)),
        Some(ChromeCommand::NavigateBack)
    );
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::alt(Key::ArrowRight)),
        Some(ChromeCommand::NavigateForward)
    );
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::ctrl(Key::from_char('r'))),
        Some(ChromeCommand::Reload)
    );
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::bare(Key::F5)),
        Some(ChromeCommand::Reload)
    );
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::ctrl_shift(Key::from_char('r'))),
        Some(ChromeCommand::ReloadBypassingCache)
    );
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::alt(Key::Home)),
        Some(ChromeCommand::NavigateHome)
    );

    // Tabs
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::ctrl(Key::from_char('t'))),
        Some(ChromeCommand::NewTab)
    );
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::ctrl(Key::from_char('w'))),
        Some(ChromeCommand::CloseTab)
    );
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::ctrl_shift(Key::from_char('t'))),
        Some(ChromeCommand::ReopenClosedTab)
    );
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::ctrl(Key::Tab)),
        Some(ChromeCommand::NextTab)
    );
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::ctrl_shift(Key::Tab)),
        Some(ChromeCommand::PreviousTab)
    );

    // Tab selection 1 through 8 and 9 (last tab)
    for i in 1..=8 {
        let digit = char::from_digit(i as u32, 10).unwrap();
        assert_eq!(
            keymap.command_for_shortcut(&KeyShortcut::ctrl(Key::Char(digit))),
            Some(ChromeCommand::SelectTab(i))
        );
    }
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::ctrl(Key::Char('9'))),
        Some(ChromeCommand::SelectLastTab)
    );

    // Windows
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::ctrl(Key::from_char('n'))),
        Some(ChromeCommand::NewWindow)
    );
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::ctrl_shift(Key::from_char('n'))),
        Some(ChromeCommand::NewPrivateWindow)
    );
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::ctrl_shift(Key::from_char('w'))),
        Some(ChromeCommand::CloseWindow)
    );

    // View & Zoom
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::ctrl(Key::Char('0'))),
        Some(ChromeCommand::ZoomReset)
    );
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::bare(Key::F11)),
        Some(ChromeCommand::ToggleFullscreen)
    );

    // Find in page
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::ctrl(Key::from_char('f'))),
        Some(ChromeCommand::FindInPage)
    );
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::bare(Key::F3)),
        Some(ChromeCommand::FindInPage)
    );
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::ctrl(Key::from_char('g'))),
        Some(ChromeCommand::FindNext)
    );
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::ctrl_shift(Key::from_char('g'))),
        Some(ChromeCommand::FindPrevious)
    );

    // Bookmarks, History, Downloads
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::ctrl(Key::from_char('d'))),
        Some(ChromeCommand::BookmarkPage)
    );
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::ctrl_shift(Key::from_char('o'))),
        Some(ChromeCommand::ShowBookmarks)
    );
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::ctrl(Key::from_char('h'))),
        Some(ChromeCommand::ShowHistory)
    );
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::ctrl(Key::from_char('j'))),
        Some(ChromeCommand::ShowDownloads)
    );
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::ctrl(Key::from_char('p'))),
        Some(ChromeCommand::Print)
    );

    // Controls & Settings
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::alt_shift(Key::from_char('h'))),
        Some(ChromeCommand::HandOffCurrentSite)
    );
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::alt_shift(Key::from_char('b'))),
        Some(ChromeCommand::ToggleContentBlocking)
    );
    assert_eq!(
        keymap.command_for_shortcut(&KeyShortcut::alt_shift(Key::from_char('t'))),
        Some(ChromeCommand::ToggleTheme)
    );
}

#[test]
fn test_shortcut_parsing_and_display() {
    let parsed = KeyShortcut::parse("Ctrl+L").expect("parse Ctrl+L");
    assert_eq!(parsed, KeyShortcut::ctrl(Key::from_char('l')));
    assert_eq!(format!("{parsed}"), "Ctrl+L");

    let parsed_shift = KeyShortcut::parse("Ctrl+Shift+T").expect("parse Ctrl+Shift+T");
    assert_eq!(parsed_shift, KeyShortcut::ctrl_shift(Key::from_char('t')));
    assert_eq!(format!("{parsed_shift}"), "Ctrl+Shift+T");

    let parsed_f6 = KeyShortcut::parse("F6").expect("parse F6");
    assert_eq!(parsed_f6, KeyShortcut::bare(Key::F6));
    assert_eq!(format!("{parsed_f6}"), "F6");

    let parsed_esc = KeyShortcut::parse("Escape").expect("parse Escape");
    assert_eq!(parsed_esc, KeyShortcut::bare(Key::Escape));
    assert_eq!(format!("{parsed_esc}"), "Escape");

    let parsed_alt_shift = KeyShortcut::parse("Alt+Shift+H").expect("parse Alt+Shift+H");
    assert_eq!(
        parsed_alt_shift,
        KeyShortcut::alt_shift(Key::from_char('h'))
    );

    // Error cases
    assert_eq!(
        KeyShortcut::parse("").unwrap_err(),
        KeymapError::EmptyShortcutString
    );
    assert!(matches!(
        KeyShortcut::parse("Ctrl+InvalidKey").unwrap_err(),
        KeymapError::UnknownKey(_)
    ));
}

#[test]
fn test_try_bind_detects_conflicts() {
    let mut keymap = Keymap::new();
    let shortcut = KeyShortcut::ctrl(Key::from_char('t'));

    // First binding succeeds
    assert!(keymap.try_bind(shortcut, ChromeCommand::NewTab).is_ok());

    // Binding the same command again is idempotent and succeeds
    assert!(keymap.try_bind(shortcut, ChromeCommand::NewTab).is_ok());

    // Binding a conflicting command fails
    let err = keymap
        .try_bind(shortcut, ChromeCommand::CloseTab)
        .unwrap_err();
    assert!(matches!(
        err,
        KeymapError::ConflictingBinding {
            shortcut: s,
            existing: ChromeCommand::NewTab,
            attempted: ChromeCommand::CloseTab,
        } if s == shortcut
    ));
}

#[test]
fn test_unbinding_and_missing_binding_detection() {
    let mut keymap = Keymap::default_keymap();
    assert!(keymap.unbound_pointer_commands().is_empty());

    // Unbind all shortcuts for NewTab
    for shortcut in keymap.shortcuts_for_command(ChromeCommand::NewTab).to_vec() {
        keymap.unbind(&shortcut);
    }

    // Now unbound_pointer_commands MUST catch that NewTab has no binding
    let unbound = keymap.unbound_pointer_commands();
    assert!(
        unbound.contains(&ChromeCommand::NewTab),
        "unbound_pointer_commands must report NewTab after unbinding all its shortcuts"
    );
}

#[test]
fn test_winit_event_matching() {
    let keymap = Keymap::default_keymap();

    // Match bare F6 key event
    let winit_f6 = WKey::Named(NamedKey::F6);
    let empty_mods = ModifiersState::empty();
    assert_eq!(
        keymap.match_event(&winit_f6, empty_mods),
        Some(ChromeCommand::FocusAddressBar)
    );

    // Match bare Escape key event
    let winit_esc = WKey::Named(NamedKey::Escape);
    assert_eq!(
        keymap.match_event(&winit_esc, empty_mods),
        Some(ChromeCommand::EscapeFocusTrap)
    );

    // Match Ctrl+T
    let winit_t = WKey::Character(SmolStr::new("t"));
    assert_eq!(
        keymap.match_event(&winit_t, ModifiersState::CONTROL),
        Some(ChromeCommand::NewTab)
    );

    // Match Ctrl+Shift+T
    assert_eq!(
        keymap.match_event(&winit_t, ModifiersState::CONTROL | ModifiersState::SHIFT),
        Some(ChromeCommand::ReopenClosedTab)
    );

    // Unbound event returns None
    let winit_z = WKey::Character(SmolStr::new("z"));
    assert_eq!(keymap.match_event(&winit_z, ModifiersState::SUPER), None);
}

#[test]
fn test_command_category_coverage() {
    let all = ChromeCommand::all();
    for cmd in all {
        match cmd.category() {
            CommandCategory::Navigation => assert!(!cmd.name().is_empty()),
            CommandCategory::Tabs => assert!(!cmd.name().is_empty()),
            CommandCategory::Windows => assert!(!cmd.name().is_empty()),
            CommandCategory::View => assert!(!cmd.name().is_empty()),
            CommandCategory::PageActions => assert!(!cmd.name().is_empty()),
            CommandCategory::PanesAndDialogs => assert!(!cmd.name().is_empty()),
            CommandCategory::FocusAndAccessibility => assert!(!cmd.name().is_empty()),
        }
    }
}
