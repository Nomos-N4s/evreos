//! Integration tests for the bookmark and folder store.
//!
//! Asserts survival across restart, the tree invariant (single root, no cycles,
//! all items reachable), cascade deletion of subtrees, and the absence of
//! independent undo logs or journals on disk.

#![forbid(unsafe_code)]

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, UNIX_EPOCH};

use evreos_shell::store::bookmarks::{BookmarkError, BookmarkSource, BookmarkStore, FolderId};

static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

fn unique_temp_dir() -> PathBuf {
    let count = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("evreos_test_bookmarks_{pid}_{count}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("failed to create temporary profile dir");
    dir
}

#[test]
fn survival_across_restart() {
    let dir = unique_temp_dir();
    let t1 = UNIX_EPOCH + Duration::from_secs(1_700_000_100);
    let t2 = UNIX_EPOCH + Duration::from_secs(1_700_000_200);

    let fid_work;
    let fid_dev;
    let bid1;
    let bid2;

    {
        let mut store = BookmarkStore::open(&dir);
        assert_eq!(store.folders().len(), 1, "only root folder on start");
        assert!(store.bookmarks().is_empty());

        fid_work = store
            .create_folder(FolderId::ROOT, "Work")
            .expect("create work folder");
        fid_dev = store
            .create_folder(fid_work, "Development")
            .expect("create dev subfolder");

        bid1 = store
            .create_bookmark_with_details(
                fid_work,
                "Internal Portal",
                "https://internal.example.org",
                t1,
                BookmarkSource::Created,
            )
            .expect("create bookmark in work");

        bid2 = store
            .create_bookmark_with_details(
                fid_dev,
                "API Reference",
                "https://api.example.org/docs",
                t2,
                BookmarkSource::Imported {
                    browser: "Chrome".to_string(),
                },
            )
            .expect("create bookmark in dev");

        assert_eq!(store.folders().len(), 3);
        assert_eq!(store.bookmarks().len(), 2);
    }

    // Reopen store from disk (restart)
    {
        let store = BookmarkStore::open(&dir);
        assert_eq!(store.folders().len(), 3);
        assert_eq!(store.bookmarks().len(), 2);

        let work = store.get_folder(fid_work).expect("work folder exists");
        assert_eq!(work.name(), "Work");
        assert_eq!(work.parent(), Some(FolderId::ROOT));

        let dev = store.get_folder(fid_dev).expect("dev folder exists");
        assert_eq!(dev.name(), "Development");
        assert_eq!(dev.parent(), Some(fid_work));

        let b1 = store.get_bookmark(bid1).expect("bookmark 1 exists");
        assert_eq!(b1.title(), "Internal Portal");
        assert_eq!(b1.address(), "https://internal.example.org");
        assert_eq!(b1.parent(), fid_work);
        assert_eq!(b1.created_at(), t1);
        assert_eq!(b1.source(), &BookmarkSource::Created);

        let b2 = store.get_bookmark(bid2).expect("bookmark 2 exists");
        assert_eq!(b2.title(), "API Reference");
        assert_eq!(b2.address(), "https://api.example.org/docs");
        assert_eq!(b2.parent(), fid_dev);
        assert_eq!(b2.created_at(), t2);
        assert_eq!(
            b2.source(),
            &BookmarkSource::Imported {
                browser: "Chrome".to_string()
            }
        );

        // Subfolder query
        let subfolders = store.subfolders(fid_work);
        assert_eq!(subfolders.len(), 1);
        assert_eq!(subfolders[0].id(), fid_dev);

        // Bookmarks in folder query
        let work_bms = store.bookmarks_in_folder(fid_work);
        assert_eq!(work_bms.len(), 1);
        assert_eq!(work_bms[0].id(), bid1);
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn tree_invariant_and_cycle_rejection() {
    let dir = unique_temp_dir();
    let mut store = BookmarkStore::open(&dir);

    // Root folder cannot be deleted or reparented
    assert!(matches!(
        store.delete_folder(FolderId::ROOT),
        Err(BookmarkError::CannotModifyRoot)
    ));
    assert!(matches!(
        store.move_folder(FolderId::ROOT, FolderId::new(1)),
        Err(BookmarkError::CannotModifyRoot)
    ));

    // Construct hierarchy: Root -> A -> B -> C
    let f_a = store.create_folder(FolderId::ROOT, "Folder A").unwrap();
    let f_b = store.create_folder(f_a, "Folder B").unwrap();
    let f_c = store.create_folder(f_b, "Folder C").unwrap();

    // Verify tree is valid
    assert!(store.validate_tree().is_ok());

    // Cycle check 1: moving folder into itself
    let cycle_self = store.move_folder(f_a, f_a);
    assert!(matches!(cycle_self, Err(BookmarkError::CycleDetected(id)) if id == f_a));

    // Cycle check 2: moving A into its direct child B
    let cycle_child = store.move_folder(f_a, f_b);
    assert!(matches!(cycle_child, Err(BookmarkError::CycleDetected(id)) if id == f_a));

    // Cycle check 3: moving A into its descendant C
    let cycle_descendant = store.move_folder(f_a, f_c);
    assert!(matches!(cycle_descendant, Err(BookmarkError::CycleDetected(id)) if id == f_a));

    // Tree invariant must remain intact after all rejected operations
    assert!(store.validate_tree().is_ok());

    // Valid move: move C up to Root
    store.move_folder(f_c, FolderId::ROOT).expect("valid move");
    assert_eq!(
        store.get_folder(f_c).unwrap().parent(),
        Some(FolderId::ROOT)
    );
    assert!(store.validate_tree().is_ok());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn cascade_deletion_erases_entire_subtree() {
    let dir = unique_temp_dir();

    let f_work;
    let f_sub;
    let b_work;
    let b_sub;
    let b_other;

    {
        let mut store = BookmarkStore::open(&dir);

        f_work = store.create_folder(FolderId::ROOT, "Work").unwrap();
        f_sub = store.create_folder(f_work, "Subproject").unwrap();

        b_work = store
            .create_bookmark(f_work, "Work Home", "https://work.example")
            .unwrap();
        b_sub = store
            .create_bookmark(f_sub, "Subproject Ticket", "https://sub.example")
            .unwrap();
        b_other = store
            .create_bookmark(FolderId::ROOT, "General Root", "https://root.example")
            .unwrap();

        assert_eq!(store.folders().len(), 3);
        assert_eq!(store.bookmarks().len(), 3);

        // Delete folder Work: must cascade to Subproject, b_work, and b_sub
        let deleted_count = store.delete_folder(f_work).expect("delete work succeeds");
        // 2 folders (Work, Subproject) + 2 bookmarks = 4 items
        assert_eq!(deleted_count, 4);

        assert_eq!(store.folders().len(), 1, "only root remains");
        assert_eq!(store.bookmarks().len(), 1, "only b_other remains");
        assert_eq!(store.get_folder(f_work), None);
        assert_eq!(store.get_folder(f_sub), None);
        assert_eq!(store.get_bookmark(b_work), None);
        assert_eq!(store.get_bookmark(b_sub), None);
        assert_eq!(store.get_bookmark(b_other).unwrap().title(), "General Root");

        // Validate tree is clean
        assert!(store.validate_tree().is_ok());
    }

    // Reopen store from disk: verify deleted items never reappear
    {
        let store = BookmarkStore::open(&dir);
        assert_eq!(store.folders().len(), 1);
        assert_eq!(store.bookmarks().len(), 1);
        assert_eq!(store.get_folder(f_work), None);
        assert_eq!(store.get_folder(f_sub), None);
        assert_eq!(store.get_bookmark(b_work), None);
        assert_eq!(store.get_bookmark(b_sub), None);

        let content = fs::read_to_string(store.file_path()).expect("read bookmarks.toml");
        assert!(!content.contains("work.example"));
        assert!(!content.contains("sub.example"));
        assert!(!content.contains("Work Home"));
        assert!(!content.contains("Subproject"));
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rename_move_and_search() {
    let dir = unique_temp_dir();
    let mut store = BookmarkStore::open(&dir);

    let fid = store.create_folder(FolderId::ROOT, "Old Folder").unwrap();
    let bid = store
        .create_bookmark(fid, "Old Title", "https://example.com/page")
        .unwrap();

    // Renaming
    store.rename_folder(fid, "New Folder").unwrap();
    store.rename_bookmark(bid, "New Title").unwrap();

    assert_eq!(store.get_folder(fid).unwrap().name(), "New Folder");
    assert_eq!(store.get_bookmark(bid).unwrap().title(), "New Title");

    // Search by title and address
    assert_eq!(store.search("new title").len(), 1);
    assert_eq!(store.search("example.com").len(), 1);
    assert!(store.search("nonexistent").is_empty());

    // Single bookmark deletion
    let deleted = store.delete_bookmark(bid).unwrap();
    assert!(deleted);
    assert_eq!(store.get_bookmark(bid), None);
    assert!(store.bookmarks().is_empty());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn no_undo_log_or_journal_on_disk() {
    let dir = unique_temp_dir();
    let mut store = BookmarkStore::open(&dir);

    let fid = store.create_folder(FolderId::ROOT, "Temp").unwrap();
    let bid = store
        .create_bookmark(fid, "T", "https://t.example")
        .unwrap();
    store.delete_bookmark(bid).unwrap();
    store.delete_folder(fid).unwrap();

    let entries: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("read dir")
        .filter_map(|e| e.ok().map(|d| d.path()))
        .collect();

    for path in entries {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        assert!(
            name == "bookmarks.toml" || name.starts_with('.'),
            "unexpected file on disk: {name}"
        );
        assert!(!name.contains(".tmp"), "no leftover tmp file: {name}");
        assert!(!name.contains(".journal"), "no journal file: {name}");
        assert!(!name.contains(".undo"), "no undo log file: {name}");
        assert!(!name.contains(".wal"), "no write-ahead log: {name}");
    }

    let _ = fs::remove_dir_all(&dir);
}
