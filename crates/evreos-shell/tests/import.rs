//! FR-012 import: reading Chrome, Edge and Firefox profiles, run against the
//! committed fixture profiles in `tests/fixtures/import/`, whose README states
//! what each one exercises. Writing into the member's stores is tested with
//! the step that adds it.

#![forbid(unsafe_code)]

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use evreos_shell::import::snapshot::{Disk, FileSource, SnapshotPolicy};
use evreos_shell::import::{
    ImportError, ImportScope, ImportedNode, SourceBrowser, SourceProfile, read_profile_with,
};

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
fn the_import_has_no_path_to_the_network() {
    // Reading another browser's files is the local computation FR-007a
    // permits; the import must hold no route by which any of it could leave.
    // Its only reference into the rest of the crate is the stores it writes.
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = vec![src.join("import.rs")];
    for entry in fs::read_dir(src.join("import")).unwrap() {
        files.push(entry.unwrap().path());
    }
    assert!(files.len() >= 6, "import.rs and its five modules");
    for file in files {
        let content = fs::read_to_string(&file).unwrap();
        for line in content.lines().map(str::trim) {
            if line.starts_with("//") {
                continue;
            }
            assert!(
                !line.contains("evreos_net") && !line.contains("evreos-net"),
                "{} references the egress crate: {line}",
                file.display()
            );
            if let Some(at) = line.find("crate::") {
                assert!(
                    line[at..].starts_with("crate::store"),
                    "{} reaches outside the stores: {line}",
                    file.display()
                );
            }
        }
    }
}
