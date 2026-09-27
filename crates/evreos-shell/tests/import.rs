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
use evreos_shell::import::json::{self, Json};
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

#[cfg(unix)]
#[test]
fn a_profile_that_cannot_be_looked_at_fails_as_a_read_not_as_missing() {
    let root = temp_dir("unlooked");
    let mut stores = StoreRegistry::open(&root);
    // Two links that lead to each other: the lookup fails with neither
    // "not found" nor "not a directory", as a refused one would; the test
    // runs where access cannot be refused to it.
    let looped = root.join("a");
    std::os::unix::fs::symlink(root.join("b"), &looped).unwrap();
    std::os::unix::fs::symlink(&looped, root.join("b")).unwrap();
    let profile = SourceProfile::new(SourceBrowser::Chrome, "looped", &looped);
    let mut job = ImportJob::new(profile, ImportScope::ALL);
    let error = job.run(&mut stores, Language::En).unwrap_err();
    assert!(
        matches!(error, ImportError::ProfileUnreadable(_)),
        "{error}"
    );
    assert_eq!(job.state(), ImportState::Failed(ImportFailure::ReadFailed));

    // Under a regular file, the profile is simply not there.
    fs::write(root.join("file"), b"").unwrap();
    let under = SourceProfile::new(SourceBrowser::Chrome, "under", root.join("file/p"));
    let mut job = ImportJob::new(under, ImportScope::ALL);
    assert!(matches!(
        job.run(&mut stores, Language::En),
        Err(ImportError::ProfileMissing)
    ));
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
fn a_store_that_cannot_be_read_from_disk_fails_as_a_read_not_a_format() {
    let source = temp_dir("unread_source");
    fs::copy(
        fixtures().join("chrome/Default/Bookmarks"),
        source.join("Bookmarks"),
    )
    .unwrap();
    // A directory where the history store should be: present, but not a
    // regular file, so the copy refuses it before reading a byte.
    fs::create_dir(source.join("History")).unwrap();

    let root = temp_dir("unread_profile");
    let mut stores = StoreRegistry::open(&root);
    let profile = SourceProfile::new(SourceBrowser::Chrome, "unread", &source);
    let mut job = ImportJob::new(profile, ImportScope::ALL).with_policy(quick());
    let error = job.run(&mut stores, Language::En).unwrap_err();
    assert!(
        matches!(
            error,
            ImportError::Io {
                store: "History",
                ..
            }
        ),
        "{error}"
    );
    assert_eq!(job.state(), ImportState::Failed(ImportFailure::ReadFailed));
    assert!(stores.bookmarks().bookmarks().is_empty());
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
fn discovery_skips_a_firefox_profile_path_in_a_share_s_own_shape() {
    // A path opening on two separators names a network share on Windows; on
    // Linux `//dir` is `/dir`, so a real local directory stands in for the
    // share, and a discovery that followed the path would find it.
    let app = temp_dir("share");
    let local = app.join("local-profile");
    fs::create_dir_all(&local).unwrap();
    let shared = format!("/{}", local.display());
    fs::write(
        app.join("profiles.ini"),
        format!(
            "[Profile0]\nName=share\nIsRelative=0\nPath={shared}\n\n\
             [Profile1]\nName=local\nIsRelative=0\nPath={}\n",
            local.display()
        ),
    )
    .unwrap();
    let found: Vec<String> = discover(&ProfileLocations {
        firefox: Some(app.clone()),
        ..ProfileLocations::default()
    })
    .into_iter()
    .map(|profile| profile.name)
    .collect();
    assert_eq!(found, ["local"]);
    fs::remove_dir_all(app).unwrap();
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

/// A token of Rust source, as the reach test needs it: a literal carries
/// nothing, a lifetime is kept as an identifier with its `'` so that its name
/// is checked, and comments and whitespace are dropped.
#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    Punct(char),
    Lit,
}

/// Tokenize Rust source closely enough that no path can hide from the reach
/// test inside what the test takes for a literal or a comment: strings and
/// raw strings with every prefix (`b`, `c`, `r`, `br`, `cr`), byte and
/// character literals, lifetimes, raw identifiers and nested block comments
/// are each read as the compiler reads them.
fn tokens(source: &str) -> Vec<Tok> {
    let chars: Vec<char> = source.chars().collect();
    let at = |i: usize| chars.get(i).copied();
    let mut out = Vec::new();
    let mut i = 0;
    // Past a quoted string whose opening quote is at `i`, honouring escapes.
    let quoted = |mut i: usize| {
        i += 1;
        while i < chars.len() && chars[i] != '"' {
            i += if chars[i] == '\\' { 2 } else { 1 };
        }
        i + 1
    };
    // Past a raw string whose hashes, if any, start at `i`.
    let raw = |mut i: usize| {
        let mut hashes = 0;
        while at(i) == Some('#') {
            hashes += 1;
            i += 1;
        }
        i += 1;
        while i < chars.len() {
            if chars[i] == '"' && (1..=hashes).all(|k| at(i + k) == Some('#')) {
                return i + 1 + hashes;
            }
            i += 1;
        }
        i
    };
    // Past a character literal whose opening quote is at `i`.
    let character = |mut i: usize| {
        i += 1;
        if at(i) == Some('\\') {
            i += 2;
        } else {
            i += 1;
        }
        while i < chars.len() && chars[i] != '\'' {
            i += 1;
        }
        i + 1
    };
    while i < chars.len() {
        let ch = chars[i];
        // Only ASCII whitespace separates tokens here. rustc also skips the
        // two direction marks, U+200E and U+200F, so they and every other
        // character outside ASCII stay in the token stream, for the check to
        // refuse: a letter or digit as part of an identifier, anything else
        // as a token of its own, and never inside a number.
        if ch.is_ascii_whitespace() || ch == '\u{b}' {
            i += 1;
        } else if ch == '/' && at(i + 1) == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if ch == '/' && at(i + 1) == Some('*') {
            let mut depth = 0;
            while i < chars.len() {
                if chars[i] == '/' && at(i + 1) == Some('*') {
                    depth += 1;
                    i += 2;
                } else if chars[i] == '*' && at(i + 1) == Some('/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        } else if ch.is_alphabetic() || ch == '_' {
            let start = i;
            while at(i).is_some_and(|c| c.is_alphanumeric() || c == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            match (word.as_str(), at(i)) {
                ("r", Some('#')) if at(i + 1).is_some_and(|c| c.is_alphabetic() || c == '_') => {
                    // A raw identifier, `r#match`, keeps its `r#`: it names
                    // the identifier itself, but is never the keyword.
                    let start = i + 1;
                    i = start;
                    while at(i).is_some_and(|c| c.is_alphanumeric() || c == '_') {
                        i += 1;
                    }
                    let name: String = chars[start..i].iter().collect();
                    out.push(Tok::Ident(format!("r#{name}")));
                }
                ("r" | "br" | "cr", Some('"' | '#')) => {
                    i = raw(i);
                    out.push(Tok::Lit);
                }
                ("b" | "c", Some('"')) => {
                    i = quoted(i);
                    out.push(Tok::Lit);
                }
                ("b", Some('\'')) => {
                    i = character(i);
                    out.push(Tok::Lit);
                }
                _ => out.push(Tok::Ident(word)),
            }
        } else if ch.is_ascii_digit() {
            // A number's digits and suffix are ASCII; anything past them is
            // read as a token of its own, so none hides inside a literal.
            while at(i).is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') {
                i += 1;
            }
            out.push(Tok::Lit);
        } else if ch == '"' {
            i = quoted(i);
            out.push(Tok::Lit);
        } else if ch == '\'' {
            let lifetime =
                at(i + 1).is_some_and(|c| c.is_alphabetic() || c == '_') && at(i + 2) != Some('\'');
            if lifetime {
                // Kept as a token, `'` and all, so that its name is held to
                // ASCII like any identifier's.
                let start = i;
                i += 1;
                while at(i).is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    i += 1;
                }
                out.push(Tok::Ident(chars[start..i].iter().collect()));
            } else {
                i = character(i);
                out.push(Tok::Lit);
            }
        } else {
            out.push(Tok::Punct(ch));
            i += 1;
        }
    }
    out
}

/// The attributes the import's shipped code may use, each the language's
/// own and named bare. An attribute can be a macro, and one the stores
/// re-exported, `#[crate::store::hook]` or a bare `#[hook]` brought in by
/// `use`, could rewrite the item it sits on.
const ATTRIBUTES: &[&str] = &[
    "allow", "cfg", "deny", "derive", "forbid", "inline", "must_use", "warn",
];

/// The derives the import's shipped code may use, the standard library's. A
/// derive is a macro too, and one the stores re-exported could run any code.
const DERIVES: &[&str] = &[
    "Clone",
    "Copy",
    "Debug",
    "Default",
    "Eq",
    "Hash",
    "PartialEq",
];

/// The five modules `import.rs` declares; no other file can join them.
const IMPORT_MODULES: &[&str] = &["chromium", "firefox", "json", "snapshot", "sqlite"];

/// The macros the import's shipped code may invoke. A macro from elsewhere in
/// the crate needs no path to be named, so it could reach past the stores;
/// these are the standard library's, which reach nothing.
const MACROS: &[&str] = &[
    "assert",
    "cfg",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "format",
    "format_args",
    "matches",
    "panic",
    "unreachable",
    "vec",
    "write",
    "writeln",
];

/// Keywords a leading `::` may follow, as in `use ::std`: before `::` they
/// start a path rather than being a segment of one.
const PATH_KEYWORDS: &[&str] = &[
    "as", "break", "const", "dyn", "else", "for", "if", "impl", "in", "let", "match", "move",
    "mut", "pub", "ref", "return", "static", "type", "use", "where", "while", "yield",
];

/// Keywords that a `!` may follow as negation, not as a macro call.
const KEYWORDS: &[&str] = &[
    "as", "break", "else", "if", "in", "let", "match", "move", "mut", "return", "while", "yield",
];

/// The dependency the import may name: the message catalogue, for the names
/// of the folders it writes.
const ALLOWED_CRATE: &str = "evreos_i18n";

/// Every dependency of this crate's library the import may not name, as its
/// code names them, read once from cargo's own resolution of the crate.
fn denied_crates() -> Vec<String> {
    static DENIED: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    DENIED.get_or_init(resolve_denied_crates).clone()
}

fn resolve_denied_crates() -> Vec<String> {
    // Offline, cargo can resolve only the packages it holds, which are the
    // ones a build for this host with every feature fetched, so the graph
    // is filtered to the host. A dependency declared for some platforms
    // only may then be missing from it, so any such dependency fails the
    // test rather than go unchecked. Every feature is enabled, so an
    // optional dependency is in the graph too, and one declared for every
    // platform always is.
    let resolved = cargo_metadata(&["--filter-platform", &host_triple(), "--all-features"]);
    let declared = cargo_metadata(&["--no-deps"]);
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("Cargo.toml")
        .canonicalize()
        .unwrap();
    let specific = platform_specific(&declared, &manifest);
    assert!(
        specific.is_empty(),
        "declared for some platforms only, so the names its code uses may not \
         be resolved here; extend the check before adding one: {specific:?}"
    );
    let catalogue = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../evreos-i18n/Cargo.toml")
        .canonicalize()
        .unwrap();
    let links = catalogue_links(&resolved, &catalogue);
    assert!(
        links.is_empty(),
        "the catalogue links crates the import could reach through it: {links:?}"
    );
    denied_by(&resolved, &catalogue)
}

/// `cargo metadata`'s report on this crate, taken offline, with `args`.
fn cargo_metadata(args: &[&str]) -> Json {
    let output = std::process::Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--offline",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .args(args)
        .output()
        .unwrap();
    // Offline, cargo resolves the whole workspace from the crates already
    // fetched, so a cache filled by building this crate alone can lack one
    // another member needs. Building the workspace first with every
    // feature, as CI's clippy step does, fetches them all.
    assert!(
        output.status.success(),
        "cargo metadata failed offline; build the whole workspace first so \
         its crates are fetched: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    json::parse(&String::from_utf8(output.stdout).unwrap()).unwrap()
}

/// The host rustc builds for by default, which the graph is filtered to,
/// as the compiler beside cargo, or else the one on the path, names it.
fn host_triple() -> String {
    let beside =
        Path::new(env!("CARGO")).with_file_name(format!("rustc{}", std::env::consts::EXE_SUFFIX));
    // Where cargo stands alone, the compiler is the one on the path.
    let rustc = if beside.is_file() {
        beside
    } else {
        PathBuf::from("rustc")
    };
    let output = std::process::Command::new(rustc)
        .arg("-vV")
        .output()
        .unwrap();
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .expect("rustc names its host")
        .to_string()
}

/// The normal dependencies the package whose manifest is `manifest`
/// declares, in `declared`, a `--no-deps` report, only for some platforms.
/// One declared for every platform is in the host's graph wherever the test
/// runs; one declared for a platform may be missing from it, and its name
/// with it, so none is taken on trust.
fn platform_specific(declared: &Json, manifest: &Path) -> Vec<String> {
    let str_at =
        |value: &Json, key: &str| value.get(key).and_then(Json::as_str).map(str::to_string);
    let array = |value: Option<&Json>| value.and_then(Json::as_array).unwrap_or_default().to_vec();
    let package = array(declared.get("packages"))
        .into_iter()
        .find(|package| {
            str_at(package, "manifest_path")
                .and_then(|path| Path::new(&path).canonicalize().ok())
                .as_deref()
                == Some(manifest)
        })
        .expect("the package declared");
    array(package.get("dependencies"))
        .iter()
        .filter(|dep| dep.get("kind") == Some(&Json::Null))
        .filter(|dep| dep.get("target") != Some(&Json::Null))
        .filter_map(|dep| str_at(dep, "rename").or_else(|| str_at(dep, "name")))
        .collect()
}

#[test]
fn a_dependency_for_some_platforms_is_not_passed_over() {
    let dir = temp_dir("platforms");
    let manifest = dir.join("Cargo.toml");
    fs::write(&manifest, "").unwrap();
    let declared = json::parse(&format!(
        r#"{{"packages": [
            {{"manifest_path": "/elsewhere/Cargo.toml", "dependencies": [
                {{"name": "other", "kind": null, "target": "cfg(windows)"}}
            ]}},
            {{"manifest_path": {manifest:?}, "dependencies": [
                {{"name": "everywhere", "kind": null, "target": null}},
                {{"name": "zz", "kind": null, "target": "cfg(unix)"}},
                {{"name": "zz", "rename": "bar", "kind": null, "target": "cfg(windows)"}},
                {{"name": "tested", "kind": "dev", "target": "cfg(windows)"}},
                {{"name": "built", "kind": "build", "target": "cfg(unix)"}}
            ]}}
        ]}}"#,
        manifest = manifest.display().to_string(),
    ))
    .unwrap();
    assert_eq!(
        platform_specific(&declared, &manifest.canonicalize().unwrap()),
        ["zz", "bar"]
    );
    fs::remove_dir_all(dir).unwrap();
}

/// The crates the catalogue itself links, in `metadata`, a resolved report,
/// where `catalogue` is its manifest. The import may name the catalogue, so
/// a crate it linked could be reached through it, re-exported under another
/// name, and none is allowed.
fn catalogue_links(metadata: &Json, catalogue: &Path) -> Vec<String> {
    let str_at =
        |value: &Json, key: &str| value.get(key).and_then(Json::as_str).map(str::to_string);
    let array = |value: Option<&Json>| value.and_then(Json::as_array).unwrap_or_default().to_vec();
    let id = array(metadata.get("packages"))
        .iter()
        .find(|package| {
            str_at(package, "manifest_path")
                .and_then(|path| Path::new(&path).canonicalize().ok())
                .as_deref()
                == Some(catalogue)
        })
        .and_then(|package| str_at(package, "id"))
        .expect("the catalogue's package");
    let node = array(
        metadata
            .get("resolve")
            .and_then(|resolve| resolve.get("nodes")),
    )
    .into_iter()
    .find(|node| str_at(node, "id").as_deref() == Some(id.as_str()))
    .expect("the catalogue's node");
    array(node.get("deps"))
        .iter()
        .filter(|dep| {
            array(dep.get("dep_kinds"))
                .iter()
                .any(|kind| kind.get("kind") == Some(&Json::Null))
        })
        .filter_map(|dep| str_at(dep, "name"))
        .collect()
}

#[test]
fn a_crate_the_catalogue_links_is_found() {
    let dir = temp_dir("catalogue-links");
    let catalogue = dir.join("Cargo.toml");
    fs::write(&catalogue, "").unwrap();
    let metadata = |deps: &str| {
        json::parse(&format!(
            r#"{{"packages": [{{"id": "cat", "manifest_path": {path:?}}}],
            "resolve": {{"root": "root", "nodes": [
                {{"id": "root", "deps": []}},
                {{"id": "cat", "deps": [{deps}]}}
            ]}}}}"#,
            path = catalogue.display().to_string(),
        ))
        .unwrap()
    };
    let catalogue = catalogue.canonicalize().unwrap();
    assert!(catalogue_links(&metadata(""), &catalogue).is_empty());
    let tested =
        r#"{"name": "trybuild", "pkg": "t", "dep_kinds": [{"kind": "dev", "target": null}]}"#;
    assert!(catalogue_links(&metadata(tested), &catalogue).is_empty());
    let linked =
        r#"{"name": "evreos_net", "pkg": "n", "dep_kinds": [{"kind": null, "target": null}]}"#;
    assert_eq!(
        catalogue_links(&metadata(linked), &catalogue),
        ["evreos_net"]
    );
    fs::remove_dir_all(dir).unwrap();
}

/// The names the root package's code gives its library's dependencies, as
/// `cargo metadata` resolves them, but the catalogue's. The name is cargo's
/// own: a rename, or the dependency's library name, whatever its manifest's
/// key or however that key is spelled. Every dependency of every kind but
/// development and build, on every platform `metadata` resolves, counts.
/// The catalogue's name is allowed only while it resolves to the package
/// whose manifest is `catalogue`, the workspace's own `evreos-i18n`; under
/// any other package it is denied like any other name.
fn denied_by(metadata: &Json, catalogue: &Path) -> Vec<String> {
    let str_at =
        |value: &Json, key: &str| value.get(key).and_then(Json::as_str).map(str::to_string);
    let resolve = metadata.get("resolve").expect("a resolved graph");
    let root = str_at(resolve, "root").expect("a root package");
    let manifest_of = |id: &str| {
        metadata
            .get("packages")
            .and_then(Json::as_array)
            .unwrap_or_default()
            .iter()
            .find(|package| str_at(package, "id").as_deref() == Some(id))
            .and_then(|package| str_at(package, "manifest_path"))
            .and_then(|path| Path::new(&path).canonicalize().ok())
    };
    let node = resolve
        .get("nodes")
        .and_then(Json::as_array)
        .unwrap_or_default()
        .iter()
        .find(|node| str_at(node, "id").as_deref() == Some(root.as_str()))
        .expect("the root package's node");
    let mut denied = Vec::new();
    for dependency in node
        .get("deps")
        .and_then(Json::as_array)
        .unwrap_or_default()
    {
        let name = str_at(dependency, "name").expect("a dependency's name");
        // A kind of `null` is a normal dependency, which the library links.
        let linked = dependency
            .get("dep_kinds")
            .and_then(Json::as_array)
            .unwrap_or_default()
            .iter()
            .any(|kind| kind.get("kind") == Some(&Json::Null));
        let pkg = str_at(dependency, "pkg").expect("a dependency's package");
        let the_catalogue =
            name == ALLOWED_CRATE && manifest_of(&pkg).as_deref() == Some(catalogue);
        if linked && !the_catalogue && !denied.contains(&name) {
            denied.push(name);
        }
    }
    denied
}

#[test]
fn a_dependency_is_denied_by_the_name_cargo_gives_it() {
    let dir = temp_dir("metadata");
    let catalogue = dir.join("evreos-i18n");
    let other = dir.join("evreos-net");
    for package in [&catalogue, &other] {
        fs::create_dir_all(package).unwrap();
        fs::write(package.join("Cargo.toml"), "").unwrap();
    }
    let manifest = |package: &Path| package.join("Cargo.toml").display().to_string();
    // The shape `cargo metadata` gives: the root's node names each
    // dependency as its code does, with its package and kinds.
    let metadata = |deps: &str| {
        json::parse(&format!(
            r#"{{"packages": [
                {{"id": "cat", "manifest_path": {cat:?}}},
                {{"id": "net", "manifest_path": {net:?}}},
                {{"id": "reg", "manifest_path": "/nowhere/Cargo.toml"}}
            ],
            "resolve": {{"root": "root", "nodes": [
                {{"id": "cat", "deps": []}},
                {{"id": "root", "deps": [{deps}]}}
            ]}}}}"#,
            cat = manifest(&catalogue),
            net = manifest(&other),
        ))
        .unwrap()
    };
    let dep = |name: &str, pkg: &str, kind: &str| {
        format!(
            r#"{{"name": "{name}", "pkg": "{pkg}", "dep_kinds": [{{"kind": {kind}, "target": null}}]}}"#
        )
    };
    let catalogue_manifest = catalogue.join("Cargo.toml").canonicalize().unwrap();
    for (deps, expected) in [
        // The catalogue itself, and development and build dependencies.
        (dep("evreos_i18n", "cat", "null"), &[][..]),
        (dep("trybuild", "reg", "\"dev\""), &[][..]),
        (dep("cc", "reg", "\"build\""), &[][..]),
        // A dependency whose library name is not its key: denied by the
        // name code gives it.
        (dep("md5", "reg", "null"), &["md5"][..]),
        // The catalogue's name on another package, by a rename or from
        // another source.
        (dep("evreos_i18n", "net", "null"), &["evreos_i18n"][..]),
        (dep("evreos_i18n", "reg", "null"), &["evreos_i18n"][..]),
        (
            format!(
                "{}, {}",
                dep("evreos_i18n", "cat", "null"),
                dep("evreos_net", "net", "null")
            ),
            &["evreos_net"][..],
        ),
    ] {
        assert_eq!(
            denied_by(&metadata(&deps), &catalogue_manifest),
            expected,
            "{deps}"
        );
    }
    // A dependency that is both normal and development still links.
    let both = r#"{"name": "winit", "pkg": "reg", "dep_kinds": [{"kind": "dev", "target": null}, {"kind": null, "target": "cfg(unix)"}]}"#;
    assert_eq!(denied_by(&metadata(both), &catalogue_manifest), ["winit"]);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_reach_check_denies_every_dependency_but_the_catalogue() {
    let denied = denied_crates();
    assert!(denied.iter().any(|name| name == "evreos_net"), "{denied:?}");
    assert!(!denied.iter().any(|name| name == ALLOWED_CRATE));
    for name in &denied {
        let source = format!("fn f() {{ {name}::g() }}");
        assert!(
            !reach_violations(&source, false).is_empty(),
            "{name} passed"
        );
    }
}

/// Every way the shipped part of one import source file reaches past the
/// stores, the standard library's computation, or its own directory. `at_root`
/// is true for `import.rs`, whose `super` is the crate root.
fn reach_violations(source: &str, at_root: bool) -> Vec<String> {
    reach_violations_in(source, at_root, false)
}

/// As [`reach_violations`], for a file that may define `O_NONBLOCK`: only the
/// copy's module may, once, as a `const`. Its unit tests pin the value on each
/// platform it names one for, each run on that platform: Linux and Android
/// (with MIPS and SPARC apart), and Apple's systems and the BSDs.
fn reach_violations_in(source: &str, at_root: bool, defines_flag: bool) -> Vec<String> {
    let denied = denied_crates();
    let toks = tokens(source);
    // An identifier as it names things, `r#` and all removed; and whether it
    // was written raw, which no keyword is.
    let ident = |i: usize| match toks.get(i) {
        Some(Tok::Ident(word)) => Some(word.strip_prefix("r#").unwrap_or(word)),
        _ => None,
    };
    let raw = |i: usize| matches!(toks.get(i), Some(Tok::Ident(word)) if word.starts_with("r#"));
    let punct = |i: usize, ch: char| toks.get(i) == Some(&Tok::Punct(ch));
    let path_sep = |i: usize| punct(i, ':') && punct(i + 1, ':');
    // Whether each token sits inside a path's `{…}` group, at any depth. A
    // brace opens such a group when it follows `::` or `use`, or follows `{`
    // or `,` inside another such group, as the unprefixed inner group of
    // `use super::{{super::tabs}}` does. A block's brace, as in
    // `{ super::f() }`, opens none.
    let in_group: Vec<bool> = {
        let mut open: Vec<bool> = Vec::new();
        let mut marks = Vec::with_capacity(toks.len());
        for (i, tok) in toks.iter().enumerate() {
            marks.push(open.last().copied().unwrap_or(false));
            match tok {
                Tok::Punct('{') => {
                    let inside = open.last().copied().unwrap_or(false);
                    let group = (i >= 2 && path_sep(i - 2))
                        || ident(i.wrapping_sub(1)) == Some("use")
                        || (inside
                            && (punct(i.wrapping_sub(1), '{') || punct(i.wrapping_sub(1), ',')));
                    open.push(group);
                }
                Tok::Punct('}') => {
                    open.pop();
                }
                _ => {}
            }
        }
        marks
    };
    let in_path_group = |i: usize| in_group[i];
    let mut found = Vec::new();

    // The unit-test module that closes the file is not shipped code, and its
    // `super` is the module above it. It must be the file's last item, so
    // that nothing shipped hides after it.
    let mut end = toks.len();
    let opening = [
        Tok::Punct('#'),
        Tok::Punct('['),
        Tok::Ident("cfg".into()),
        Tok::Punct('('),
        Tok::Ident("test".into()),
        Tok::Punct(')'),
        Tok::Punct(']'),
        Tok::Ident("mod".into()),
        Tok::Ident("tests".into()),
        Tok::Punct('{'),
    ];
    if let Some(start) = toks.windows(opening.len()).position(|w| w == opening) {
        let mut depth = 0usize;
        let mut close = None;
        for (i, tok) in toks.iter().enumerate().skip(start + opening.len() - 1) {
            match tok {
                Tok::Punct('{') => depth += 1,
                Tok::Punct('}') => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(i);
                        break;
                    }
                }
                _ => {}
            }
        }
        if close != Some(toks.len() - 1) {
            found.push("the test module is not the file's last item".to_string());
        }
        end = start;
    }

    // Outside its literals and comments the import is written in ASCII. A
    // character beyond it is either invisible to a reader and skipped by the
    // compiler, as the direction marks are, or part of an identifier this
    // test would read differently from the compiler; either can hide a path.
    for tok in &toks[..end] {
        let outside_ascii = match tok {
            Tok::Ident(word) => !word.is_ascii(),
            Tok::Punct(ch) => !ch.is_ascii(),
            Tok::Lit => false,
        };
        if outside_ascii {
            found.push(format!("a character outside ASCII in code: {tok:?}"));
        }
    }

    let mut flag_definitions = 0;
    for i in 0..end {
        // A glob brings in names the checks below then see bare, with
        // nothing to say where they came from: `use std::fs::*;` makes
        // `write(…)` the file-writing one. The import names what it uses.
        if punct(i, '*') && ((i >= 2 && path_sep(i - 2)) || in_path_group(i)) {
            found.push("a glob import".to_string());
        }
        // An attribute, outer or inner, names one of the language's own.
        if punct(i, '#') {
            let open = if punct(i + 1, '!') { i + 2 } else { i + 1 };
            if punct(open, '[') {
                let name = ident(open + 1);
                if raw(open + 1)
                    || path_sep(open + 2)
                    || !name.is_some_and(|name| ATTRIBUTES.contains(&name))
                {
                    found.push(format!("the attribute {:?}", toks.get(open + 1)));
                }
            }
        }
        let Some(word) = ident(i) else {
            continue;
        };
        // `pub(crate)`, `pub(super)` and `pub(in crate)`: a visibility, not a
        // path.
        let visibility = (punct(i.wrapping_sub(1), '(')
            || (ident(i.wrapping_sub(1)) == Some("in") && punct(i.wrapping_sub(2), '(')))
            && punct(i + 1, ')');
        let next = if path_sep(i + 1) {
            toks.get(i + 3)
        } else {
            None
        };
        let next_word = if path_sep(i + 1) { ident(i + 3) } else { None };
        match word {
            // The egress crate, and every other dependency of this crate but
            // the catalogue's, by the names cargo resolves them to, so that
            // one added later is refused too; and `extern`, which could name
            // any crate.
            _ if denied.iter().any(|name| name == word) => {
                found.push(format!("names the dependency `{word}`"));
            }
            "extern" => found.push("names `extern`".to_string()),
            // `pub(crate)` is a visibility; any other `crate` leads to the
            // crate root, and only the stores may be reached from it.
            "crate" if !visibility && next_word != Some("store") => {
                found.push(format!("`crate` not followed by `::store`: {next:?}"));
            }
            // From import.rs `super` is the crate root, held to the stores as
            // `crate` is. From a module under it, `super` is import.rs, and it
            // may go no further up: neither `super::super`, nor a group that
            // names `super` again, nor a rename.
            "super" if !visibility => {
                // After `::` it continues a path; after a lone `:`, as in
                // `T: super::Trait`, it starts one.
                if path_sep(i.wrapping_sub(2)) || in_path_group(i) {
                    found.push("`super` inside a path".to_string());
                } else if at_root {
                    if next_word != Some("store") {
                        found.push(format!("`super` not followed by `::store`: {next:?}"));
                    }
                } else if next.is_none() || next_word == Some("super") {
                    found.push(format!(
                        "`super` not followed by `::` and an item: {next:?}"
                    ));
                }
            }
            // The standard library, less the parts that open a socket, start
            // a process or reach the platform, and only by a plain path that
            // it begins: after another segment, as in `self::std` or
            // `super::std`, it is a name whose meaning the test cannot see.
            "std" | "core" | "alloc"
                if i >= 3
                    && path_sep(i - 2)
                    && ident(i - 3).is_some_and(|segment| !PATH_KEYWORDS.contains(&segment)) =>
            {
                found.push(format!("`{word}` after another path segment"));
            }
            // Inside a use tree's group it follows the group's prefix, as in
            // `use self::{std::fs::File};`, and names whatever that holds.
            "std" | "core" | "alloc" if in_path_group(i) => {
                found.push(format!("`{word}` inside a use group"));
            }
            "std" | "core" | "alloc" => match next_word {
                // Of the platform's own parts, only `OpenOptionsExt`, the
                // file-opening options the copy uses to open a store without
                // waiting on it, named whole and alone.
                Some("os")
                    if path_sep(i + 4)
                        && ident(i + 6) == Some("unix")
                        && path_sep(i + 7)
                        && ident(i + 9) == Some("fs")
                        && path_sep(i + 10)
                        && ident(i + 12) == Some("OpenOptionsExt")
                        && !path_sep(i + 13) => {}
                Some("net" | "process" | "os") | None => {
                    found.push(format!("`{word}` followed by {next:?}"));
                }
                Some(_) => {}
            },
            // The import writes no file itself; its rows reach disk through
            // the stores alone. Nothing in it may write, create, remove or
            // rename a file, or change one's permissions, mode or times. A
            // file it wrote could be anything the operating system
            // treats as code or as a route out, which no refusal of a module
            // could see.
            "remove_file" | "remove_dir" | "remove_dir_all" | "rename" | "create_dir"
            | "create_dir_all" | "hard_link" | "soft_link" | "symlink" | "set_permissions"
            | "write_all" | "set_len" | "set_times" | "set_modified" | "create_buffered"
            | "write_fmt" | "write_vectored" | "write_all_vectored" | "write_at" | "seek_write" => {
                found.push(format!("names `{word}`, which writes"))
            }
            // The stores write where they are opened, and each opens at any
            // path it is given, so the import opens none: it writes only the
            // stores its caller hands it. No `open` or `try_open` after a
            // path is passed, whatever names the path: the in-tree reader
            // reads bytes already in memory, by `Database::from_bytes`, and
            // has no `open` of its own.
            "open" | "try_open" if path_sep(i.wrapping_sub(2)) => {
                found.push(format!("calls `{word}` after a path"));
            }
            // `write!` on a stream calls its `write_fmt`, which only the
            // `Write` trait in scope provides; without the name, the one
            // `write!` the import can make is a formatter's. `fmt::Write`,
            // which writes only to memory, is refused with it, since the
            // two share the name and the import uses neither.
            "Write" => found.push("names `Write`, through which a stream is written".to_string()),
            "write" | "copy" | "create" | "create_new" | "append"
                if punct(i.wrapping_sub(1), '.')
                    || punct(i.wrapping_sub(1), ':')
                    || in_path_group(i) =>
            {
                found.push(format!("calls `{word}`, which writes"));
            }
            // `OpenOptionsExt` passes flags to the open as they are, and
            // `O_CREAT` or `O_TRUNC` among them would create a file or empty
            // it on a read-only open. The copy passes `O_NONBLOCK` alone, so
            // that is the one argument allowed, and no mode is set at all.
            "custom_flags"
                if !(punct(i + 1, '(')
                    && ident(i + 2) == Some("O_NONBLOCK")
                    && punct(i + 3, ')')) =>
            {
                found.push("`custom_flags` with anything but `O_NONBLOCK`".to_string());
            }
            // The flag the copy passes is named, not given a value here, so
            // what the name holds is held too: it is the one `const` the
            // copy's module defines, and nothing may rebind or shadow it.
            "O_NONBLOCK" => {
                let passed = punct(i.wrapping_sub(1), '(')
                    && ident(i.wrapping_sub(2)) == Some("custom_flags")
                    && punct(i + 1, ')');
                let defined = ident(i.wrapping_sub(1)) == Some("const");
                if defined {
                    flag_definitions += 1;
                }
                if !(passed || (defined && defines_flag)) {
                    found.push("names `O_NONBLOCK` other than to pass it".to_string());
                }
            }
            "mode" if punct(i.wrapping_sub(1), '.') || punct(i.wrapping_sub(1), ':') => {
                found.push("sets a file's mode".to_string());
            }
            // A name the import may invoke as a macro, derive or use as an
            // attribute is the language's only while nothing brings in
            // another under it: a path that ends in it,
            // `use crate::store::format;` or `crate::store::format!()`, or a
            // rename to it, could reach a macro the stores re-export.
            _ if (MACROS.contains(&word)
                || DERIVES.contains(&word)
                || ATTRIBUTES.contains(&word))
                && (path_sep(i.wrapping_sub(2))
                    || in_path_group(i)
                    || matches!(ident(i.wrapping_sub(1)), Some("use" | "as"))) =>
            {
                found.push(format!("names `{word}` by a path or a rename"));
            }
            // A macro named without a path, from anywhere in the crate.
            // A raw identifier is never a keyword, so `r#match!` is a macro,
            // and none of the standard library's is invoked that way.
            _ if punct(i + 1, '!')
                && !punct(i + 2, '=')
                && (raw(i) || !KEYWORDS.contains(&word)) =>
            {
                if raw(i) || !MACROS.contains(&word) {
                    found.push(format!("invokes `{word}!`"));
                }
            }
            // A module loaded from another file: only import.rs may declare
            // one, only its own five, and never from a path of its choosing.
            "mod" if punct(i + 2, ';') => {
                let name = ident(i + 1).unwrap_or("");
                if !at_root || !IMPORT_MODULES.contains(&name) {
                    found.push(format!("declares `mod {name};`"));
                }
            }
            // A derive names its macros bare, and only the standard
            // library's are allowed.
            "derive" if punct(i.wrapping_sub(1), '[') && punct(i + 1, '(') => {
                let mut at = i + 2;
                while !punct(at, ')') && at < end {
                    let name = ident(at);
                    if !(punct(at, ',') || name.is_some_and(|name| DERIVES.contains(&name))) {
                        found.push(format!("derives by {:?}", toks[at]));
                    }
                    at += 1;
                }
            }
            "path" if punct(i.wrapping_sub(1), '[') => {
                found.push("a `#[path]` attribute".to_string());
            }
            // `cfg_attr` applies any attribute it names, `path` included, so
            // no attribute can be told harmless from its first word.
            "cfg_attr" => found.push("a `cfg_attr` attribute".to_string()),
            _ => {}
        }
    }
    if flag_definitions > 1 {
        found.push("defines `O_NONBLOCK` more than once".to_string());
    }
    found
}

#[test]
fn the_reach_check_sees_through_literals_spacing_and_renames() {
    for (source, at_root) in [
        ("use crate :: /* x */ tabs;", false),
        ("use crate\n    ::tabs;", false),
        ("use crate as root; fn f() { root::tabs::g() }", false),
        ("use super::{super::tabs};", false),
        ("use super::super as up;", false),
        ("use super::super::tabs;", false),
        ("use super::tabs;", true),
        ("use std as s;", false),
        ("use std::net::TcpStream;", false),
        ("use std::{net};", false),
        ("use ::std::process::Command;", false),
        ("use core::net::Ipv4Addr;", false),
        ("use std::os::unix::net::UnixStream;", false),
        ("use std::os::fd::FromRawFd;", false),
        ("use std::os::{unix::fs};", false),
        ("use self::std::fs::File;", false),
        (
            "fn f() { super::std::fs::remove_file(\"x\").unwrap(); }",
            false,
        ),
        ("use crate::store::core::X;", false),
        ("use std::os::unix::fs::symlink;", false),
        ("use std::os::unix::fs::{OpenOptionsExt, symlink};", false),
        ("use std::os::unix::fs;", false),
        (
            "fn f() -> &'static str { return r\"\\\"; } use crate::tabs;",
            false,
        ),
        (
            "fn f() { let _ = br\"\\\"; crate::tabs::g(); let _ = b'\"'; }",
            false,
        ),
        ("fn f() { let _ = cr#\"x\"#; crate::tabs::g(); }", false),
        ("fn f() { let _ = '\\''; crate::tabs::g(); }", false),
        ("fn f<'a>(x: &'a u8) { crate::tabs::g(x) }", false),
        ("use r#crate::tabs;", false),
        ("use super::\u{200E}super::tabs;", false),
        ("use super::{\u{200F}super::tabs as t};", false),
        ("use super::{json, super::tabs};", false),
        ("use super::{{super::tabs}};", false),
        ("use super::{json, {super::tabs as t}};", false),
        ("use {super::super::tabs};", false),
        ("use super::{json::{Json, {super::super::tabs}}};", false),
        ("use super::{json::{self, Json}, super::tabs};", false),
        ("fn f() { m\u{200E}!() }", false),
        ("fn f() { let x\u{301} = 1; }", false),
        ("fn f() { let caf\u{e9} = 1; }", false),
        ("fn f() { let x = 1\u{e9}; }", false),
        ("fn f() { let x\u{b2} = 1; }", false),
        ("fn f<'caf\u{e9}>(x: &'caf\u{e9} u8) {}", false),
        ("extern crate evreos_net;", false),
        ("fn f() { evreos_net::connect() }", false),
        ("fn f() { some_macro!() }", false),
        ("fn f() { if !some_macro![] {} }", false),
        ("fn f() { r#match!() }", false),
        ("fn f() -> String { r#format!(\"x\") }", false),
        (
            "use std::io::Write; fn f(mut o: std::fs::File) { write!(o, \"x\").unwrap(); }",
            false,
        ),
        (
            "use std::io::Write as _; fn f(mut o: std::fs::File) { let _ = writeln!(o); }",
            false,
        ),
        (
            "fn f(mut o: std::fs::File) { o.write_fmt(format_args!(\"x\")).ok(); }",
            false,
        ),
        (
            "fn f(o: &std::fs::File) { std::io::Write::write_vectored(o, &[]).ok(); }",
            false,
        ),
        (
            "fn f(p: &std::path::Path) { crate::store::BookmarkStore::open(p); }",
            false,
        ),
        (
            "use crate::store::HistoryStore; fn f(p: &std::path::Path) { HistoryStore::try_open(p).ok(); }",
            false,
        ),
        (
            "fn f() { <crate::store::StoreRegistry>::open(\"x\"); }",
            false,
        ),
        (
            "use crate::store::StoreRegistry as Database; fn f() { Database::open(\"x\"); }",
            false,
        ),
        (
            "type Database = crate::store::StoreRegistry; fn f() { Database::open(\"x\"); }",
            false,
        ),
        ("fn f(a: &[u8]) { let _ = Database::open(a, None); }", false),
        (
            "fn f<Database: crate::store::Store>() { Database::try_open(\"x\"); }",
            false,
        ),
        ("use crate::store::hook; #[hook] fn f() {}", false),
        ("#[crate::store::hook] fn f() {}", false),
        ("#[r#inline] fn f() {}", false),
        ("#![crate::store::hook]", false),
        (
            "use crate::store::Hook; #[core::prelude::v1::derive(Hook)] struct S;",
            false,
        ),
        (
            "use crate::store::hook as inline; #[inline] fn f() {}",
            false,
        ),
        ("use self::{std::fs::File};", false),
        ("use super::{std::fs::File};", true),
        ("use crate::store::{core::X};", false),
        ("use crate::store::{Y, std::X};", false),
        ("use {alloc::string::String};", false),
        ("use crate::store::format;", false),
        ("use crate::store::{format};", false),
        ("use crate::store::{Store, format as f};", false),
        ("fn f() -> String { crate::store::format!(\"x\") }", false),
        ("use crate::store::fmt as format;", false),
        ("#[derive(Debug, Serialize)] struct S;", false),
        ("#[derive(Debug, serde::Serialize)] struct S;", false),
        ("use crate::store::Debug; #[derive(Debug)] struct S;", false),
        (
            "use crate::store::Hook as Clone; #[derive(Clone)] struct S;",
            false,
        ),
        ("include!(\"../x.rs\");", false),
        ("#[path = \"../x.rs\"] mod x;", true),
        (
            "#[cfg_attr(all(), path = \"../outside.rs\")] mod json;",
            true,
        ),
        ("#[cfg_attr(unix, allow(dead_code))] fn f() {}", false),
        ("mod elsewhere;", true),
        ("mod json;", false),
        ("fn f() { std::fs::write(\"x\", b\"y\").unwrap(); }", false),
        (
            "use std::fs::{write}; fn f() { write(\"x\", b\"\").unwrap(); }",
            false,
        ),
        (
            "fn f(o: &mut std::fs::OpenOptions) { o.write(true); }",
            false,
        ),
        ("fn f() { std::fs::File::create(\"x\").unwrap(); }", false),
        ("fn f() { std::fs::remove_file(\"x\").unwrap(); }", false),
        (
            "fn f(file: &mut std::fs::File) { file.set_len(0).unwrap(); }",
            false,
        ),
        (
            "use std::fs::*; fn f() { write(\"x\", b\"y\").unwrap(); }",
            false,
        ),
        (
            "use std::fs; use fs::*; fn f() { copy(\"a\", \"b\").unwrap(); }",
            false,
        ),
        ("use std::io::*;", false),
        (
            "use std::fs::{File, *}; fn f() { write(\"x\", b\"y\").unwrap(); }",
            false,
        ),
        ("use std::fs::{*};", false),
        ("use std::io::{self, *};", false),
        ("use super::{*};", false),
        (
            "fn f(o: &mut std::fs::OpenOptions) { o.custom_flags(0o100 | 0o1000); }",
            false,
        ),
        (
            "fn f(o: &mut std::fs::OpenOptions) { o.custom_flags(O_NONBLOCK | 0o100); }",
            false,
        ),
        (
            "fn f(o: &mut std::fs::OpenOptions) { o.mode(0o755); }",
            false,
        ),
        (
            "const O_NONBLOCK: i32 = 0o1100; fn f(o: &mut OpenOptions) { o.custom_flags(O_NONBLOCK); }",
            false,
        ),
        (
            "mod flags { pub const O_NONBLOCK: i32 = 0o1100; } use flags::O_NONBLOCK;",
            false,
        ),
        (
            "fn f(o: &mut OpenOptions, O_NONBLOCK: i32) { o.custom_flags(O_NONBLOCK); }",
            false,
        ),
        (
            "fn f(o: &mut OpenOptions) { let O_NONBLOCK = 0o1100; o.custom_flags(O_NONBLOCK); }",
            false,
        ),
        (
            "fn f(o: &mut OpenOptions) { OpenOptionsExt::mode(o, 0o777); }",
            false,
        ),
        (
            "fn f(file: &std::fs::File) { file.set_modified(std::time::SystemTime::now()).unwrap(); }",
            false,
        ),
        ("use super::*;", false),
        (
            "#[cfg(test)]\nmod tests { #[test] fn t() {} }\nfn leak() { crate::tabs::g() }",
            false,
        ),
    ] {
        assert!(
            !reach_violations(source, at_root).is_empty(),
            "{source:?} passed"
        );
    }
    for (source, at_root) in [
        ("use crate::store::{BookmarkStore};", true),
        ("use super::store::HistoryStore;", true),
        ("pub(crate) fn f() {} pub(super) fn g() {}", false),
        ("pub(in crate) fn f() {}", false),
        ("fn f() { use std::os::unix::fs::OpenOptionsExt; }", false),
        (
            "fn f(o: &mut std::fs::OpenOptions) { o.custom_flags(O_NONBLOCK); }",
            false,
        ),
        ("fn f() -> u8 { super::g() }", false),
        ("use ::std::fs::File;", false),
        ("pub use ::core::fmt::Display;", false),
        (
            "fn f(a: &[u8]) { let _ = Database::from_bytes(a, None); }",
            false,
        ),
        (
            "fn g<T: super::snapshot::FileSource>(x: super::json::Json) -> u8 { 0 }",
            false,
        ),
        ("fn h<T>() where T: super::snapshot::FileSource {}", false),
        (
            "fn f() -> u8 { if true { super::g() } else { super::h() } }",
            false,
        ),
        ("use super::{json::{self, Json}, sqlite::Value};", false),
        (
            "use std::path::{Path, PathBuf}; fn f() -> String { format!(\"{}\", 1) }",
            false,
        ),
        (
            "fn f<'a>(x: &'a str) -> char { let _ = \"crate::tabs\"; 'x' }",
            false,
        ),
        ("mod json;", true),
        (
            "fn f(f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, \"x\") }",
            false,
        ),
        ("fn f(v: &mut Vec<u8>) { v.truncate(3); }", false),
        (
            "fn f(a: bool) -> bool { if !a { return !a; } cfg!(windows) }",
            false,
        ),
        (
            "use crate::store::X;\n#[cfg(test)]\nmod tests { use super::*; include_bytes!(\"f\"); }",
            true,
        ),
    ] {
        assert_eq!(
            reach_violations(source, at_root),
            Vec::<String>::new(),
            "{source:?}"
        );
    }
}

#[test]
fn no_macro_in_the_crate_takes_a_name_the_import_may_invoke() {
    // The import's macros are allowed by name. A macro given one of their
    // names anywhere in this crate, or macros brought in by `#[macro_use]`,
    // could stand in for one of them inside the import, so neither exists.
    fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                sources(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    assert!(!files.is_empty());
    for file in files {
        let toks = tokens(&fs::read_to_string(&file).unwrap());
        for (i, tok) in toks.iter().enumerate() {
            let word = match tok {
                Tok::Ident(word) => word.strip_prefix("r#").unwrap_or(word),
                _ => continue,
            };
            assert_ne!(word, "macro_use", "{}", file.display());
            // A macro can take one of those names, a derive's or an
            // attribute's, by being defined under it, by `macro_rules!` or
            // `macro`, or by being renamed to it with `as`, as a re-export
            // would be.
            let named = if word == "macro_rules" && toks.get(i + 1) == Some(&Tok::Punct('!')) {
                toks.get(i + 2)
            } else if word == "macro" || word == "as" {
                toks.get(i + 1)
            } else {
                None
            };
            if let Some(Tok::Ident(name)) = named {
                let name = name.strip_prefix("r#").unwrap_or(name);
                assert!(
                    !MACROS.contains(&name)
                        && !DERIVES.contains(&name)
                        && !ATTRIBUTES.contains(&name),
                    "{} gives a macro the name `{name}`, which the import may invoke",
                    file.display()
                );
            }
        }
    }
}

#[test]
fn the_import_names_no_egress_crate_and_reaches_only_the_stores() {
    // Reading another browser's files is the local computation FR-007a
    // permits; the import must hold no route by which any of it could leave.
    // The crate as a whole depends on evreos-net, so what is asserted is the
    // module's own reach, token by token rather than by text, as the module
    // documentation of `import.rs` lists it and `reach_violations` enforces
    // it, one rule at a time.
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = vec![(src.join("import.rs"), true)];
    for entry in fs::read_dir(src.join("import")).unwrap() {
        files.push((entry.unwrap().path(), false));
    }
    assert!(files.len() >= 6, "import.rs and its five modules");
    for (file, at_root) in files {
        let defines_flag = file.file_name().is_some_and(|name| name == "snapshot.rs");
        let violations =
            reach_violations_in(&fs::read_to_string(&file).unwrap(), at_root, defines_flag);
        assert!(violations.is_empty(), "{}: {violations:?}", file.display());
    }
}
