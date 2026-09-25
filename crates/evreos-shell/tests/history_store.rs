//! Integration tests for the history store.
//!
//! Asserts survival across restart, that a deleted entry and a deleted range
//! never reappear, that a private window produces no entry, and that no
//! secondary journal or undo log is retained on disk.

#![forbid(unsafe_code)]

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, UNIX_EPOCH};

use evreos_shell::store::history::{HistoryEntryId, HistorySource, HistoryStore, WindowKind};

static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

fn unique_temp_dir() -> PathBuf {
    let count = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("evreos_test_history_{pid}_{count}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("failed to create temporary profile dir");
    dir
}

#[test]
fn survival_across_restart() {
    let dir = unique_temp_dir();

    let t1 = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let t2 = UNIX_EPOCH + Duration::from_secs(1_700_000_100);

    {
        let mut store = HistoryStore::open(&dir);
        assert!(store.is_empty());
        assert_eq!(store.count(), 0);

        let id1 = store
            .record_with_details(
                "https://example.com/first",
                "First Page",
                t1,
                HistorySource::Navigated,
                WindowKind::Normal,
            )
            .expect("recording first entry succeeds")
            .expect("normal window produces an entry");

        let id2 = store
            .record_with_details(
                "https://example.org/second",
                "Second Page",
                t2,
                HistorySource::Imported {
                    browser: "ExternalBrowser".to_string(),
                },
                WindowKind::Normal,
            )
            .expect("recording second entry succeeds")
            .expect("normal window produces an entry");

        assert_eq!(store.count(), 2);
        assert_eq!(id1, HistoryEntryId::new(1));
        assert_eq!(id2, HistoryEntryId::new(2));

        let review = store.review();
        assert_eq!(review.len(), 2);
        // Review must be ordered newest first (t2 before t1)
        assert_eq!(review[0].entry_id(), id2);
        assert_eq!(review[0].address(), "https://example.org/second");
        assert_eq!(review[0].title(), "Second Page");
        assert_eq!(
            review[0].source(),
            &HistorySource::Imported {
                browser: "ExternalBrowser".to_string()
            }
        );

        assert_eq!(review[1].entry_id(), id1);
        assert_eq!(review[1].address(), "https://example.com/first");
        assert_eq!(review[1].title(), "First Page");
        assert_eq!(review[1].source(), &HistorySource::Navigated);
    }

    // Restart: reopen the store from disk
    {
        let store = HistoryStore::open(&dir);
        assert_eq!(store.count(), 2);

        let review = store.review();
        assert_eq!(review.len(), 2);
        assert_eq!(review[0].entry_id(), HistoryEntryId::new(2));
        assert_eq!(review[0].address(), "https://example.org/second");
        assert_eq!(review[0].title(), "Second Page");
        assert_eq!(review[0].visited_at(), t2);
        assert_eq!(
            review[0].source(),
            &HistorySource::Imported {
                browser: "ExternalBrowser".to_string()
            }
        );

        assert_eq!(review[1].entry_id(), HistoryEntryId::new(1));
        assert_eq!(review[1].address(), "https://example.com/first");
        assert_eq!(review[1].title(), "First Page");
        assert_eq!(review[1].visited_at(), t1);
        assert_eq!(review[1].source(), &HistorySource::Navigated);
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn private_window_produces_no_entry() {
    let dir = unique_temp_dir();

    {
        let mut store = HistoryStore::open(&dir);

        // Recording in a private window produces no entry and leaves no trace
        let res = store
            .record(
                "https://secret.example.com/path",
                "Confidential Page",
                WindowKind::Private,
            )
            .expect("recording call succeeds");

        assert_eq!(res, None, "private window must return None");
        assert_eq!(store.count(), 0, "store remains empty in memory");
        assert!(store.review().is_empty());
        assert!(store.search("secret").is_empty());

        // File should either not exist or have zero entries
        let file_path = store.file_path();
        if file_path.exists() {
            let content = fs::read_to_string(&file_path).expect("read history file");
            assert!(!content.contains("secret.example.com"));
        }

        // Record a normal navigation
        let normal_id = store
            .record(
                "https://public.example.com/home",
                "Public Home",
                WindowKind::Normal,
            )
            .expect("normal navigation succeeds")
            .expect("normal navigation produces an entry ID");

        assert_eq!(store.count(), 1);

        // Record another private navigation
        let private_res2 = store
            .record(
                "https://private.example.com/checkout",
                "Private Checkout",
                WindowKind::Private,
            )
            .expect("recording call succeeds");
        assert_eq!(private_res2, None);
        assert_eq!(store.count(), 1);

        let entries = store.review();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].entry_id(), normal_id);
        assert_eq!(entries[0].address(), "https://public.example.com/home");
    }

    // Reopen store from disk and assert private navigations left no residue
    {
        let store = HistoryStore::open(&dir);
        assert_eq!(store.count(), 1);
        let review = store.review();
        assert_eq!(review.len(), 1);
        assert_eq!(review[0].address(), "https://public.example.com/home");

        let content = fs::read_to_string(store.file_path()).expect("read history file");
        assert!(!content.contains("secret.example.com"));
        assert!(!content.contains("private.example.com"));
        assert!(!content.contains("Confidential Page"));
        assert!(!content.contains("Private Checkout"));
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn deleted_entry_never_reappears() {
    let dir = unique_temp_dir();

    let t1 = UNIX_EPOCH + Duration::from_secs(100);
    let t2 = UNIX_EPOCH + Duration::from_secs(200);
    let t3 = UNIX_EPOCH + Duration::from_secs(300);

    let id_a;
    let id_b;
    let id_c;

    {
        let mut store = HistoryStore::open(&dir);

        id_a = store
            .record_with_details(
                "https://site-a.example/item",
                "Site A",
                t1,
                HistorySource::Navigated,
                WindowKind::Normal,
            )
            .unwrap()
            .unwrap();
        id_b = store
            .record_with_details(
                "https://site-b.example/item",
                "Site B",
                t2,
                HistorySource::Navigated,
                WindowKind::Normal,
            )
            .unwrap()
            .unwrap();
        id_c = store
            .record_with_details(
                "https://site-c.example/item",
                "Site C",
                t3,
                HistorySource::Navigated,
                WindowKind::Normal,
            )
            .unwrap()
            .unwrap();

        assert_eq!(store.count(), 3);

        // Search finds site B before deletion
        let results_before = store.search("site-b");
        assert_eq!(results_before.len(), 1);
        assert_eq!(results_before[0].entry_id(), id_b);

        // Delete single entry B
        let deleted = store.delete_entry(id_b).expect("delete succeeds");
        assert!(deleted, "entry was found and deleted");
        assert_eq!(store.count(), 2);

        // Verify in-memory state
        assert_eq!(store.get(id_b), None);
        assert!(store.search("site-b").is_empty());
        let remaining_ids: Vec<HistoryEntryId> =
            store.review().into_iter().map(|e| e.entry_id()).collect();
        assert_eq!(remaining_ids, vec![id_c, id_a]);

        // Attempting to delete already deleted entry returns false
        let delete_again = store.delete_entry(id_b).expect("delete succeeds");
        assert!(!delete_again, "already deleted entry returns false");
    }

    // Reopen store from disk: verify B never reappears
    {
        let mut store = HistoryStore::open(&dir);
        assert_eq!(store.count(), 2);
        assert_eq!(store.get(id_b), None);
        assert!(store.search("site-b").is_empty());

        let remaining: Vec<HistoryEntryId> =
            store.review().into_iter().map(|e| e.entry_id()).collect();
        assert_eq!(remaining, vec![id_c, id_a]);

        let content = fs::read_to_string(store.file_path()).expect("read history file");
        assert!(!content.contains("site-b.example"));
        assert!(!content.contains("Site B"));

        // Delete entry A as well
        let deleted_a = store.delete_entry(id_a).expect("delete succeeds");
        assert!(deleted_a);
        assert_eq!(store.count(), 1);
    }

    // Reopen again: only C survives
    {
        let store = HistoryStore::open(&dir);
        assert_eq!(store.count(), 1);
        assert_eq!(store.review()[0].entry_id(), id_c);
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn deleted_range_never_reappears() {
    let dir = unique_temp_dir();

    let t1 = UNIX_EPOCH + Duration::from_secs(1_000);
    let t2 = UNIX_EPOCH + Duration::from_secs(2_000);
    let t3 = UNIX_EPOCH + Duration::from_secs(3_000);
    let t4 = UNIX_EPOCH + Duration::from_secs(4_000);
    let t5 = UNIX_EPOCH + Duration::from_secs(5_000);

    {
        let mut store = HistoryStore::open(&dir);

        store
            .record_with_details(
                "https://range.example/1",
                "Entry 1",
                t1,
                HistorySource::Navigated,
                WindowKind::Normal,
            )
            .unwrap();
        store
            .record_with_details(
                "https://range.example/2",
                "Entry 2",
                t2,
                HistorySource::Navigated,
                WindowKind::Normal,
            )
            .unwrap();
        store
            .record_with_details(
                "https://range.example/3",
                "Entry 3",
                t3,
                HistorySource::Navigated,
                WindowKind::Normal,
            )
            .unwrap();
        store
            .record_with_details(
                "https://range.example/4",
                "Entry 4",
                t4,
                HistorySource::Navigated,
                WindowKind::Normal,
            )
            .unwrap();
        store
            .record_with_details(
                "https://range.example/5",
                "Entry 5",
                t5,
                HistorySource::Navigated,
                WindowKind::Normal,
            )
            .unwrap();

        assert_eq!(store.count(), 5);

        // Delete time range [t2, t4] inclusive
        let removed = store.delete_range(t2, t4).expect("delete_range succeeds");
        assert_eq!(removed, 3, "should delete entries 2, 3, 4");
        assert_eq!(store.count(), 2);

        let remaining = store.review();
        assert_eq!(remaining.len(), 2);
        assert_eq!(remaining[0].title(), "Entry 5");
        assert_eq!(remaining[1].title(), "Entry 1");
    }

    // Reopen store from disk: verify deleted range never reappears
    {
        let store = HistoryStore::open(&dir);
        assert_eq!(store.count(), 2);

        let remaining = store.review();
        assert_eq!(remaining.len(), 2);
        assert_eq!(remaining[0].title(), "Entry 5");
        assert_eq!(remaining[1].title(), "Entry 1");

        let content = fs::read_to_string(store.file_path()).expect("read history file");
        assert!(!content.contains("Entry 2"));
        assert!(!content.contains("Entry 3"));
        assert!(!content.contains("Entry 4"));
        assert!(!content.contains("range.example/2"));
        assert!(!content.contains("range.example/3"));
        assert!(!content.contains("range.example/4"));
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn search_and_case_insensitivity() {
    let dir = unique_temp_dir();
    let mut store = HistoryStore::open(&dir);

    store
        .record(
            "https://crates.io/crates/winit",
            "Cross-Platform Window Handling",
            WindowKind::Normal,
        )
        .unwrap();

    store
        .record(
            "https://docs.rs/serde",
            "Serialization Framework",
            WindowKind::Normal,
        )
        .unwrap();

    // Matching title
    let res1 = store.search("window");
    assert_eq!(res1.len(), 1);
    assert_eq!(res1[0].title(), "Cross-Platform Window Handling");

    // Matching address
    let res2 = store.search("docs.rs");
    assert_eq!(res2.len(), 1);
    assert_eq!(res2[0].title(), "Serialization Framework");

    // Case-insensitivity
    let res3 = store.search("CROSS-PLATFORM");
    assert_eq!(res3.len(), 1);

    // Empty search returns empty
    assert!(store.search("").is_empty());
    assert!(store.search("   ").is_empty());

    // Non-matching query
    assert!(store.search("nonexistent-query-xyz").is_empty());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn no_undo_log_or_journal_on_disk() {
    let dir = unique_temp_dir();

    let mut store = HistoryStore::open(&dir);
    let id1 = store
        .record("https://persist.example/1", "Persist 1", WindowKind::Normal)
        .unwrap()
        .unwrap();
    let id2 = store
        .record("https://persist.example/2", "Persist 2", WindowKind::Normal)
        .unwrap()
        .unwrap();

    store.delete_entry(id1).unwrap();
    store.delete_entry(id2).unwrap();

    // Verify files in profile directory
    let entries: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("read dir")
        .filter_map(|e| e.ok().map(|d| d.path()))
        .collect();

    for path in entries {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        assert!(
            name == "history.toml" || name.starts_with('.'),
            "unexpected journal or log file on disk: {name}"
        );
        assert!(!name.contains(".tmp"), "no leftover tmp file: {name}");
        assert!(!name.contains(".journal"), "no journal file: {name}");
        assert!(!name.contains(".undo"), "no undo log file: {name}");
        assert!(!name.contains(".wal"), "no write-ahead log file: {name}");
    }

    let _ = fs::remove_dir_all(&dir);
}
