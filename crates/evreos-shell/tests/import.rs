//! FR-012 import: reading Chrome, Edge and Firefox profiles, run against the
//! committed fixture profiles in `tests/fixtures/import/`, whose README states
//! what each one exercises. Writing into the member's stores is tested with
//! the step that adds it.

#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use evreos_shell::import::snapshot::{Disk, SnapshotPolicy};
use evreos_shell::import::{
    ImportScope, ImportedNode, SourceBrowser, SourceProfile, read_profile_with,
};

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

fn quick() -> SnapshotPolicy {
    SnapshotPolicy {
        attempts: 3,
        first_backoff: Duration::ZERO,
        max_backoff: Duration::ZERO,
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
    assert!(files.len() >= 5, "import.rs and its four modules");
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
