//! The instrument behind the read, write and batch times in
//! `docs/measurements/import-profile-read.md`.
//!
//! With a profile given, it reads that profile through the import's own read
//! and writes the result into fresh stores through the import's own write,
//! timing each. With `batch`, it times one bookmark batch creating 20,000 and
//! then 50,000 bookmarks in one folder. The stores it writes live in a
//! temporary directory it removes.
//!
//! ```text
//! cargo run --release -p evreos-shell --example import_timing -- \
//!     <chrome|edge|firefox> <profile directory>
//! cargo run --release -p evreos-shell --example import_timing -- batch
//! ```
//!
//! It prints row counts and timings, never an address or a title.

#![forbid(unsafe_code)]

use std::process::ExitCode;
use std::time::{Instant, SystemTime};

use evreos_i18n::Language;
use evreos_shell::import::{
    ImportScope, SourceBrowser, SourceProfile, read_profile, write_imported,
};
use evreos_shell::store::{BookmarkSource, BookmarkStore, FolderId, StoreRegistry};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [batch] if batch == "batch" => time_batches(),
        [browser, profile] => {
            let browser = match browser.as_str() {
                "chrome" => SourceBrowser::Chrome,
                "edge" => SourceBrowser::Edge,
                "firefox" => SourceBrowser::Firefox,
                _ => return usage(),
            };
            time_profile(browser, profile)
        }
        _ => return usage(),
    }
    ExitCode::SUCCESS
}

fn usage() -> ExitCode {
    eprintln!("usage: import_timing <chrome|edge|firefox> <profile directory> | batch");
    ExitCode::FAILURE
}

fn scratch(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "evreos_import_timing_{label}_{}",
        std::process::id()
    ))
}

fn time_profile(browser: SourceBrowser, profile: &str) {
    let source = SourceProfile::new(browser, "timed", profile);
    let started = Instant::now();
    let data = read_profile(&source, ImportScope::ALL).expect("the profile reads");
    let read = started.elapsed();

    let root = scratch("stores");
    let mut stores = StoreRegistry::open(&root);
    let started = Instant::now();
    let counts = write_imported(&data, &mut stores, Language::En, SystemTime::now())
        .expect("the import writes");
    let write = started.elapsed();
    println!(
        "bookmarks {}, history {}: read {} ms, write {} ms",
        counts.bookmarks_imported,
        counts.history_imported,
        read.as_millis(),
        write.as_millis()
    );
    std::fs::remove_dir_all(root).expect("the scratch stores are removed");
}

fn time_batches() {
    for count in [20_000u32, 50_000] {
        let dir = scratch(&format!("batch_{count}"));
        let mut store = BookmarkStore::open(&dir);
        let started = Instant::now();
        store
            .batch(|batch| {
                let folder = batch.create_folder(FolderId::ROOT, "Imported")?;
                for n in 0..count {
                    batch.create_bookmark_with_details(
                        folder,
                        format!("Bookmark {n}"),
                        format!("https://example.test/{n}"),
                        SystemTime::now(),
                        BookmarkSource::imported("Chrome"),
                    )?;
                }
                Ok(())
            })
            .expect("the batch writes");
        println!(
            "{count} bookmarks in one folder: {} ms",
            started.elapsed().as_millis()
        );
        std::fs::remove_dir_all(dir).expect("the scratch store is removed");
    }
}
