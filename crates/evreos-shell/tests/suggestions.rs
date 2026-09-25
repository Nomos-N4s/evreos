//! Integration tests for the suggestion index.
//!
//! Asserts that suggestions are drawn exclusively from local sources (history,
//! bookmarks, open tabs), that deletions in history or bookmarks immediately
//! propagate so deleted entries never reappear as suggestions, that deleted time
//! ranges leave none behind, that lookups run off the UI thread on the worker pool,
//! and that evreos-shell maintains no dependency path to evreos-net.

#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, UNIX_EPOCH};

use evreos_shell::store::bookmarks::{BookmarkStore, FolderId};
use evreos_shell::store::history::{HistorySource, HistoryStore, WindowKind};
use evreos_shell::suggest::{OpenTab, SuggestionIndex, SuggestionSource};
use evreos_shell::work::WorkerPool;

static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

fn unique_temp_dir() -> PathBuf {
    let count = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("evreos_test_suggestions_{pid}_{count}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("failed to create temporary profile dir");
    dir
}

#[test]
fn deleted_history_entry_never_reappears_as_suggestion() {
    let dir = unique_temp_dir();
    let mut history = HistoryStore::open(&dir.join("history"));
    let bookmarks = BookmarkStore::open(&dir.join("bookmarks"));
    let open_tabs = Vec::new();

    let id = history
        .record(
            "https://rust-lang.org/learn",
            "Learn Rust Programming Language",
            WindowKind::Normal,
        )
        .unwrap()
        .expect("must produce id");

    // Live query returns the suggestion
    let suggestions = SuggestionIndex::query(&history, &bookmarks, &open_tabs, "rust");
    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0].address(), "https://rust-lang.org/learn");
    assert_eq!(suggestions[0].source(), SuggestionSource::History);

    // Delete the entry from the history store
    let deleted = history.delete_entry(id).expect("delete succeeds");
    assert!(deleted);

    // Live suggestion index query immediately reflects deletion
    let after_delete = SuggestionIndex::query(&history, &bookmarks, &open_tabs, "rust");
    assert!(
        after_delete.is_empty(),
        "deleted history entry must never reappear as suggestion"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn deleted_bookmark_never_reappears_as_suggestion() {
    let dir = unique_temp_dir();
    let history = HistoryStore::open(&dir.join("history"));
    let mut bookmarks = BookmarkStore::open(&dir.join("bookmarks"));
    let open_tabs = Vec::new();

    let bm_id = bookmarks
        .create_bookmark(
            FolderId::ROOT,
            "Crates.io Rust Package Registry",
            "https://crates.io/crates/winit",
        )
        .expect("create bookmark");

    // Suggestion query returns the bookmark
    let suggestions = SuggestionIndex::query(&history, &bookmarks, &open_tabs, "winit");
    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0].address(), "https://crates.io/crates/winit");
    assert_eq!(suggestions[0].source(), SuggestionSource::Bookmark);

    // Delete the bookmark
    bookmarks.delete_bookmark(bm_id).expect("delete bookmark");

    // Assert deletion immediately propagates to suggestions
    let after_delete = SuggestionIndex::query(&history, &bookmarks, &open_tabs, "winit");
    assert!(
        after_delete.is_empty(),
        "deleted bookmark must never reappear as suggestion"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn deleted_time_range_leaves_no_suggestion_behind() {
    let dir = unique_temp_dir();
    let mut history = HistoryStore::open(&dir.join("history"));
    let bookmarks = BookmarkStore::open(&dir.join("bookmarks"));
    let open_tabs = Vec::new();

    let base = UNIX_EPOCH + Duration::from_secs(1_000_000);
    let t1 = base + Duration::from_secs(10);
    let t2 = base + Duration::from_secs(20);
    let t3 = base + Duration::from_secs(30);
    let t_outside = base + Duration::from_secs(100);

    history
        .record_with_details(
            "https://example.com/alpha",
            "Alpha Topic",
            t1,
            HistorySource::Navigated,
            WindowKind::Normal,
        )
        .unwrap();

    history
        .record_with_details(
            "https://example.com/beta",
            "Beta Topic",
            t2,
            HistorySource::Navigated,
            WindowKind::Normal,
        )
        .unwrap();

    history
        .record_with_details(
            "https://example.com/gamma",
            "Gamma Topic",
            t3,
            HistorySource::Navigated,
            WindowKind::Normal,
        )
        .unwrap();

    history
        .record_with_details(
            "https://example.com/omega",
            "Omega Topic",
            t_outside,
            HistorySource::Navigated,
            WindowKind::Normal,
        )
        .unwrap();

    // Prior to range deletion, all 4 match
    let before = SuggestionIndex::query(&history, &bookmarks, &open_tabs, "example.com");
    assert_eq!(before.len(), 4);

    // Delete range covering t1, t2, t3 [base, base + 50s]
    let removed = history
        .delete_range(base, base + Duration::from_secs(50))
        .expect("delete range");
    assert_eq!(removed, 3);

    // Query suggestions: alpha, beta, and gamma must not appear; only omega survives
    let after = SuggestionIndex::query(&history, &bookmarks, &open_tabs, "example.com");
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].address(), "https://example.com/omega");

    // Specifically confirm none of the deleted range entries reappear
    for s in &after {
        assert_ne!(s.address(), "https://example.com/alpha");
        assert_ne!(s.address(), "https://example.com/beta");
        assert_ne!(s.address(), "https://example.com/gamma");
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn open_tabs_yield_suggestions_with_top_priority() {
    let dir = unique_temp_dir();
    let mut history = HistoryStore::open(&dir.join("history"));
    let mut bookmarks = BookmarkStore::open(&dir.join("bookmarks"));

    let target_url = "https://docs.rs/winit/latest";

    // 1. In history
    history
        .record(target_url, "History Winit Docs", WindowKind::Normal)
        .unwrap();

    // 2. In bookmarks
    bookmarks
        .create_bookmark(FolderId::ROOT, "Bookmarked Winit Docs", target_url)
        .unwrap();

    // 3. In open tabs
    let open_tabs = vec![OpenTab::new(target_url, "Live Active Winit Tab")];

    // Query: should deduplicate and return OpenTab as top priority
    let suggestions = SuggestionIndex::query(&history, &bookmarks, &open_tabs, "winit");
    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0].address(), target_url);
    assert_eq!(suggestions[0].source(), SuggestionSource::OpenTab);
    assert_eq!(suggestions[0].title(), "Live Active Winit Tab");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn lookups_executed_off_ui_thread_on_worker_pool() {
    let dir = unique_temp_dir();
    let mut history = HistoryStore::open(&dir.join("history"));
    let bookmarks = BookmarkStore::open(&dir.join("bookmarks"));
    let open_tabs = vec![OpenTab::new(
        "https://crates.io/crates/winit",
        "Winit Windowing Library",
    )];

    history
        .record(
            "https://github.com/rust-windowing/winit",
            "GitHub Repository for Winit",
            WindowKind::Normal,
        )
        .unwrap();

    let mut pool = WorkerPool::<Vec<evreos_shell::suggest::Suggestion>>::new(2, 8);
    let ui_thread_id = thread::current().id();

    // Submit async suggestion lookup to run on worker pool
    let job_id = SuggestionIndex::query_async(
        &pool,
        history.clone(),
        bookmarks.clone(),
        open_tabs,
        "winit",
    )
    .expect("job submission must succeed");

    // Wait for worker thread to complete execution
    let mut results = Vec::new();
    for _ in 0..50 {
        thread::sleep(Duration::from_millis(10));
        let drained = pool.drain_results();
        if !drained.is_empty() {
            results.extend(drained);
            break;
        }
    }

    assert_eq!(results.len(), 1);
    let result = results.remove(0);
    assert_eq!(result.id(), job_id);
    assert!(result.is_success());

    // Verify lookup was executed off the UI thread
    assert_ne!(
        result.worker_thread_id(),
        ui_thread_id,
        "suggestion lookup must run on a background worker thread"
    );
    assert_eq!(
        result.delivery_thread_id(),
        ui_thread_id,
        "results must be delivered back on the UI thread"
    );

    let suggestions = result.into_value().expect("value exists");
    assert_eq!(suggestions.len(), 2);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn no_dependency_path_to_evreos_net() {
    // Build assertion: evreos-shell must have no dependency path to evreos-net
    // (directly or transitively) so that no network suggestion service can exist
    // or be contacted (FR-007a, Principle VI).
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml_path = manifest_dir.join("Cargo.toml");
    let content = fs::read_to_string(&cargo_toml_path).expect("read Cargo.toml");

    assert!(
        !content.contains("evreos-net"),
        "crates/evreos-shell Cargo.toml must not declare evreos-net as a dependency"
    );

    // Verify via cargo tree that evreos-shell has zero direct or transitive dependency on evreos-net
    let output = Command::new("cargo")
        .args(["tree", "-p", "evreos-shell"])
        .current_dir(manifest_dir)
        .output()
        .expect("cargo tree invocation");

    assert!(output.status.success(), "cargo tree must succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !stdout.contains("evreos-net"),
        "evreos-shell must have zero direct or transitive dependency path to evreos-net"
    );
}
