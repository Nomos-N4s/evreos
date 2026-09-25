//! Integration tests for the download store.
//!
//! Asserts survival across restart, cancel and remove behaviours,
//! that the destination path is present on every entry, and the absence
//! of undo logs or secondary journals on disk.

#![forbid(unsafe_code)]

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use evreos_shell::store::downloads::{DownloadError, DownloadId, DownloadState, DownloadStore};

static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

fn unique_temp_dir() -> PathBuf {
    let count = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("evreos_test_downloads_{pid}_{count}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("failed to create temporary profile dir");
    dir
}

#[test]
fn survival_across_restart() {
    let dir = unique_temp_dir();
    let dl1 = DownloadId::new(101);
    let dl2 = DownloadId::new(102);

    let dest1 = dir.join("archive.zip");
    let dest2 = dir.join("document.pdf");

    {
        let mut store = DownloadStore::open(&dir);
        assert!(store.is_empty());
        assert_eq!(store.count(), 0);

        // Record an in-progress download
        store
            .start_download(
                dl1,
                "https://example.org/files/archive.zip",
                &dest1,
                Some(10_000_000),
            )
            .expect("start dl1");

        store
            .update_progress(dl1, 3_500_000)
            .expect("progress on dl1");

        // Record another download and complete it
        store
            .start_download(
                dl2,
                "https://example.org/docs/document.pdf",
                &dest2,
                Some(250_000),
            )
            .expect("start dl2");

        store.complete_download(dl2).expect("complete dl2");

        assert_eq!(store.count(), 2);
    }

    // Reopen store from same directory (simulating browser restart)
    {
        let store = DownloadStore::open(&dir);
        assert_eq!(store.count(), 2);

        let entry1 = store.get(dl1).expect("dl1 must survive restart");
        assert_eq!(entry1.id(), dl1);
        assert_eq!(
            entry1.source_address(),
            "https://example.org/files/archive.zip"
        );
        assert_eq!(entry1.destination_path(), dest1.as_path());
        assert_eq!(entry1.bytes_total(), Some(10_000_000));
        assert_eq!(entry1.bytes_received(), 3_500_000);
        assert_eq!(entry1.state(), DownloadState::InProgress);
        assert!(entry1.is_in_progress());
        assert!(entry1.finished_at().is_none());

        let entry2 = store.get(dl2).expect("dl2 must survive restart");
        assert_eq!(entry2.id(), dl2);
        assert_eq!(
            entry2.source_address(),
            "https://example.org/docs/document.pdf"
        );
        assert_eq!(entry2.destination_path(), dest2.as_path());
        assert_eq!(entry2.bytes_total(), Some(250_000));
        assert_eq!(entry2.bytes_received(), 250_000);
        assert_eq!(entry2.state(), DownloadState::Completed);
        assert!(entry2.is_completed());
        assert!(entry2.finished_at().is_some());
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn destination_path_present_on_every_entry() {
    let dir = unique_temp_dir();
    let mut store = DownloadStore::open(&dir);

    let id1 = store.allocate_id();
    let id2 = store.allocate_id();
    let id3 = store.allocate_id();
    let id4 = store.allocate_id();

    let dest1 = dir.join("file1.dat");
    let dest2 = dir.join("file2.dat");
    let dest3 = dir.join("file3.dat");
    let dest4 = dir.join("file4.dat");

    store
        .start_download(id1, "https://a.test/1", &dest1, None)
        .unwrap();
    store
        .start_download(id2, "https://a.test/2", &dest2, Some(100))
        .unwrap();
    store
        .start_download(id3, "https://a.test/3", &dest3, Some(200))
        .unwrap();
    store
        .start_download(id4, "https://a.test/4", &dest4, Some(300))
        .unwrap();

    store.complete_download(id2).unwrap();
    store.cancel_download(id3).unwrap();
    store.fail_download(id4).unwrap();

    // Verify every entry carries a valid, non-empty destination path on disk
    for entry in store.entries() {
        assert!(
            !entry.destination_path().as_os_str().is_empty(),
            "destination path must not be empty on entry {:?}",
            entry.id()
        );
        assert!(
            entry.destination_path().is_absolute(),
            "destination path must be absolute on entry {:?}",
            entry.id()
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn cancel_in_progress_and_refuse_already_finished() {
    let dir = unique_temp_dir();
    let mut store = DownloadStore::open(&dir);

    let id1 = store.allocate_id();
    let id2 = store.allocate_id();
    let dest1 = dir.join("cancel_test.bin");
    let dest2 = dir.join("complete_test.bin");

    store
        .start_download(id1, "https://example.com/c1", &dest1, Some(5000))
        .unwrap();
    store
        .start_download(id2, "https://example.com/c2", &dest2, Some(6000))
        .unwrap();

    // Cancel in-progress download
    store.cancel_download(id1).expect("cancel should succeed");
    let entry1 = store.get(id1).unwrap();
    assert_eq!(entry1.state(), DownloadState::Cancelled);
    assert!(entry1.is_cancelled());
    assert!(entry1.finished_at().is_some());

    // Attempting to cancel again should fail with AlreadyFinished
    match store.cancel_download(id1) {
        Err(DownloadError::AlreadyFinished(err_id)) => assert_eq!(err_id, id1),
        other => panic!("expected AlreadyFinished error, got: {other:?}"),
    }

    // Complete id2, then attempt to cancel id2
    store.complete_download(id2).unwrap();
    match store.cancel_download(id2) {
        Err(DownloadError::AlreadyFinished(err_id)) => assert_eq!(err_id, id2),
        other => panic!("expected AlreadyFinished error, got: {other:?}"),
    }

    // Cancel non-existent id
    let nonexistent = DownloadId::new(9999);
    match store.cancel_download(nonexistent) {
        Err(DownloadError::NotFound(err_id)) => assert_eq!(err_id, nonexistent),
        other => panic!("expected NotFound error, got: {other:?}"),
    }

    // Restart and assert id1 remains cancelled
    drop(store);
    let reopened = DownloadStore::open(&dir);
    assert_eq!(reopened.get(id1).unwrap().state(), DownloadState::Cancelled);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn remove_from_list_deletes_record_and_never_the_saved_file() {
    let dir = unique_temp_dir();
    let downloads_dir = dir.join("downloads");
    fs::create_dir_all(&downloads_dir).unwrap();

    let file_on_disk = downloads_dir.join("report.pdf");
    let file_content = b"PDF member saved payload: private data";
    fs::write(&file_on_disk, file_content).expect("member saves file");

    let mut store = DownloadStore::open(&dir);
    let dl_id = DownloadId::new(42);

    store
        .start_download(
            dl_id,
            "https://bank.example.org/statements/report.pdf",
            &file_on_disk,
            Some(file_content.len() as u64),
        )
        .unwrap();
    store.complete_download(dl_id).unwrap();

    assert_eq!(store.count(), 1);
    assert!(file_on_disk.exists(), "file exists on disk before remove");

    // Remove download entry from list (FR-004)
    let removed = store
        .remove_from_list(dl_id)
        .expect("remove_from_list should succeed");
    assert!(removed);
    assert_eq!(store.count(), 0);
    assert!(store.get(dl_id).is_none());

    // CRITICAL: The file the member saved on disk MUST NOT be deleted!
    assert!(
        file_on_disk.exists(),
        "file on disk must remain intact after remove_from_list"
    );
    let read_back = fs::read(&file_on_disk).expect("reading member file");
    assert_eq!(read_back, file_content);

    // Verify deletion survives restart and file is still intact
    drop(store);
    let reopened = DownloadStore::open(&dir);
    assert_eq!(reopened.count(), 0);
    assert!(reopened.get(dl_id).is_none());
    assert!(
        file_on_disk.exists(),
        "file on disk survives restart untouched"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn no_undo_log_or_journal_on_disk() {
    let dir = unique_temp_dir();
    let mut store = DownloadStore::open(&dir);

    for i in 1..=10 {
        let id = DownloadId::new(i);
        let path = dir.join(format!("file_{i}.bin"));
        store
            .start_download(id, format!("https://example.com/{i}"), &path, Some(i * 100))
            .unwrap();
        if i % 2 == 0 {
            store.complete_download(id).unwrap();
        } else if i % 3 == 0 {
            store.cancel_download(id).unwrap();
        }
    }

    // Delete some entries
    store.remove_from_list(DownloadId::new(2)).unwrap();
    store.remove_from_list(DownloadId::new(4)).unwrap();

    // Inspect files under profile directory
    let entries: Vec<_> = fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).collect();

    for entry in &entries {
        let name = entry.file_name().to_string_lossy().to_string();
        assert!(
            !name.ends_with(".journal")
                && !name.ends_with(".wal")
                && !name.ends_with(".undo")
                && !name.ends_with(".bak")
                && !name.contains(".tmp."),
            "no journal or undo log allowed on disk, found: {name}"
        );
    }

    // Ensure downloads.toml is the only file created by store
    let names: Vec<_> = entries
        .iter()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(names, vec![DownloadStore::FILE_NAME.to_string()]);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn list_ordered_by_time_descending() {
    let dir = unique_temp_dir();
    let mut store = DownloadStore::open(&dir);

    let id1 = DownloadId::new(10);
    let id2 = DownloadId::new(20);
    let id3 = DownloadId::new(30);

    store
        .start_download(id1, "https://a.test/1", dir.join("1"), None)
        .unwrap();
    thread::sleep(Duration::from_millis(15));
    store
        .start_download(id2, "https://a.test/2", dir.join("2"), None)
        .unwrap();
    thread::sleep(Duration::from_millis(15));
    store
        .start_download(id3, "https://a.test/3", dir.join("3"), None)
        .unwrap();

    let ordered = store.list_ordered();
    assert_eq!(ordered.len(), 3);
    assert_eq!(ordered[0].id(), id3);
    assert_eq!(ordered[1].id(), id2);
    assert_eq!(ordered[2].id(), id1);

    let _ = fs::remove_dir_all(&dir);
}
