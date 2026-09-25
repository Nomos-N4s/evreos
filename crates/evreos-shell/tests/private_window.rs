//! Private-window integration tests for evreos-shell.
//!
//! Under FR-007, data-model §1.2, and T052:
//! - A private window selects the non-persistent data store on the engine seam.
//! - Produces NO history entry and NO session record.
//! - On close, destroys its data store together with its transient permissions
//!   and transient blocking exceptions.
//! - Runs a full private session against the headless engine and asserts that
//!   the profile directory is byte-identical before and after.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use evreos_engine::{DataStoreSelector, Engine, Request, SurfaceState};
use evreos_engine_headless::HeadlessEngine;
use evreos_shell::app::{App, mint_window_id};
use evreos_shell::permissions::{Capability, PermissionDecision, PermissionStore, WindowScope};
use evreos_shell::private::{PrivateSession, PrivateWindow, record_history_safely};
use evreos_shell::profile::Profile;
use evreos_shell::session::SessionStore;
use evreos_shell::site_key::SiteKey;
use evreos_shell::store::{BookmarkStore, DownloadStore, HistoryStore, WindowKind};
use evreos_shell::tabs::{MockClock, WindowTabs};

static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(1);

fn temp_profile_dir(test_name: &str) -> PathBuf {
    let count = TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "evreos_test_private_{test_name}_{}_{count}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp profile dir");
    dir
}

/// Recursively read all files under `dir` into a map of relative path to byte content.
fn snapshot_directory(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut snapshot = BTreeMap::new();
    collect_files_recursive(dir, dir, &mut snapshot);
    snapshot
}

fn collect_files_recursive(base: &Path, current: &Path, snapshot: &mut BTreeMap<PathBuf, Vec<u8>>) {
    if !current.exists() {
        return;
    }
    let entries = match fs::read_dir(current) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            let rel = path
                .strip_prefix(base)
                .expect("strip base prefix")
                .to_path_buf();
            let bytes = fs::read(&path).expect("read file bytes");
            snapshot.insert(rel, bytes);
        } else if path.is_dir() {
            collect_files_recursive(base, &path, snapshot);
        }
    }
}

#[test]
fn full_private_session_leaves_profile_directory_byte_identical() {
    let profile_dir = temp_profile_dir("byte_identical_populated");
    let clock = MockClock::new(Instant::now());

    // 1. Establish initial baseline profile with populated normal stores
    let profile = Profile::new(&profile_dir);
    profile.save().expect("save baseline profile");

    let mut hist_store = HistoryStore::open(&profile_dir);
    hist_store
        .record(
            "https://public.example.com/home",
            "Public Home",
            WindowKind::Normal,
        )
        .expect("record normal history 1");
    hist_store
        .record(
            "https://public.example.com/docs",
            "Public Docs",
            WindowKind::Normal,
        )
        .expect("record normal history 2");

    let mut bm_store = BookmarkStore::open(&profile_dir);
    bm_store
        .create_bookmark(
            evreos_shell::store::FolderId::ROOT,
            "Public Docs",
            "https://public.example.com/docs",
        )
        .expect("add normal bookmark");

    let _dl_store = DownloadStore::open(&profile_dir);

    let mut perm_store = PermissionStore::open(&profile_dir);
    let normal_site = SiteKey::from_url("https://public.example.com").expect("normal site key");
    perm_store
        .record_decision(
            normal_site.clone(),
            Capability::Camera,
            PermissionDecision::Granted,
            WindowScope::Persistent,
        )
        .expect("grant persistent normal permission");

    let mut normal_window = WindowTabs::new(mint_window_id(), WindowKind::Normal);
    let t0 = normal_window.open_tab("https://public.example.com/home", &clock);
    normal_window
        .get_tab_mut(t0)
        .unwrap()
        .set_title("Public Home");
    let session_store = SessionStore::open(&profile_dir);
    session_store
        .save(&[&normal_window])
        .expect("save baseline session");

    // 2. Take exact cryptographic byte snapshot of the profile directory BEFORE the private session
    let before_snapshot = snapshot_directory(&profile_dir);
    assert!(
        !before_snapshot.is_empty(),
        "baseline profile directory must contain files"
    );

    // 3. Run a FULL private browsing session against HeadlessEngine
    let mut engine = HeadlessEngine::new()
        .with_page(
            "https://secret-bank.invalid/account",
            "Confidential Bank Portal",
        )
        .with_page(
            "https://confidential.example.gov/secrets",
            "Government Classified",
        );

    let mut private_win = PrivateWindow::open(&mut engine);
    let private_win_id = private_win.id();
    let private_surface_id = private_win.surface_id();

    // Verify engine seam selects non-persistent data store
    assert_eq!(
        private_win.data_store(),
        DataStoreSelector::NonPersistent,
        "private window must select non-persistent data store"
    );
    assert_eq!(
        engine.surface_data_store(private_surface_id),
        Some(DataStoreSelector::NonPersistent)
    );

    // Add private tabs
    let p_tab0 = private_win
        .tabs_mut()
        .open_tab("https://secret-bank.invalid/account", &clock);
    let p_tab1 = private_win
        .tabs_mut()
        .open_tab("https://confidential.example.gov/secrets", &clock);
    private_win
        .tabs_mut()
        .get_tab_mut(p_tab0)
        .unwrap()
        .set_title("Confidential Bank Portal");
    private_win
        .tabs_mut()
        .get_tab_mut(p_tab1)
        .unwrap()
        .set_title("Government Classified");

    // Execute navigations on the non-persistent engine surface
    let _ = engine.start_surface_navigation(
        private_surface_id,
        &Request::new("https://secret-bank.invalid/account"),
    );
    while engine.poll_event().is_some() {}

    let _ = engine.start_surface_navigation(
        private_surface_id,
        &Request::new("https://confidential.example.gov/secrets"),
    );
    while engine.poll_event().is_some() {}

    // Verify engine has retained data in memory during the active session
    assert!(
        engine.surface_has_retained_data(private_surface_id),
        "active non-persistent surface holds data in memory"
    );

    // Record transient permission for private window
    let secret_bank_site = SiteKey::from_url("https://secret-bank.invalid").expect("bank site key");
    private_win
        .record_permission(
            secret_bank_site.clone(),
            Capability::Location,
            PermissionDecision::Granted,
            &mut perm_store,
        )
        .expect("grant transient private permission");

    assert_eq!(
        private_win.query_permission(&secret_bank_site, Capability::Location, &perm_store),
        PermissionDecision::Granted
    );
    assert_eq!(
        perm_store.query(
            &secret_bank_site,
            Capability::Location,
            WindowScope::Persistent
        ),
        PermissionDecision::Ask,
        "persistent scope must remain Ask"
    );

    // Grant transient blocking exception
    private_win.allow_blocking_exception(secret_bank_site.clone());
    assert!(private_win.is_blocking_exception(&secret_bank_site));

    // Attempt history recording for private window: returns Ok(None) and writes nothing
    assert!(!private_win.can_record_history());
    let hist_outcome = record_history_safely(
        &mut hist_store,
        "https://secret-bank.invalid/account",
        "Confidential Bank Portal",
        WindowKind::Private,
    )
    .expect("safe history record");
    assert_eq!(hist_outcome, None);

    // Private window produces no session record (FR-007)
    assert!(!private_win.can_persist_session());

    // 4. Close the private window and destroy its non-persistent data store,
    //    transient permissions, and transient blocking exceptions
    private_win.close(&mut engine, Some(&mut perm_store));

    assert!(private_win.is_closed());
    assert_eq!(
        engine.surface_state(private_surface_id),
        Some(SurfaceState::Closed)
    );
    assert_eq!(engine.surface_current(private_surface_id), None);
    assert!(
        !engine.surface_has_retained_data(private_surface_id),
        "all data in non-persistent store must be destroyed on close"
    );

    // Transient permissions purged
    assert_eq!(
        perm_store.query(
            &secret_bank_site,
            Capability::Location,
            WindowScope::PrivateWindow(private_win_id)
        ),
        PermissionDecision::Ask
    );

    // Transient blocking exceptions destroyed
    assert!(!private_win.is_blocking_exception(&secret_bank_site));

    // 5. Take exact cryptographic byte snapshot of profile directory AFTER the private session
    let after_snapshot = snapshot_directory(&profile_dir);

    // 6. Assert byte-identical profile directory invariant (FR-007, T052)
    assert_eq!(
        before_snapshot.len(),
        after_snapshot.len(),
        "file count must be identical before and after private session"
    );

    for (file_rel_path, before_bytes) in &before_snapshot {
        let after_bytes = after_snapshot.get(file_rel_path).unwrap_or_else(|| {
            panic!(
                "file {} was missing after private session",
                file_rel_path.display()
            )
        });
        assert_eq!(
            before_bytes,
            after_bytes,
            "file {} content differed! Private session must leave profile byte-identical",
            file_rel_path.display()
        );
    }
}

#[test]
fn empty_profile_directory_remains_completely_empty_after_private_session() {
    let profile_dir = temp_profile_dir("byte_identical_empty");
    let clock = MockClock::new(Instant::now());

    // Directory starts empty
    let before_snapshot = snapshot_directory(&profile_dir);
    assert!(before_snapshot.is_empty());

    let mut engine = HeadlessEngine::new().with_page("https://secret.invalid/", "Secret");
    let mut private_win = PrivateWindow::open(&mut engine);
    let surface_id = private_win.surface_id();

    let _ = private_win
        .tabs_mut()
        .open_tab("https://secret.invalid/", &clock);
    let _ = engine.start_surface_navigation(surface_id, &Request::new("https://secret.invalid/"));
    while engine.poll_event().is_some() {}

    assert!(engine.surface_has_retained_data(surface_id));

    private_win.close(&mut engine, None);

    assert!(!engine.surface_has_retained_data(surface_id));

    // After session, directory remains completely empty
    let after_snapshot = snapshot_directory(&profile_dir);
    assert!(
        after_snapshot.is_empty(),
        "private session must create zero files in profile directory"
    );
}

#[test]
fn private_session_coordinator_destroys_all_windows_and_data_stores() {
    let mut engine = HeadlessEngine::new()
        .with_page("https://page1.invalid/", "Page 1")
        .with_page("https://page2.invalid/", "Page 2");
    let mut perm_store = PermissionStore::in_memory();

    let mut session = PrivateSession::new();
    assert_eq!(session.open_window_count(), 0);
    assert!(!session.has_open_windows());

    let w1 = session.open_window(&mut engine);
    let w2 = session.open_window(&mut engine);

    assert_eq!(session.open_window_count(), 2);
    assert!(session.has_open_windows());
    assert!(session.is_private_window(w1));
    assert!(session.is_private_window(w2));

    let s1 = session.get_window(w1).unwrap().surface_id();
    let s2 = session.get_window(w2).unwrap().surface_id();

    assert_eq!(
        engine.surface_data_store(s1),
        Some(DataStoreSelector::NonPersistent)
    );
    assert_eq!(
        engine.surface_data_store(s2),
        Some(DataStoreSelector::NonPersistent)
    );

    let site1 = SiteKey::from_url("https://site1.invalid").unwrap();
    let site2 = SiteKey::from_url("https://site2.invalid").unwrap();

    session
        .get_window_mut(w1)
        .unwrap()
        .allow_blocking_exception(site1.clone());
    session
        .get_window_mut(w2)
        .unwrap()
        .allow_blocking_exception(site2.clone());

    session
        .get_window(w1)
        .unwrap()
        .record_permission(
            site1.clone(),
            Capability::Camera,
            PermissionDecision::Granted,
            &mut perm_store,
        )
        .unwrap();

    let _ = engine.start_surface_navigation(s1, &Request::new("https://page1.invalid/"));
    let _ = engine.start_surface_navigation(s2, &Request::new("https://page2.invalid/"));
    while engine.poll_event().is_some() {}

    assert!(engine.surface_has_retained_data(s1));
    assert!(engine.surface_has_retained_data(s2));

    // Close all windows simultaneously
    session.close_all(&mut engine, Some(&mut perm_store));

    assert_eq!(session.open_window_count(), 0);
    assert!(!session.has_open_windows());

    assert!(!engine.surface_has_retained_data(s1));
    assert!(!engine.surface_has_retained_data(s2));
    assert_eq!(engine.surface_state(s1), Some(SurfaceState::Closed));
    assert_eq!(engine.surface_state(s2), Some(SurfaceState::Closed));

    // Transient permissions purged
    assert_eq!(
        perm_store.query(&site1, Capability::Camera, WindowScope::PrivateWindow(w1)),
        PermissionDecision::Ask
    );
}

#[test]
fn app_private_window_integration_and_lifecycle() {
    let engine = HeadlessEngine::new();
    let mut app = App::new(engine);

    assert_eq!(app.window_count(), 0);
    assert_eq!(app.child_views_alive(), 0);

    let priv_id = app.open_private_window("Private App Window");

    assert_eq!(app.window_count(), 1);
    assert_eq!(app.child_views_alive(), 1);
    assert!(app.is_private_window(priv_id));

    let window = app.get_window(priv_id).expect("private window exists");
    assert!(window.is_private());
    assert_eq!(window.kind(), WindowKind::Private);
    assert_eq!(window.data_store(), DataStoreSelector::NonPersistent);

    // Closing the private window destroys its engine surface and child view
    let closed = app.close_window(priv_id);
    assert!(closed);
    assert_eq!(app.window_count(), 0);
    assert_eq!(app.child_views_alive(), 0);
}

#[test]
fn private_window_produces_no_history_entry_and_no_session_record() {
    let profile_dir = temp_profile_dir("no_history_no_session");
    let clock = MockClock::new(Instant::now());

    let mut hist_store = HistoryStore::open(&profile_dir);
    let session_store = SessionStore::open(&profile_dir);

    let mut private_win = PrivateWindow::new(mint_window_id(), evreos_engine::SurfaceId::FIRST);
    let tab0 = private_win
        .tabs_mut()
        .open_tab("https://secret.example.org", &clock);
    private_win
        .tabs_mut()
        .get_tab_mut(tab0)
        .unwrap()
        .set_title("Secret Title");

    // History record returns Ok(None)
    let res = hist_store
        .record(
            "https://secret.example.org",
            "Secret Title",
            private_win.kind(),
        )
        .expect("history record");
    assert_eq!(res, None);
    assert!(hist_store.entries().is_empty());
    assert!(!hist_store.file_path().exists());

    // Session save write-path exclusion: saves zero windows
    session_store
        .save(&[private_win.tabs()])
        .expect("save session");
    let loaded = session_store.load().expect("load session").unwrap();
    assert!(loaded.windows.is_empty());
}
