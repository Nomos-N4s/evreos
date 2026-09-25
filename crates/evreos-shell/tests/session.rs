//! Tab session store integration tests for evreos-shell.
//!
//! Under FR-001, FR-007, SC-002, and SC-004:
//! - Asserts close-and-reopen restore preserves tab identity, order, and active tab.
//! - Asserts order preservation across reordering and browser restart.
//! - Asserts session restoration defers loading until first activation (`RestoredNotLoaded`).
//! - Asserts that a session file written while a private window was open contains nothing of it
//!   as a write-path exclusion rather than a read filter.
//! - Asserts atomic write safety and file cleanup via `clear()`.

#![forbid(unsafe_code)]

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use evreos_shell::app::mint_window_id;
use evreos_shell::session::SessionStore;
use evreos_shell::store::WindowKind;
use evreos_shell::tabs::{MockClock, TabLifecycle, WindowTabs};

static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(1);

fn temp_profile_dir(test_name: &str) -> PathBuf {
    let count = TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "evreos_test_session_{test_name}_{}_{count}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp profile dir");
    dir
}

#[test]
fn close_and_reopen_restore_preserves_tabs_identity_and_active_state() {
    let profile_dir = temp_profile_dir("close_reopen_restore");
    let store = SessionStore::open(&profile_dir);
    let clock = MockClock::new(Instant::now());

    let mut window = WindowTabs::new(mint_window_id(), WindowKind::Normal);
    let t0 = window.open_tab("https://news.example.com/daily", &clock);
    let t1 = window.open_tab("https://docs.example.com/guide", &clock);
    let t2 = window.open_tab("https://mail.example.com/inbox", &clock);

    window.get_tab_mut(t0).unwrap().set_title("News Daily");
    window.get_tab_mut(t1).unwrap().set_title("Developer Guide");
    window.get_tab_mut(t2).unwrap().set_title("Inbox (3)");

    // Activate the second tab
    window.activate_tab(t1, &clock).expect("activate tab 1");
    assert_eq!(window.active_tab_id(), Some(t1));

    // Save session
    store.save(&[&window]).expect("save session");

    // Simulate browser close by dropping window and reopening store from disk
    drop(window);
    let reopened_store = SessionStore::open(&profile_dir);
    let loaded = reopened_store
        .load()
        .expect("load session")
        .expect("snapshot exists");

    assert_eq!(loaded.windows.len(), 1);
    assert_eq!(loaded.windows[0].tabs.len(), 3);

    let restored_windows = reopened_store.restore_into(&loaded);
    assert_eq!(restored_windows.len(), 1);

    let restored_win = &restored_windows[0];
    assert_eq!(restored_win.len(), 3);
    assert_eq!(restored_win.kind(), WindowKind::Normal);

    // Verify tabs content and positions
    let tabs = restored_win.tabs();
    assert_eq!(
        tabs[0].displayed_address(),
        "https://news.example.com/daily"
    );
    assert_eq!(tabs[0].title(), "News Daily");
    assert_eq!(tabs[0].position(), 0);

    assert_eq!(
        tabs[1].displayed_address(),
        "https://docs.example.com/guide"
    );
    assert_eq!(tabs[1].title(), "Developer Guide");
    assert_eq!(tabs[1].position(), 1);

    assert_eq!(
        tabs[2].displayed_address(),
        "https://mail.example.com/inbox"
    );
    assert_eq!(tabs[2].title(), "Inbox (3)");
    assert_eq!(tabs[2].position(), 2);

    // Verify the previously active tab is now the active tab
    let active_tab = restored_win.active_tab().expect("active tab present");
    assert_eq!(
        active_tab.displayed_address(),
        "https://docs.example.com/guide"
    );
    assert_eq!(restored_win.active_tab_id(), Some(tabs[1].id()));
}

#[test]
fn order_preservation_across_reorder_and_restart() {
    let profile_dir = temp_profile_dir("order_preservation");
    let store = SessionStore::open(&profile_dir);
    let clock = MockClock::new(Instant::now());

    let mut window = WindowTabs::new(mint_window_id(), WindowKind::Normal);
    let _t0 = window.open_tab("https://tab-alpha.example.com", &clock);
    let _t1 = window.open_tab("https://tab-bravo.example.com", &clock);
    let _t2 = window.open_tab("https://tab-charlie.example.com", &clock);
    let t3 = window.open_tab("https://tab-delta.example.com", &clock);

    // Initial positions: [alpha, bravo, charlie, delta]
    // Reorder: move delta (position 3) to position 0
    window.reorder_tab(t3, 0).expect("reorder t3 to 0");

    // Expected order: [delta, alpha, bravo, charlie]
    let current_addrs: Vec<&str> = window
        .tabs()
        .iter()
        .map(|t| t.displayed_address())
        .collect();
    assert_eq!(
        current_addrs,
        vec![
            "https://tab-delta.example.com",
            "https://tab-alpha.example.com",
            "https://tab-bravo.example.com",
            "https://tab-charlie.example.com"
        ]
    );

    // Save session
    store.save(&[&window]).expect("save session");

    // Reload and restore
    let snapshot = store
        .load()
        .expect("load session")
        .expect("snapshot exists");
    let restored_windows = store.restore_into(&snapshot);
    let restored_win = &restored_windows[0];

    assert_eq!(restored_win.len(), 4);
    let restored_addrs: Vec<&str> = restored_win
        .tabs()
        .iter()
        .map(|t| t.displayed_address())
        .collect();
    assert_eq!(
        restored_addrs,
        vec![
            "https://tab-delta.example.com",
            "https://tab-alpha.example.com",
            "https://tab-bravo.example.com",
            "https://tab-charlie.example.com"
        ],
        "tab order must be strictly preserved across save and restore"
    );

    for (idx, tab) in restored_win.tabs().iter().enumerate() {
        assert_eq!(tab.position(), idx);
    }
}

#[test]
fn session_restoration_defers_loading_until_activation() {
    let profile_dir = temp_profile_dir("deferred_loading");
    let store = SessionStore::open(&profile_dir);
    let clock = MockClock::new(Instant::now());

    let mut window = WindowTabs::new(mint_window_id(), WindowKind::Normal);
    let _t0 = window.open_tab("https://primary.example.com", &clock);
    let _t1 = window.open_tab("https://secondary.example.com", &clock);

    // Save session
    store.save(&[&window]).expect("save session");

    // Load and restore
    let snapshot = store.load().unwrap().unwrap();
    let mut restored_windows = store.restore_into(&snapshot);
    let restored_win = &mut restored_windows[0];

    // Under FR-001 and SC-002, all restored tabs start in RestoredNotLoaded
    for tab in restored_win.tabs() {
        assert_eq!(
            tab.lifecycle(),
            &TabLifecycle::RestoredNotLoaded,
            "restored tab must enter RestoredNotLoaded state"
        );
        assert!(tab.lifecycle().is_restored_not_loaded());
        assert!(!tab.lifecycle().is_loading());
        assert!(!tab.lifecycle().is_live());
        assert!(!tab.is_rendered(), "unloaded tab must not be rendered");
        assert!(
            tab.loading_started_at().is_none(),
            "deferred tab has no loading start time"
        );
    }

    // Now activate the first tab
    let first_id = restored_win.tabs()[0].id();
    restored_win
        .activate_tab(first_id, &clock)
        .expect("activate first tab");

    // The activated tab transitions to Loading with clock timestamp
    let activated = restored_win.get_tab(first_id).unwrap();
    assert_eq!(activated.lifecycle(), &TabLifecycle::Loading);
    assert!(activated.lifecycle().is_loading());
    assert!(activated.loading_started_at().is_some());

    // The other tab remains in RestoredNotLoaded (deferred)
    let second_id = restored_win.tabs()[1].id();
    let deferred = restored_win.get_tab(second_id).unwrap();
    assert_eq!(deferred.lifecycle(), &TabLifecycle::RestoredNotLoaded);
    assert!(deferred.loading_started_at().is_none());
}

#[test]
fn write_path_exclusion_of_private_windows() {
    let profile_dir = temp_profile_dir("private_write_path_exclusion");
    let store = SessionStore::open(&profile_dir);
    let clock = MockClock::new(Instant::now());

    // 1. Create a Normal window with public browsing tabs
    let mut normal_win = WindowTabs::new(mint_window_id(), WindowKind::Normal);
    let n0 = normal_win.open_tab("https://public.example.com/portal", &clock);
    normal_win
        .get_tab_mut(n0)
        .unwrap()
        .set_title("Public Portal");

    // 2. Create a Private window with sensitive URLs and secret titles
    let mut private_win = WindowTabs::new(mint_window_id(), WindowKind::Private);
    let p0 = private_win.open_tab("https://secret.bank.example.org/accounts", &clock);
    let p1 = private_win.open_tab("https://classified.example.gov/confidential", &clock);
    private_win
        .get_tab_mut(p0)
        .unwrap()
        .set_title("Bank Secret Account");
    private_win
        .get_tab_mut(p1)
        .unwrap()
        .set_title("Top Secret Document");

    // 3. Save both windows simultaneously
    store
        .save(&[&normal_win, &private_win])
        .expect("save session with mixed windows");

    // 4. Assert disk file invariants: Read raw session.toml from disk
    let session_file = store.file_path();
    assert!(session_file.is_file(), "session.toml must exist");
    let raw_content = fs::read_to_string(&session_file).expect("read raw session file");

    // Write-path exclusion assertion: Zero traces of private browsing enter the session file
    assert!(
        raw_content.contains("https://public.example.com/portal"),
        "normal window tab must be present"
    );
    assert!(
        raw_content.contains("Public Portal"),
        "normal window title must be present"
    );

    assert!(
        !raw_content.contains("secret.bank.example.org"),
        "FR-007: private window URL must never be written to session file"
    );
    assert!(
        !raw_content.contains("Bank Secret Account"),
        "FR-007: private window title must never be written to session file"
    );
    assert!(
        !raw_content.contains("classified.example.gov"),
        "FR-007: private window URL must never be written to session file"
    );
    assert!(
        !raw_content.contains("Top Secret Document"),
        "FR-007: private window title must never be written to session file"
    );
    assert!(
        !raw_content.to_lowercase().contains("private"),
        "FR-007: no private marker or metadata shall be persisted in tab session"
    );

    // 5. Load and restore: only the normal window is restored
    let snapshot = store.load().unwrap().expect("snapshot exists");
    assert_eq!(
        snapshot.windows.len(),
        1,
        "only normal windows can be stored or restored"
    );
    assert_eq!(snapshot.windows[0].tabs.len(), 1);
    assert_eq!(
        snapshot.windows[0].tabs[0].address,
        "https://public.example.com/portal"
    );

    let restored = store.restore_into(&snapshot);
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].kind(), WindowKind::Normal);
    assert_eq!(restored[0].len(), 1);
}

#[test]
fn only_private_windows_saves_empty_session_without_private_leak() {
    let profile_dir = temp_profile_dir("only_private_save");
    let store = SessionStore::open(&profile_dir);
    let clock = MockClock::new(Instant::now());

    let mut private_win = WindowTabs::new(mint_window_id(), WindowKind::Private);
    let p0 = private_win.open_tab("https://incognito.example.org/test", &clock);
    private_win
        .get_tab_mut(p0)
        .unwrap()
        .set_title("Private Only");

    store
        .save(&[&private_win])
        .expect("save session with only private window");

    let raw_content = fs::read_to_string(store.file_path()).expect("read session file");
    assert!(
        !raw_content.contains("incognito.example.org"),
        "private window URL must not be written"
    );
    assert!(
        !raw_content.contains("Private Only"),
        "private window title must not be written"
    );
    assert!(
        !raw_content.contains("[[windows]]"),
        "no windows should be serialized when all are private"
    );

    let snapshot = store.load().unwrap().expect("snapshot exists");
    assert!(snapshot.windows.is_empty());
    let restored = store.restore_into(&snapshot);
    assert!(restored.is_empty());
}

#[test]
fn atomic_write_safety_temp_file_cleanup_and_clear() {
    let profile_dir = temp_profile_dir("atomic_and_cleanup");
    let store = SessionStore::open(&profile_dir);
    let clock = MockClock::new(Instant::now());

    let mut window = WindowTabs::new(mint_window_id(), WindowKind::Normal);
    window.open_tab("https://clean.example.com", &clock);

    // Save session
    store.save(&[&window]).expect("save session");

    // Verify session.toml exists
    assert!(store.file_path().is_file());

    // Verify no temporary files remain in profile_dir
    let entries: Vec<PathBuf> = fs::read_dir(&profile_dir)
        .expect("read dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();

    for entry in &entries {
        let name = entry.file_name().unwrap().to_string_lossy();
        assert!(
            !name.ends_with(".tmp"),
            "temporary file {name} must be atomically renamed and not left lingering"
        );
    }

    // Verify clear() removes the session file
    store.clear().expect("clear session store");
    assert!(!store.file_path().exists(), "session file must be removed");

    // Load after clear returns Ok(None)
    let loaded = store.load().expect("load after clear");
    assert!(loaded.is_none());
}

#[test]
fn load_missing_file_returns_ok_none() {
    let profile_dir = temp_profile_dir("missing_file");
    let store = SessionStore::open(&profile_dir);
    let result = store.load().expect("load missing file");
    assert!(result.is_none());
}
