//! Copy-then-read: take a consistent in-memory copy of a store a running
//! browser may be writing.
//!
//! The import-profile-read measurement at
//! `docs/measurements/import-profile-read.md` found that all three browsers
//! hold their stores under an exclusive SQLite lock while they run, so a
//! direct read through the SQLite library fails, and that a single naive copy
//! of the files, though it read cleanly in every trial of the live runs, has a
//! window in which the copy is torn — a rollback journal hot mid-transaction,
//! a checkpoint rewriting the main file — which the synthetic worst-case run
//! in the same file hits. So the copy is verified rather than trusted:
//!
//! 1. the store's rollback journal, if it has one, must not be hot — its
//!    header must not carry the journal magic, which SQLite writes when a
//!    transaction starts and zeroes or deletes when it ends;
//! 2. the main file and its write-ahead log are read whole into memory;
//! 3. both are read again and compared with what was held, and the journal
//!    checked again.
//!
//! A copy is accepted only when nothing moved between the first read and the
//! second and no transaction was open at any check. A transaction that began
//! and ended while the first read was under way changes the bytes the second
//! read sees; a checkpoint that rewrote part of the main file does the same.
//! Frames appended to the log mid-copy are harmless on their own — the reader
//! honours only frames up to the log's last valid commit — but they fail the
//! comparison too, which costs a retry and nothing else.
//!
//! One interleaving escapes it, and is stated rather than assumed away: a
//! rollback-journal transaction that begins after the first journal check,
//! ends before the last, writes a page ahead of both reads and another page
//! behind both, leaves two identical reads of a torn file. SQLite writes a
//! transaction's pages in ascending order in a few milliseconds, so the
//! transaction would have to straddle both whole-file reads; no run in the
//! measurement produced one, and the parser still refuses any copy whose
//! structure does not hold together.
//!
//! The copy lives only in memory and is dropped when the import has read it:
//! no copy of another browser's history is ever written to disk, so a crash
//! mid-import leaves nothing of it behind.

#![forbid(unsafe_code)]

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

/// The eight bytes a live rollback journal's header begins with.
const JOURNAL_MAGIC: [u8; 8] = [0xd9, 0xd5, 0x05, 0xf9, 0x20, 0xa1, 0x63, 0xd7];

/// How hard to try before telling the member the browser is too busy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotPolicy {
    /// Attempts before giving up, the first included.
    pub attempts: u32,
    /// The pause before the second attempt; it doubles, up to `max_backoff`.
    pub first_backoff: Duration,
    /// The longest pause between attempts.
    pub max_backoff: Duration,
}

impl Default for SnapshotPolicy {
    /// Eight attempts over about 1.6 s of pauses. Against the live browsers
    /// in the measurement no trial needed more than three; the synthetic
    /// writer that rewrites its whole store in every transaction needed all
    /// eight once. The bound is what keeps a browser that never stops
    /// writing from holding the import open indefinitely.
    fn default() -> Self {
        Self {
            attempts: 8,
            first_backoff: Duration::from_millis(25),
            max_backoff: Duration::from_millis(400),
        }
    }
}

/// The files of one store: the main file and the sidecars SQLite keeps
/// beside it. A JSON store has no sidecars.
#[derive(Debug, Clone)]
pub struct StoreFiles {
    /// The main file.
    pub main: PathBuf,
    /// The write-ahead log, `<main>-wal`, for an SQLite store.
    pub wal: Option<PathBuf>,
    /// The rollback journal, `<main>-journal`, for an SQLite store.
    pub journal: Option<PathBuf>,
}

impl StoreFiles {
    /// The files SQLite keeps for a database at `main`.
    pub fn sqlite(main: PathBuf) -> Self {
        let sidecar = |suffix: &str| {
            let mut name = main.clone().into_os_string();
            name.push(suffix);
            Some(PathBuf::from(name))
        };
        Self {
            wal: sidecar("-wal"),
            journal: sidecar("-journal"),
            main,
        }
    }

    /// A store that is one file, replaced whole when it changes.
    pub fn single(main: PathBuf) -> Self {
        Self {
            main,
            wal: None,
            journal: None,
        }
    }
}

/// A verified copy of one store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// The main file's bytes.
    pub main: Vec<u8>,
    /// The write-ahead log's bytes, when the store has one.
    pub wal: Option<Vec<u8>>,
    /// How many attempts the copy took, the successful one included.
    pub attempts: u32,
}

/// Why no verified copy was taken.
#[derive(Debug)]
pub enum SnapshotError {
    /// The main file does not exist: the browser has never written this
    /// store, which is not a failure of the import.
    Absent,
    /// No attempt got a still copy, and the files were seen changing: the
    /// store moved between the two reads of an attempt, or differed between
    /// attempts refused for an open write. Not every attempt need have seen
    /// it move.
    Busy {
        /// Attempts made.
        attempts: u32,
    },
    /// Every attempt found a transaction open in the rollback journal, and
    /// the files never moved: the browser is holding a write open, or it
    /// stopped mid-write and left the journal for it to roll back the next
    /// time it opens the store.
    Interrupted {
        /// Attempts made.
        attempts: u32,
    },
    /// The files could not be read at all.
    Io(io::Error),
}

impl From<io::Error> for SnapshotError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Where the bytes come from. The real source is the file system; the unit
/// tests below script a writer that moves the files between reads.
pub trait FileSource {
    /// A file's whole contents, or `None` if it does not exist.
    fn read(&mut self, path: &Path) -> io::Result<Option<Vec<u8>>>;

    /// Whether the file still holds exactly `held` (`None`: still absent).
    fn unchanged(&mut self, path: &Path, held: Option<&[u8]>) -> io::Result<bool>;

    /// Whether the rollback journal at `path` is hot.
    fn journal_hot(&mut self, path: &Path) -> io::Result<bool>;

    /// Pause between attempts.
    fn pause(&mut self, duration: Duration);
}

/// The file system.
#[derive(Debug, Default, Clone, Copy)]
pub struct Disk;

/// The most one file of a store may hold for the import to read it: far
/// beyond any `History` or `places.sqlite` a profile used for years reaches,
/// and a bound on what a file that never ends can make the reader hold.
pub const MAX_STORE_FILE_BYTES: u64 = 1 << 30;

/// Open `path` if it is there and is a regular file. A pipe would block the
/// open and a device such as `/dev/zero` never ends, and a profile directory
/// can hold either under a store's name, behind a link or not; neither is a
/// store, so each is an error rather than a read. The path is checked before
/// it is opened, and what was opened is checked again, so a pipe or device
/// swapped in between is refused too. Where [`O_NONBLOCK`] is known, the open
/// itself cannot block either, since a pipe opened without waiting for a
/// writer returns at once; on a Unix where it is not, a pipe swapped in
/// between the check and the open can still hold the open until a writer
/// comes.
fn open_if_present(path: &Path) -> io::Result<Option<File>> {
    match fs::metadata(path) {
        Ok(meta) if !meta.is_file() => return Err(not_regular()),
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    }
    match open_regular(path) {
        Ok(file) => Ok(Some(file)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn not_regular() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "not a regular file")
}

/// Open `path` without waiting on it, and keep it only if what was opened is
/// a regular file.
fn open_regular(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(O_NONBLOCK);
    }
    let file = options.open(path)?;
    if file.metadata()?.is_file() {
        Ok(file)
    } else {
        Err(not_regular())
    }
}

/// `O_NONBLOCK`, which the standard library does not name, for the Unix
/// systems whose value for it is known here: Linux and Android on each
/// architecture (MIPS and SPARC define their own), and the BSDs and Apple's
/// systems, which share one. On a regular file it changes nothing; on a pipe
/// it lets the open return without a writer. Any other Unix opens without
/// it, `0` here, and is left with the check made before the open.
#[cfg(unix)]
const O_NONBLOCK: i32 = if cfg!(any(target_os = "linux", target_os = "android")) {
    if cfg!(any(
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6"
    )) {
        0x0080
    } else if cfg!(any(target_arch = "sparc", target_arch = "sparc64")) {
        0x4000
    } else {
        0o4000
    }
} else if cfg!(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
)) {
    0x0004
} else {
    0
};

/// The whole of the regular file at `path`, or `None` if it does not exist;
/// an error if it is not a regular file or holds more than `limit` bytes.
/// Past its opening check the file is read through a bound, so one that grows
/// while it is read cannot carry the read past `limit` either.
pub fn read_bounded(path: &Path, limit: u64) -> io::Result<Option<Vec<u8>>> {
    let Some(file) = open_if_present(path)? else {
        return Ok(None);
    };
    let too_large = || io::Error::new(io::ErrorKind::InvalidData, "larger than the import reads");
    if file.metadata()?.len() > limit {
        return Err(too_large());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(too_large());
    }
    Ok(Some(bytes))
}

impl FileSource for Disk {
    fn read(&mut self, path: &Path) -> io::Result<Option<Vec<u8>>> {
        read_bounded(path, MAX_STORE_FILE_BYTES)
    }

    /// Compared in chunks as it is read, so the second pass holds no second
    /// copy of a store that runs to tens of megabytes, and more on a
    /// profile used for years.
    fn unchanged(&mut self, path: &Path, held: Option<&[u8]>) -> io::Result<bool> {
        let (mut file, held) = match (open_if_present(path)?, held) {
            (None, None) => return Ok(true),
            (Some(file), Some(held)) => (file, held),
            _ => return Ok(false),
        };
        let mut chunk = vec![0u8; 64 * 1024];
        let mut at = 0;
        loop {
            let read = file.read(&mut chunk)?;
            if read == 0 {
                return Ok(at == held.len());
            }
            if held.get(at..at + read) != Some(&chunk[..read]) {
                return Ok(false);
            }
            at += read;
        }
    }

    fn journal_hot(&mut self, path: &Path) -> io::Result<bool> {
        let Some(mut file) = open_if_present(path)? else {
            return Ok(false);
        };
        let mut header = [0u8; 8];
        let mut filled = 0;
        while filled < header.len() {
            let read = file.read(&mut header[filled..])?;
            if read == 0 {
                return Ok(false);
            }
            filled += read;
        }
        Ok(header == JOURNAL_MAGIC)
    }

    fn pause(&mut self, duration: Duration) {
        thread::sleep(duration);
    }
}

/// The lengths of the main file, its log and its rollback journal and a hash
/// over all three: enough to tell whether a store refused for an open write
/// changed between attempts. The journal is in it because a writer holding a
/// rollback-journal transaction open writes there first and may not touch
/// the main file until it commits.
type Fingerprint = (usize, usize, usize, u64);

fn fingerprint(source: &mut impl FileSource, files: &StoreFiles) -> io::Result<Fingerprint> {
    // FNV-1a: a comparison between two reads of one store, not a defence
    // against an adversary, which the parser's own checks are. Each file is
    // hashed and let go before the next is read, so the fingerprint holds
    // one file at a time, never the store and its journal together.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut lengths = [0usize; 3];
    let paths = [
        Some(&files.main),
        files.wal.as_ref(),
        files.journal.as_ref(),
    ];
    for (length, path) in lengths.iter_mut().zip(paths) {
        let Some(path) = path else { continue };
        let bytes = source.read(path)?.unwrap_or_default();
        for byte in &bytes {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3);
        }
        *length = bytes.len();
    }
    Ok((lengths[0], lengths[1], lengths[2], hash))
}

fn journal_hot(source: &mut impl FileSource, files: &StoreFiles) -> io::Result<bool> {
    match &files.journal {
        Some(path) => source.journal_hot(path),
        None => Ok(false),
    }
}

/// Record an attempt refused for an open write, at either journal check:
/// whether the files differ from what the last such attempt found.
fn refused_open(
    source: &mut impl FileSource,
    files: &StoreFiles,
    held: &mut Option<Fingerprint>,
) -> io::Result<bool> {
    let now = fingerprint(source, files)?;
    let moved = held.is_some_and(|before| before != now);
    *held = Some(now);
    Ok(moved)
}

/// Take a verified copy of `files` under `policy`.
pub fn take(
    source: &mut impl FileSource,
    files: &StoreFiles,
    policy: SnapshotPolicy,
) -> Result<Snapshot, SnapshotError> {
    let attempts = policy.attempts.max(1);
    let mut backoff = policy.first_backoff;
    let mut moved = false;
    // What the files held at the last attempt refused for an open write, so
    // that a store refused every time can still be told moving from still.
    let mut held: Option<Fingerprint> = None;
    for attempt in 1..=attempts {
        if attempt > 1 {
            source.pause(backoff);
            backoff = (backoff * 2).min(policy.max_backoff);
        }
        if journal_hot(source, files)? {
            moved |= refused_open(source, files, &mut held)?;
            continue;
        }
        let Some(main) = source.read(&files.main)? else {
            return Err(SnapshotError::Absent);
        };
        let wal = match &files.wal {
            Some(path) => source.read(path)?,
            None => None,
        };
        if !source.unchanged(&files.main, Some(&main))? {
            moved = true;
            continue;
        }
        // A log that appeared since the first read counts as a change: it
        // means writing began.
        if let Some(path) = &files.wal {
            if !source.unchanged(path, wal.as_deref())? {
                moved = true;
                continue;
            }
        }
        if journal_hot(source, files)? {
            // Let this attempt's copy go before the fingerprint reads the
            // files again, so a refusal never holds more than one of them.
            drop((main, wal));
            moved |= refused_open(source, files, &mut held)?;
            continue;
        }
        return Ok(Snapshot {
            main,
            wal,
            attempts: attempt,
        });
    }
    if moved {
        Err(SnapshotError::Busy { attempts })
    } else {
        Err(SnapshotError::Interrupted { attempts })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A write the scripted writer makes: to the files, and to whether the
    /// journal is hot.
    type Write = Box<dyn Fn(&mut HashMap<PathBuf, Vec<u8>>, &mut bool)>;

    /// A scripted store: each read or check advances a clock, and a writer
    /// changes the files at chosen ticks.
    struct Scripted {
        files: HashMap<PathBuf, Vec<u8>>,
        hot: bool,
        tick: u32,
        writes: Vec<(u32, Write)>,
        paused: Vec<Duration>,
        /// Answer every second journal check hot, whatever `hot` says.
        hot_at_last_check: bool,
        checks: u32,
        /// A write made while the import backs off.
        on_pause: Option<Write>,
    }

    impl Scripted {
        fn new(main: &[u8]) -> Self {
            let mut files = HashMap::new();
            files.insert(PathBuf::from("db"), main.to_vec());
            Self {
                files,
                hot: false,
                tick: 0,
                writes: Vec::new(),
                paused: Vec::new(),
                hot_at_last_check: false,
                checks: 0,
                on_pause: None,
            }
        }

        fn advance(&mut self) {
            self.tick += 1;
            let tick = self.tick;
            for (at, write) in &self.writes {
                if *at == tick {
                    write(&mut self.files, &mut self.hot);
                }
            }
        }
    }

    impl FileSource for Scripted {
        fn read(&mut self, path: &Path) -> io::Result<Option<Vec<u8>>> {
            self.advance();
            Ok(self.files.get(path).cloned())
        }

        fn unchanged(&mut self, path: &Path, held: Option<&[u8]>) -> io::Result<bool> {
            self.advance();
            Ok(self.files.get(path).map(Vec::as_slice) == held)
        }

        fn journal_hot(&mut self, _: &Path) -> io::Result<bool> {
            self.advance();
            self.checks += 1;
            Ok(self.hot || (self.hot_at_last_check && self.checks % 2 == 0))
        }

        fn pause(&mut self, duration: Duration) {
            self.paused.push(duration);
            if let Some(write) = &self.on_pause {
                write(&mut self.files, &mut self.hot);
            }
        }
    }

    fn files() -> StoreFiles {
        StoreFiles::sqlite(PathBuf::from("db"))
    }

    fn quick() -> SnapshotPolicy {
        SnapshotPolicy {
            attempts: 4,
            first_backoff: Duration::from_millis(10),
            max_backoff: Duration::from_millis(30),
        }
    }

    #[test]
    fn a_quiet_store_is_copied_on_the_first_attempt() {
        let mut source = Scripted::new(b"quiet");
        let snapshot = take(&mut source, &files(), quick()).unwrap();
        assert_eq!(snapshot.main, b"quiet");
        assert_eq!(snapshot.wal, None);
        assert_eq!(snapshot.attempts, 1);
        assert!(source.paused.is_empty());
    }

    #[test]
    fn a_write_between_the_two_reads_forces_a_retry() {
        let mut source = Scripted::new(b"before");
        // Tick 2 is the first read of the main file; tick 3 lands between it
        // and the verifying read.
        source.writes.push((
            3,
            Box::new(|files, _| {
                files.insert(PathBuf::from("db"), b"after".to_vec());
            }),
        ));
        let snapshot = take(&mut source, &files(), quick()).unwrap();
        assert_eq!(snapshot.main, b"after", "the copy is the settled state");
        assert_eq!(snapshot.attempts, 2);
        assert_eq!(source.paused, [Duration::from_millis(10)]);
    }

    #[test]
    fn a_hot_journal_is_never_copied_through() {
        let mut source = Scripted::new(b"torn");
        source.hot = true;
        // The transaction ends before the third attempt: each refused
        // attempt checks the journal and fingerprints the main file, its log
        // and its journal, four ticks, so the sixth falls in the second.
        source.writes.push((
            6,
            Box::new(|files, hot| {
                files.insert(PathBuf::from("db"), b"committed".to_vec());
                *hot = false;
            }),
        ));
        let snapshot = take(&mut source, &files(), quick()).unwrap();
        assert_eq!(snapshot.main, b"committed");
        assert_eq!(snapshot.attempts, 3);
    }

    #[test]
    fn a_journal_that_turns_hot_after_the_copy_rejects_it() {
        let mut source = Scripted::new(b"x");
        // Ticks: 1 hot?, 2 read main, 3 read wal, 4 verify main, 5 re-read
        // wal, 6 hot? -- the transaction opens just before the last check.
        source.writes.push((6, Box::new(|_, hot| *hot = true)));
        source.writes.push((8, Box::new(|_, hot| *hot = false)));
        let snapshot = take(&mut source, &files(), quick()).unwrap();
        assert!(snapshot.attempts > 1);
    }

    #[test]
    fn a_log_that_appears_mid_copy_forces_a_retry() {
        let mut source = Scripted::new(b"main");
        source.writes.push((
            4,
            Box::new(|files, _| {
                files.insert(PathBuf::from("db-wal"), b"frames".to_vec());
            }),
        ));
        let snapshot = take(&mut source, &files(), quick()).unwrap();
        assert_eq!(snapshot.wal.as_deref(), Some(&b"frames"[..]));
        assert_eq!(snapshot.attempts, 2);
    }

    #[test]
    fn a_store_that_never_settles_is_reported_busy_after_the_bound() {
        let mut source = Scripted::new(b"0");
        for tick in 1..200 {
            source.writes.push((
                tick,
                Box::new(move |files, _| {
                    files.insert(PathBuf::from("db"), tick.to_le_bytes().to_vec());
                }),
            ));
        }
        match take(&mut source, &files(), quick()) {
            Err(SnapshotError::Busy { attempts }) => assert_eq!(attempts, 4),
            other => panic!("expected Busy, got {other:?}"),
        }
        assert_eq!(
            source.paused,
            [10, 20, 30].map(Duration::from_millis),
            "the backoff doubles and is capped"
        );
    }

    #[test]
    fn a_journal_left_hot_on_a_still_store_is_interrupted_not_busy() {
        let mut source = Scripted::new(b"left by a crash");
        source.hot = true;
        match take(&mut source, &files(), quick()) {
            Err(SnapshotError::Interrupted { attempts }) => assert_eq!(attempts, 4),
            other => panic!("expected Interrupted, got {other:?}"),
        }
    }

    #[test]
    fn a_writer_holding_every_check_hot_while_writing_is_busy_not_interrupted() {
        let mut source = Scripted::new(b"0");
        source.hot = true;
        for tick in 1..200 {
            source.writes.push((
                tick,
                Box::new(move |files, _| {
                    files.insert(PathBuf::from("db"), tick.to_le_bytes().to_vec());
                }),
            ));
        }
        match take(&mut source, &files(), quick()) {
            Err(SnapshotError::Busy { attempts }) => assert_eq!(attempts, 4),
            other => panic!("expected Busy, got {other:?}"),
        }
    }

    #[test]
    fn a_writer_changing_only_its_journal_is_busy_not_interrupted() {
        let mut source = Scripted::new(b"main never changes");
        source.hot = true;
        for tick in 1..200 {
            source.writes.push((
                tick,
                Box::new(move |files, _| {
                    files.insert(PathBuf::from("db-journal"), tick.to_le_bytes().to_vec());
                }),
            ));
        }
        match take(&mut source, &files(), quick()) {
            Err(SnapshotError::Busy { attempts }) => assert_eq!(attempts, 4),
            other => panic!("expected Busy, got {other:?}"),
        }
    }

    #[test]
    fn a_writer_caught_open_only_at_the_last_check_is_busy_not_interrupted() {
        let mut source = Scripted::new(b"0");
        // Every attempt finds the journal cold at its first check and hot at
        // its last, and the writer commits a new main file while the import
        // backs off.
        source.hot_at_last_check = true;
        source.on_pause = Some(Box::new(|files, _| {
            let next = files[&PathBuf::from("db")][0] + 1;
            files.insert(PathBuf::from("db"), vec![next]);
        }));
        match take(&mut source, &files(), quick()) {
            Err(SnapshotError::Busy { attempts }) => assert_eq!(attempts, 4),
            other => panic!("expected Busy, got {other:?}"),
        }
    }

    #[test]
    fn an_absent_store_is_absent_not_busy() {
        let mut source = Scripted::new(b"");
        source.files.clear();
        assert!(matches!(
            take(&mut source, &files(), quick()),
            Err(SnapshotError::Absent)
        ));
    }

    #[test]
    fn the_disk_source_compares_in_chunks_and_reads_the_journal_header() {
        let dir = std::env::temp_dir().join(format!(
            "evreos_snapshot_unit_{}_{}",
            std::process::id(),
            line!()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let main = dir.join("store");
        let big: Vec<u8> = (0..200_000u32).map(|n| n as u8).collect();
        std::fs::write(&main, &big).unwrap();
        let mut disk = Disk;
        assert!(disk.unchanged(&main, Some(&big)).unwrap());
        let mut other = big.clone();
        other[150_000] ^= 1;
        assert!(!disk.unchanged(&main, Some(&other)).unwrap());
        assert!(!disk.unchanged(&main, Some(&big[..big.len() - 1])).unwrap());

        let journal = StoreFiles::sqlite(main.clone()).journal.unwrap();
        assert!(!disk.journal_hot(&journal).unwrap(), "absent is cold");
        std::fs::write(&journal, [0u8; 512]).unwrap();
        assert!(!disk.journal_hot(&journal).unwrap(), "zeroed is cold");
        let mut hot = JOURNAL_MAGIC.to_vec();
        hot.extend_from_slice(&[0u8; 100]);
        std::fs::write(&journal, hot).unwrap();
        assert!(disk.journal_hot(&journal).unwrap());
        assert!(disk.unchanged(&dir.join("absent"), None).unwrap());
        assert!(!disk.unchanged(&main, None).unwrap());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_disk_source_reads_only_a_bounded_regular_file() {
        let dir = std::env::temp_dir().join(format!(
            "evreos_snapshot_unit_{}_{}",
            std::process::id(),
            line!()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let store = dir.join("store");
        std::fs::write(&store, b"twelve bytes").unwrap();
        assert_eq!(read_bounded(&store, 12).unwrap().unwrap(), b"twelve bytes");
        assert_eq!(
            read_bounded(&store, 11).unwrap_err().kind(),
            io::ErrorKind::InvalidData,
            "one byte over the bound"
        );
        // A sparse file past the store bound is refused before any of it is read.
        let huge = dir.join("huge");
        File::create(&huge)
            .unwrap()
            .set_len(MAX_STORE_FILE_BYTES + 1)
            .unwrap();
        assert!(Disk.read(&huge).is_err());
        assert!(Disk.read(&dir).is_err(), "a directory is not a store");
        #[cfg(unix)]
        {
            let zero = dir.join("zero");
            std::os::unix::fs::symlink("/dev/zero", &zero).unwrap();
            assert!(Disk.read(&zero).is_err(), "a device behind a link");
            assert!(Disk.unchanged(&zero, Some(b"x")).is_err());
            assert!(Disk.journal_hot(&zero).is_err());
            let pipe = dir.join("pipe");
            let made = std::process::Command::new("mkfifo")
                .arg(&pipe)
                .status()
                .is_ok_and(|status| status.success());
            if made {
                assert!(Disk.read(&pipe).is_err(), "a pipe, which would block");
            }
            // A pipe swapped in after the path was checked: where the flag's
            // value is known, the open itself must return, not wait for a
            // writer. Where it is not, the documentation says it can wait.
            if made && O_NONBLOCK != 0 {
                let (sent, received) = std::sync::mpsc::channel();
                let swapped = pipe.clone();
                std::thread::spawn(move || {
                    let _ = sent.send(open_regular(&swapped).is_err());
                });
                assert_eq!(
                    received.recv_timeout(Duration::from_secs(5)),
                    Ok(true),
                    "opening a pipe returned, and refused it"
                );
            }
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
