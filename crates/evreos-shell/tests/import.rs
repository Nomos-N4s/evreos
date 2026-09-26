//! FR-012 import, run against the committed fixture profiles in
//! `tests/fixtures/import/`, whose README states what each one exercises.
//!
//! Asserts that bookmarks and history from Chrome, Edge and Firefox arrive
//! as ordinary rows marked imported; that no site credential arrives by any
//! route — no credential store is opened, and a password carried in an
//! address is removed; that a Firefox store's write-ahead log is read up to
//! its last commit and no further; and that a failed import writes nothing.

#![forbid(unsafe_code)]

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use evreos_i18n::{Language, catalogue};
use evreos_shell::import::snapshot::{Disk, FileSource, SnapshotPolicy};
use evreos_shell::import::{
    ImportError, ImportFailure, ImportJob, ImportScope, ImportState, ImportedData, ImportedNode,
    ProfileLocations, ReadRequest, SourceBrowser, SourceProfile, discover, read_profile_with,
};
use evreos_shell::store::{BookmarkSource, BookmarkStore, FolderId, HistorySource, StoreRegistry};
use evreos_shell::work::WorkerPool;

/// The value every decoy credential store in the fixtures carries.
const SENTINEL: &str = "fixture-credential-sentinel-4b1d";

static COUNTER: AtomicU64 = AtomicU64::new(1);

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/import")
}

fn chrome() -> SourceProfile {
    SourceProfile::new(
        SourceBrowser::Chrome,
        "Person 1",
        fixtures().join("chrome/Default"),
    )
}

fn edge() -> SourceProfile {
    SourceProfile::new(
        SourceBrowser::Edge,
        "Profile 1",
        fixtures().join("edge/Default"),
    )
}

fn firefox() -> SourceProfile {
    SourceProfile::new(
        SourceBrowser::Firefox,
        "default-release",
        fixtures().join("firefox/Profiles/fx1a2b3c.default-release"),
    )
}

fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "evreos_test_import_{label}_{}_{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn quick() -> SnapshotPolicy {
    SnapshotPolicy {
        attempts: 3,
        first_backoff: Duration::ZERO,
        max_backoff: Duration::ZERO,
    }
}

fn import(profile: SourceProfile, language: Language) -> (PathBuf, StoreRegistry, ImportJob) {
    let root = temp_dir("profile");
    let mut stores = StoreRegistry::open(&root);
    let mut job = ImportJob::new(profile, ImportScope::ALL).with_policy(quick());
    job.run(&mut stores, language).expect("import succeeds");
    (root, stores, job)
}

/// The path of folder names from the root to `folder`, root excluded.
fn folder_path(store: &BookmarkStore, mut folder: FolderId) -> Vec<String> {
    let mut names = Vec::new();
    while !folder.is_root() {
        let held = store.get_folder(folder).expect("folder exists");
        names.push(held.name().to_string());
        folder = held.parent().expect("non-root folder has a parent");
    }
    names.reverse();
    names
}

fn bookmark_paths(store: &BookmarkStore) -> Vec<(Vec<String>, String, String)> {
    store
        .bookmarks()
        .iter()
        .map(|b| {
            (
                folder_path(store, b.parent()),
                b.title().to_string(),
                b.address().to_string(),
            )
        })
        .collect()
}

fn strings(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| part.to_string()).collect()
}

fn every_file_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).expect("readable dir") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            out.extend(every_file_under(&path));
        } else {
            out.push(path);
        }
    }
    out
}

#[test]
fn chrome_bookmarks_and_history_arrive_as_ordinary_rows_marked_imported() {
    let (root, stores, job) = import(chrome(), Language::En);
    assert_eq!(job.state(), ImportState::Written);
    assert_eq!(job.source_browser(), SourceBrowser::Chrome);

    let bookmarks = stores.bookmarks();
    let paths = bookmark_paths(bookmarks);
    let bar = ["Imported from Chrome", "Bookmarks toolbar"];
    let expected = [
        (strings(&bar), "Σελίδα έναρξης", "https://start.example/"),
        (
            strings(&[bar[0], bar[1], "Work"]),
            "Tracker",
            "https://tracker.example/board",
        ),
        (
            strings(&[bar[0], bar[1], "Work", "Deep"]),
            "Deep link",
            "https://deep.example/a/b",
        ),
        (strings(&bar), "Bank login", "https://bank.example/login"),
        (
            strings(&bar),
            "https://untitled.example/",
            "https://untitled.example/",
        ),
        // AccountBookmarks, merged after the local bar.
        (
            strings(&bar),
            "Account bookmark",
            "https://account.example/",
        ),
        (
            strings(&[bar[0], "Other bookmarks"]),
            "Recipes",
            "https://recipes.example/",
        ),
        (
            strings(&[bar[0], "Mobile bookmarks"]),
            "From phone",
            "https://phone.example/",
        ),
    ];
    for (path, title, address) in &expected {
        assert!(
            paths.contains(&(path.clone(), title.to_string(), address.to_string())),
            "missing {path:?} / {title} / {address}; have {paths:#?}"
        );
    }
    assert_eq!(paths.len(), expected.len(), "the bookmarklet is skipped");
    assert_eq!(job.counts().bookmarks_imported, expected.len());

    // The source's order is kept, and an empty source folder is kept too.
    let top = bookmarks.subfolders(FolderId::ROOT)[0].id();
    let bar_folder = bookmarks.subfolders(top)[0].id();
    let bar_titles: Vec<&str> = bookmarks
        .bookmarks_in_folder(bar_folder)
        .iter()
        .map(|b| b.title())
        .collect();
    assert_eq!(
        bar_titles,
        [
            "Σελίδα έναρξης",
            "Bank login",
            "https://untitled.example/",
            "Account bookmark"
        ]
    );
    let work = bookmarks.subfolders(bar_folder)[0].id();
    let work_folders: Vec<&str> = bookmarks
        .subfolders(work)
        .iter()
        .map(|f| f.name())
        .collect();
    assert_eq!(work_folders, ["Deep", "Empty"]);

    for bookmark in bookmarks.bookmarks() {
        assert_eq!(bookmark.source(), &BookmarkSource::imported("Chrome"));
    }
    let start = bookmarks
        .bookmarks()
        .iter()
        .find(|b| b.address() == "https://start.example/")
        .unwrap();
    assert_eq!(
        start.created_at(),
        UNIX_EPOCH + Duration::from_secs(1_700_000_001),
        "date_added converts from Chromium's 1601 epoch"
    );

    let history = stores.history();
    // 1500 generated rows, the Greek title, the overflowing address and the
    // bank row; hidden, browser-internal, inline and never-visited rows are
    // not imported.
    assert_eq!(history.count(), 1503);
    assert_eq!(job.counts().history_imported, 1503);
    for entry in history.entries() {
        assert_eq!(entry.source(), &HistorySource::imported("Chrome"));
    }
    let greek = history.search("Καλημέρα");
    assert_eq!(greek.len(), 1);
    assert_eq!(greek[0].address(), "https://el.example/");
    assert_eq!(
        greek[0].visited_at(),
        UNIX_EPOCH + Duration::from_secs(1_700_000_000 + 1_600 * 60)
    );
    let long = history.search("Long address");
    assert_eq!(
        long[0].address().len(),
        "https://long.example/?q=".len() + 6000
    );
    assert!(history.search("hidden.example").is_empty());
    assert!(history.search("chrome://").is_empty());
    assert!(history.search("never.example").is_empty());
    assert_eq!(
        history.search("bank.example")[0].address(),
        "https://bank.example/account"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn edge_imports_through_the_chromium_formats_and_is_named_edge() {
    let (root, stores, job) = import(edge(), Language::En);
    assert_eq!(job.counts().bookmarks_imported, 2);
    assert_eq!(job.counts().history_imported, 2, "edge:// is skipped");
    let paths = bookmark_paths(stores.bookmarks());
    assert!(paths.contains(&(
        strings(&["Imported from Edge", "Other bookmarks", "Travel"]),
        "Maps".into(),
        "https://maps.example/".into()
    )));
    for entry in stores.history().entries() {
        assert_eq!(entry.source(), &HistorySource::imported("Edge"));
    }
    for bookmark in stores.bookmarks().bookmarks() {
        assert_eq!(bookmark.source(), &BookmarkSource::imported("Edge"));
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn firefox_reads_its_write_ahead_log_to_the_last_commit_and_no_further() {
    let (root, stores, job) = import(firefox(), Language::En);
    let history = stores.history();

    // Committed after the last checkpoint: present only in places.sqlite-wal.
    assert_eq!(history.search("only-in-log.example").len(), 1);
    assert_eq!(history.search("Renamed in the log").len(), 1);
    // Spilled into the log by a transaction that never committed.
    assert!(history.search("uncommitted.example").is_empty());

    // 800 generated rows, the Greek one, the bank row, the untitled one, the
    // tagged one and the log-only one. Hidden, about:, place:, javascript:
    // and never-visited rows are not history.
    assert_eq!(history.count(), 805);
    assert_eq!(job.counts().history_imported, 805);
    assert!(history.search("hidden.example").is_empty());
    assert!(history.search("bookmarked-only").is_empty());
    for entry in history.entries() {
        assert_eq!(entry.source(), &HistorySource::imported("Firefox"));
    }

    let paths = bookmark_paths(stores.bookmarks());
    let top = "Imported from Firefox";
    let expected = [
        (
            strings(&[top, "Bookmarks toolbar"]),
            "Γειά",
            "https://el.example/firefox",
        ),
        (
            strings(&[top, "Bookmarks toolbar", "Reading", "Later"]),
            "Bookmarked only",
            "https://bookmarked-only.example/",
        ),
        // No title of its own: the place's title stands in.
        (
            strings(&[top, "Bookmarks toolbar"]),
            "Place title",
            "https://untitled.example/",
        ),
        (
            strings(&[top, "Bookmarks toolbar"]),
            "Bank",
            "https://bank.example/",
        ),
        (
            strings(&[top, "Bookmarks menu", "Mozilla Firefox"]),
            "Get Help",
            "https://fx0.example/p/0",
        ),
        (
            strings(&[top, "Bookmarks menu", "Mozilla Firefox"]),
            "Get Involved",
            "https://fx1.example/p/1",
        ),
        (
            strings(&[top, "Other bookmarks"]),
            "Unfiled one",
            "https://fx2.example/p/2",
        ),
        (
            strings(&[top, "Other bookmarks"]),
            "Only in the log",
            "https://only-in-log.example/",
        ),
    ];
    for (path, title, address) in &expected {
        assert!(
            paths.contains(&(path.clone(), title.to_string(), address.to_string())),
            "missing {path:?} / {title} / {address}; have {paths:#?}"
        );
    }
    // Separators, the place: query, the javascript: bookmark and the tag
    // entry are not bookmarks to import.
    assert_eq!(paths.len(), expected.len(), "have {paths:#?}");
    assert!(
        !paths
            .iter()
            .any(|(_, _, address)| address.contains("tagged"))
    );

    // Toolbar order follows Firefox's positions, not its row order.
    let bookmarks = stores.bookmarks();
    let top_id = bookmarks.subfolders(FolderId::ROOT)[0].id();
    let toolbar = bookmarks.subfolders(top_id)[0];
    assert_eq!(
        toolbar.name(),
        "Bookmarks toolbar",
        "roots in their fixed order"
    );
    let order: Vec<&str> = bookmarks
        .bookmarks_in_folder(toolbar.id())
        .iter()
        .map(|b| b.title())
        .collect();
    assert_eq!(order, ["Γειά", "Place title", "Bank"]);
    let root_names: Vec<&str> = bookmarks
        .subfolders(top_id)
        .iter()
        .map(|f| f.name())
        .collect();
    assert_eq!(
        root_names,
        ["Bookmarks toolbar", "Bookmarks menu", "Other bookmarks"],
        "the empty mobile root is not written"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn no_site_credential_is_imported_by_any_route() {
    // Structurally: no browser's list of files read names a credential store.
    let credential_stores = [
        "Login Data",
        "Login Data For Account",
        "Web Data",
        "Cookies",
        "logins.json",
        "logins.db",
        "key4.db",
        "key3.db",
        "signons.sqlite",
        "cookies.sqlite",
        "formhistory.sqlite",
    ];
    for browser in SourceBrowser::ALL {
        for file in browser.store_files() {
            assert!(
                !credential_stores.contains(file),
                "{} reads {file}",
                browser.name()
            );
        }
    }

    // In practice: every path each import opens is on its browser's list.
    struct Recording(Vec<PathBuf>);
    impl FileSource for Recording {
        fn read(&mut self, path: &Path) -> io::Result<Option<Vec<u8>>> {
            self.0.push(path.to_path_buf());
            Disk.read(path)
        }
        fn unchanged(&mut self, path: &Path, held: Option<&[u8]>) -> io::Result<bool> {
            self.0.push(path.to_path_buf());
            Disk.unchanged(path, held)
        }
        fn journal_hot(&mut self, path: &Path) -> io::Result<bool> {
            self.0.push(path.to_path_buf());
            Disk.journal_hot(path)
        }
        fn pause(&mut self, _: Duration) {}
    }
    for profile in [chrome(), edge(), firefox()] {
        let mut recording = Recording(Vec::new());
        read_profile_with(&profile, ImportScope::ALL, quick(), &mut recording).unwrap();
        assert!(!recording.0.is_empty());
        for path in &recording.0 {
            assert_eq!(path.parent(), Some(profile.path.as_path()));
            let name = path.file_name().unwrap().to_str().unwrap();
            assert!(
                profile.browser.store_files().contains(&name),
                "{} opened {name}",
                profile.browser.name()
            );
        }
    }

    // And in the result: the sentinel every decoy credential store carries,
    // which the fixtures also embed as the password of three addresses,
    // appears in no file Evreos wrote.
    for profile in [chrome(), edge(), firefox()] {
        let (root, _, _) = import(profile, Language::En);
        let written = every_file_under(&root);
        assert!(!written.is_empty());
        for file in written {
            let bytes = fs::read(&file).unwrap();
            let text = String::from_utf8_lossy(&bytes);
            assert!(
                !text.contains(SENTINEL),
                "{} holds a credential",
                file.display()
            );
            assert!(
                !text.contains("alice:"),
                "{} holds a user name",
                file.display()
            );
        }
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn folder_names_come_from_the_catalogue_in_the_members_language() {
    for language in Language::ALL {
        let (root, stores, _) = import(firefox(), language);
        let messages = catalogue(language);
        let bookmarks = stores.bookmarks();
        let top = bookmarks.subfolders(FolderId::ROOT)[0];
        assert_eq!(
            top.name(),
            messages
                .resolve("import.folder", &[("browser", "Firefox")])
                .unwrap()
        );
        let names: Vec<&str> = bookmarks
            .subfolders(top.id())
            .iter()
            .map(|f| f.name())
            .collect();
        let expected: Vec<String> = ["toolbar", "menu", "other"]
            .iter()
            .map(|root| {
                messages
                    .resolve(&format!("import.root.{root}"), &[])
                    .unwrap()
            })
            .collect();
        assert_eq!(names, expected, "{}", language.subtag());
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn imported_rows_survive_restart_and_delete_like_any_other_row() {
    let (root, stores, _) = import(chrome(), Language::En);
    let history_count = stores.history().count();
    let bookmark_count = stores.bookmarks().bookmarks().len();
    drop(stores);

    let mut reopened = StoreRegistry::open(&root);
    assert_eq!(reopened.history().count(), history_count);
    assert_eq!(reopened.bookmarks().bookmarks().len(), bookmark_count);
    assert!(
        reopened
            .history()
            .entries()
            .iter()
            .all(|e| e.source() == &HistorySource::imported("Chrome")),
        "the imported mark survives the round trip"
    );

    let greek = reopened.history().search("Καλημέρα")[0].entry_id();
    assert!(reopened.history_mut().delete_entry(greek).unwrap());
    let top = reopened.bookmarks().subfolders(FolderId::ROOT)[0].id();
    reopened.bookmarks_mut().delete_folder(top).unwrap();
    drop(reopened);

    let again = StoreRegistry::open(&root);
    assert!(again.history().search("Καλημέρα").is_empty());
    assert!(again.bookmarks().bookmarks().is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_second_import_adds_no_duplicate_history() {
    let (root, mut stores, _) = import(edge(), Language::En);
    let mut again = ImportJob::new(edge(), ImportScope::ALL).with_policy(quick());
    let counts = again.run(&mut stores, Language::En).unwrap();
    assert_eq!(counts.history_imported, 0);
    assert_eq!(stores.history().count(), 2);
    assert_eq!(
        stores.bookmarks().subfolders(FolderId::ROOT).len(),
        2,
        "each import files its bookmarks in a folder of its own"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scope_limits_what_is_read_and_written() {
    let root = temp_dir("scope");
    let mut stores = StoreRegistry::open(&root);
    let scope = ImportScope {
        bookmarks: false,
        history: true,
    };
    let mut job = ImportJob::new(chrome(), scope).with_policy(quick());
    let counts = job.run(&mut stores, Language::En).unwrap();
    assert_eq!(counts.bookmarks_imported, 0);
    assert!(stores.bookmarks().bookmarks().is_empty());
    assert_eq!(stores.bookmarks().folders().len(), 1, "only the root");
    assert!(stores.history().count() > 0);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_missing_profile_fails_and_writes_nothing() {
    let root = temp_dir("missing");
    let mut stores = StoreRegistry::open(&root);
    let gone = SourceProfile::new(SourceBrowser::Chrome, "gone", root.join("no-such-profile"));
    let mut job = ImportJob::new(gone, ImportScope::ALL);
    assert!(matches!(
        job.run(&mut stores, Language::En),
        Err(ImportError::ProfileMissing)
    ));
    assert_eq!(
        job.state(),
        ImportState::Failed(ImportFailure::ProfileMissing)
    );
    assert_eq!(job.counts().history_imported, 0);
    assert!(stores.history().is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_corrupt_store_fails_the_import_before_anything_is_written() {
    let source = temp_dir("corrupt_source");
    let original = fixtures().join("chrome/Default");
    for name in ["Bookmarks", "History"] {
        fs::copy(original.join(name), source.join(name)).unwrap();
    }
    // Cut the history store short: its b-tree now points past the file.
    let history = fs::read(source.join("History")).unwrap();
    fs::write(source.join("History"), &history[..history.len() / 3]).unwrap();

    let root = temp_dir("corrupt_profile");
    let mut stores = StoreRegistry::open(&root);
    let profile = SourceProfile::new(SourceBrowser::Chrome, "corrupt", &source);
    let mut job = ImportJob::new(profile, ImportScope::ALL).with_policy(quick());
    let error = job.run(&mut stores, Language::En).unwrap_err();
    assert!(
        matches!(
            error,
            ImportError::Unreadable {
                store: "History",
                ..
            }
        ),
        "{error}"
    );
    assert_eq!(job.state(), ImportState::Failed(ImportFailure::Unreadable));
    assert!(
        stores.bookmarks().bookmarks().is_empty(),
        "the readable bookmarks were not written either"
    );
    assert!(stores.history().is_empty());
    fs::remove_dir_all(source).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_log_cut_mid_frame_still_reads_to_its_last_whole_commit() {
    let source = temp_dir("torn_log");
    let original = firefox().path;
    fs::copy(original.join("places.sqlite"), source.join("places.sqlite")).unwrap();
    let log = fs::read(original.join("places.sqlite-wal")).unwrap();
    // Header, then the first transaction's two frames and half the third:
    // the second transaction's commit frame is torn away.
    let frame = 24 + 32 * 1024;
    fs::write(
        source.join("places.sqlite-wal"),
        &log[..32 + 2 * frame + frame / 2],
    )
    .unwrap();

    let profile = SourceProfile::new(SourceBrowser::Firefox, "torn", &source);
    let data = read_profile_with(&profile, ImportScope::ALL, quick(), &mut Disk).unwrap();
    let titles: HashSet<&str> = data.history.iter().map(|v| v.title.as_str()).collect();
    assert!(
        titles.contains("Only in the log"),
        "the first commit is kept"
    );
    assert!(
        !titles.contains("Renamed in the log"),
        "the torn second commit is not applied"
    );
    fs::remove_dir_all(source).unwrap();
}

#[test]
fn a_store_that_never_holds_still_is_reported_busy() {
    struct Restless;
    impl FileSource for Restless {
        fn read(&mut self, path: &Path) -> io::Result<Option<Vec<u8>>> {
            Disk.read(path)
        }
        fn unchanged(&mut self, _: &Path, _: Option<&[u8]>) -> io::Result<bool> {
            Ok(false)
        }
        fn journal_hot(&mut self, _: &Path) -> io::Result<bool> {
            Ok(false)
        }
        fn pause(&mut self, _: Duration) {}
    }
    let result = read_profile_with(&firefox(), ImportScope::ALL, quick(), &mut Restless);
    match result {
        Err(ImportError::SourceBusy { store, attempts }) => {
            assert_eq!(store, "places.sqlite");
            assert_eq!(attempts, 3);
        }
        other => panic!("expected SourceBusy, got {other:?}"),
    }
}

#[test]
fn a_failed_history_write_takes_the_bookmarks_back_out() {
    let root = temp_dir("rollback");
    // A directory where the history file belongs makes its write fail.
    fs::create_dir_all(root.join("history.toml")).unwrap();
    let mut stores = StoreRegistry::open(&root);
    let mut job = ImportJob::new(edge(), ImportScope::ALL).with_policy(quick());
    let error = job.run(&mut stores, Language::En).unwrap_err();
    assert!(matches!(error, ImportError::History(_)), "{error}");
    assert_eq!(job.state(), ImportState::Failed(ImportFailure::WriteFailed));
    assert!(stores.bookmarks().bookmarks().is_empty());
    assert_eq!(stores.bookmarks().folders().len(), 1);
    assert!(stores.history().is_empty());
    assert!(
        StoreRegistry::open(&root)
            .bookmarks()
            .bookmarks()
            .is_empty(),
        "and nothing of it reached disk"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn the_read_runs_on_the_worker_pool_and_the_write_on_the_ui_thread() {
    fn assert_send<T: Send + 'static>() {}
    assert_send::<ReadRequest>();
    assert_send::<Result<ImportedData, ImportError>>();

    let root = temp_dir("pool");
    let mut stores = StoreRegistry::open(&root);
    let mut pool = WorkerPool::<Result<ImportedData, ImportError>>::new(1, 1);
    let mut job = ImportJob::new(firefox(), ImportScope::ALL).with_policy(quick());
    let request = job.start_reading();
    assert_eq!(job.state(), ImportState::Reading);
    let id = pool.submit(move || request.execute()).unwrap();

    let mut read = None;
    for _ in 0..1000 {
        if let Some(done) = pool.drain_results().into_iter().find(|r| r.id() == id) {
            assert_ne!(done.worker_thread_id(), std::thread::current().id());
            read = done.into_value();
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let read = read.expect("the read completes on the pool");
    let now = SystemTime::now();
    let counts = job.finish(read, &mut stores, Language::En, now).unwrap();
    assert_eq!(job.state(), ImportState::Written);
    assert_eq!(counts, job.counts());
    assert_eq!(stores.history().count(), counts.history_imported);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn discovery_finds_each_browsers_profiles_by_the_names_they_carry() {
    let locations = ProfileLocations {
        chrome: Some(fixtures().join("chrome")),
        edge: Some(fixtures().join("edge")),
        firefox: Some(fixtures().join("firefox")),
    };
    let found: Vec<(SourceBrowser, String, String)> = discover(&locations)
        .into_iter()
        .map(|p| {
            let dir = p.path.file_name().unwrap().to_string_lossy().into_owned();
            (p.browser, p.name, dir)
        })
        .collect();
    assert_eq!(
        found,
        [
            (SourceBrowser::Chrome, "Person 1".into(), "Default".into()),
            (SourceBrowser::Chrome, "Work".into(), "Profile 1".into()),
            (SourceBrowser::Edge, "Profile 1".into(), "Default".into()),
            (
                SourceBrowser::Firefox,
                "default-release".into(),
                "fx1a2b3c.default-release".into()
            ),
        ],
        "System Profile, Guest Profile and a profiles.ini entry whose \
         directory is gone are not profiles"
    );
    assert!(discover(&ProfileLocations::default()).is_empty());
}

#[test]
fn a_profile_with_no_stores_imports_nothing_and_succeeds() {
    let (root, stores, job) = import(
        SourceProfile::new(
            SourceBrowser::Chrome,
            "Work",
            fixtures().join("chrome/Profile 1"),
        ),
        Language::En,
    );
    assert_eq!(job.state(), ImportState::Written);
    assert_eq!(job.counts(), Default::default());
    assert_eq!(
        stores.bookmarks().folders().len(),
        1,
        "no empty import folder"
    );
    assert!(stores.history().is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn imported_data_counts_bookmarks_but_not_folders() {
    let data = read_profile_with(&chrome(), ImportScope::ALL, quick(), &mut Disk).unwrap();
    assert_eq!(data.bookmark_count(), 8);
    let folders = data
        .roots
        .iter()
        .flat_map(|root| &root.children)
        .filter(|node| matches!(node, ImportedNode::Folder { .. }))
        .count();
    assert_eq!(folders, 1, "Work, on the bar");
}

#[test]
fn the_import_names_no_egress_crate_and_reaches_only_the_stores() {
    // Reading another browser's files is the local computation FR-007a
    // permits; the import must hold no route by which any of it could leave.
    // The crate as a whole depends on evreos-net, so what is asserted is the
    // module's own reach: it names no egress crate, and its only way into the
    // rest of this crate is the stores it writes. `crate::` and, from
    // import.rs, `super::` both lead to the crate root, as `super::super::`
    // does from a submodule, so each is held to the stores alone.
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = vec![(src.join("import.rs"), true)];
    for entry in fs::read_dir(src.join("import")).unwrap() {
        files.push((entry.unwrap().path(), false));
    }
    assert!(files.len() >= 6, "import.rs and its five modules");
    for (file, at_root) in files {
        let content = fs::read_to_string(&file).unwrap();
        for line in content.lines().map(str::trim) {
            // The unit-test module that closes each file is not shipped code,
            // and its `super::` is the module above it, not the crate root.
            if line == "#[cfg(test)]" {
                break;
            }
            if line.starts_with("//") {
                continue;
            }
            assert!(
                !line.contains("evreos_net") && !line.contains("evreos-net"),
                "{} references the egress crate: {line}",
                file.display()
            );
            let root_paths: &[&str] = if at_root {
                &["crate::", "super::"]
            } else {
                &["crate::", "super::super::"]
            };
            for root in root_paths {
                for (at, _) in line.match_indices(root) {
                    assert!(
                        line[at + root.len()..].starts_with("store"),
                        "{} reaches outside the stores: {line}",
                        file.display()
                    );
                }
            }
        }
    }
}
