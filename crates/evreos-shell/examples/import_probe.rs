//! The instrument behind `docs/measurements/import-profile-read.md`.
//!
//! Reads one live browser profile over and over for a stated time, through
//! the same code an FR-012 import runs, and reports how the copy-then-read
//! protocol behaved: how many trials succeeded, how many attempts each took,
//! what made an attempt retry, and whether any accepted copy read as torn —
//! a parse error, or fewer history rows or bookmarks than an earlier trial,
//! which a browser that only adds them cannot produce. Beside it, two unverified arms run the
//! same trial the ways the measurement compares against: one naive read of
//! the files with the log applied, and one of the main file alone.
//!
//! ```text
//! cargo run --release -p evreos-shell --example import_probe -- \
//!     <chrome|edge|firefox> <profile directory> <seconds> [synthetic]
//! ```
//!
//! `synthetic` is for the worst-case writer the measurement describes, which
//! stamps one generation into every row of every transaction: there, a copy
//! holding more than one visit time is torn, and the probe counts those too.
//!
//! It prints row counts and timings, never an address or a title.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use evreos_shell::import::json;
use evreos_shell::import::snapshot::{Disk, FileSource, SnapshotPolicy};
use evreos_shell::import::sqlite::Database;
use evreos_shell::import::{ImportScope, SourceBrowser, SourceProfile, read_profile_with};

/// Disk, counting what happened on each call.
#[derive(Default)]
struct Counting {
    reads: u32,
    moved: u32,
    hot: u32,
}

impl FileSource for Counting {
    fn read(&mut self, path: &Path) -> io::Result<Option<Vec<u8>>> {
        self.reads += 1;
        Disk.read(path)
    }

    fn unchanged(&mut self, path: &Path, held: Option<&[u8]>) -> io::Result<bool> {
        let same = Disk.unchanged(path, held)?;
        self.moved += u32::from(!same);
        Ok(same)
    }

    fn journal_hot(&mut self, path: &Path) -> io::Result<bool> {
        let hot = Disk.journal_hot(path)?;
        self.hot += u32::from(hot);
        Ok(hot)
    }

    fn pause(&mut self, duration: Duration) {
        Disk.pause(duration);
    }
}

/// Rows of the history table in one unverified read of the files, and how
/// many distinct values its time column holds.
fn naive_rows(store: &Path, table: &str, with_log: bool) -> Result<(usize, usize), String> {
    let main = fs::read(store).map_err(|e| e.to_string())?;
    let mut wal_path = store.as_os_str().to_owned();
    wal_path.push("-wal");
    let wal = if with_log {
        fs::read(&wal_path).ok()
    } else {
        None
    };
    let db = Database::open(&main, wal.as_deref()).map_err(|e| e.to_string())?;
    let table = db.table(table).map_err(|e| e.to_string())?;
    let time = ["last_visit_time", "last_visit_date"]
        .iter()
        .find_map(|name| table.column(name).ok())
        .ok_or("no time column")?;
    let mut rows = 0;
    let mut times = BTreeSet::new();
    db.scan(&table, |row| {
        rows += 1;
        times.insert(row.get(time).as_integer());
        Ok(())
    })
    .map_err(|e| e.to_string())?;
    Ok((rows, times.len()))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let (browser, dir, seconds, synthetic) = match args.as_slice() {
        [_, browser, dir, seconds] => (browser, dir, seconds, false),
        [_, browser, dir, seconds, flag] if flag == "synthetic" => (browser, dir, seconds, true),
        _ => {
            return Err(
                "usage: import_probe <chrome|edge|firefox> <profile directory> <seconds> [synthetic]"
                    .into(),
            );
        }
    };
    let browser = match browser.as_str() {
        "chrome" => SourceBrowser::Chrome,
        "edge" => SourceBrowser::Edge,
        "firefox" => SourceBrowser::Firefox,
        other => return Err(format!("unknown browser {other}").into()),
    };
    let profile = SourceProfile::new(browser, "probe", dir);
    let (store, table) = match browser {
        SourceBrowser::Firefox => ("places.sqlite", "moz_places"),
        _ => ("History", "urls"),
    };
    let store = Path::new(dir).join(store);
    let deadline = Instant::now() + Duration::from_secs(seconds.parse()?);
    let policy = SnapshotPolicy::default();

    let mut trials = 0u32;
    let mut failures: BTreeMap<String, u32> = BTreeMap::new();
    let mut attempts: BTreeMap<u32, u32> = BTreeMap::new();
    let (mut moved, mut hot) = (0u32, 0u32);
    let mut torn = 0u32;
    let mut last_rows = 0usize;
    let mut last_bookmarks = 0usize;
    let (mut naive_json_errors, mut naive_json_reads) = (0u32, 0u32);
    let mut slowest = Duration::ZERO;
    let (mut naive_errors, mut naive_torn, mut naive_last) = (0u32, 0u32, 0usize);
    let (mut main_only_errors, mut main_only_short) = (0u32, 0u32);

    while Instant::now() < deadline {
        trials += 1;
        let mut source = Counting::default();
        let started = Instant::now();
        let result = read_profile_with(&profile, ImportScope::ALL, policy, &mut source);
        slowest = slowest.max(started.elapsed());
        moved += source.moved;
        hot += source.hot;
        match result {
            Ok(data) => {
                // Every retry was caused by exactly one moved file or one hot
                // journal, summed over the stores the trial copied.
                *attempts.entry(source.moved + source.hot + 1).or_default() += 1;
                let generations: BTreeSet<_> =
                    data.history.iter().map(|visit| visit.visited_at).collect();
                if data.history.len() < last_rows
                    || data.bookmark_count() < last_bookmarks
                    || (synthetic && generations.len() > 1)
                {
                    torn += 1;
                }
                last_rows = last_rows.max(data.history.len());
                last_bookmarks = last_bookmarks.max(data.bookmark_count());
            }
            Err(error) => {
                let kind = format!("{:?}", error.failure());
                *failures.entry(kind).or_default() += 1;
            }
        }

        match naive_rows(&store, table, true) {
            Ok((rows, generations)) => {
                naive_torn += u32::from(rows < naive_last || (synthetic && generations > 1));
                naive_last = naive_last.max(rows);
                match naive_rows(&store, table, false) {
                    Ok((main_only, _)) => main_only_short += u32::from(main_only < rows),
                    Err(_) => main_only_errors += 1,
                }
            }
            Err(_) => naive_errors += 1,
        }
        if let Ok(bytes) = fs::read(Path::new(dir).join("Bookmarks")) {
            naive_json_reads += 1;
            let parsed = String::from_utf8(bytes)
                .ok()
                .and_then(|text| json::parse(&text).ok());
            naive_json_errors += u32::from(parsed.is_none());
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    println!("browser {} store {}", browser.name(), store.display());
    println!("verified copy-then-read (the shipped protocol):");
    println!("  trials {trials}, failed {failures:?}");
    println!("  attempts per successful trial, over its stores (attempts: trials): {attempts:?}");
    println!("  retries caused by: bytes moved between reads {moved}, journal hot {hot}");
    println!("  accepted copies that read torn: {torn}");
    println!("  slowest trial: {} ms", slowest.as_millis());
    println!("  final imported history rows: {last_rows}, bookmarks: {last_bookmarks}");
    println!("unverified single read, log applied:");
    println!("  parse errors {naive_errors}, read torn {naive_torn}");
    println!(
        "unverified single read of Bookmarks: {naive_json_reads} reads, {naive_json_errors} unparseable"
    );
    println!("unverified single read, main file only:");
    println!("  parse errors {main_only_errors}, fewer rows than with the log {main_only_short}");
    Ok(())
}
