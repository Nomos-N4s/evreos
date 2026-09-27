//! FR-014's release to a proportion of users, decided on the machine.
//!
//! The manifest carries a rollout in millionths. Each install draws one
//! value, once, uniformly from 0 to 999,999, keeps it in a file of its own,
//! and is offered an update only when that value is below the rollout
//! (research §10.1). The value never leaves the machine: it belongs in the
//! data model's residence class L, which does not yet name it, and nothing
//! that plans an update check takes it. It comes from the operating system's randomness, never from anything
//! about the machine, which FR-036a forbids deriving a correlator from.

use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use super::manifest::ROLLOUT_WHOLE;

/// This install's rollout value, from 0 to 999,999.
///
/// Its `Debug` output leaves the value out, so that a log line or a crash
/// report that prints one does not carry it off the machine.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RolloutDraw(u32);

impl fmt::Debug for RolloutDraw {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RolloutDraw(..)")
    }
}

impl RolloutDraw {
    /// The value kept at `path`, or, when there is none, or none that reads
    /// as a value, a new one drawn and kept there.
    ///
    /// The value is drawn the first time an install asks, so it is kept from
    /// then on. The file holds the value in decimal and nothing else. A file
    /// that does not read as a value is replaced by a new draw, which may
    /// move the install in or out of a rollout in progress, once.
    ///
    /// A new draw is written to a file of its own and flushed to the disk,
    /// then linked into place only if no file is there yet. So when two
    /// processes draw at once for an install that has no file, one value is
    /// kept and both return it. That holds on a file system with hard links,
    /// as the ones Windows and macOS install to have. On one without, such
    /// as FAT, the draw is renamed into place instead, and the last process
    /// to draw wins. Replacing a file that is not a value makes no such
    /// promise either: the last process to replace it wins.
    ///
    /// Its errors are the file system's. The file's own absence is never
    /// one, since a new value is drawn then, so an error of kind `NotFound`
    /// means the directory `path` names is missing.
    pub fn load_or_draw(path: &Path) -> io::Result<Self> {
        match fs::read(path) {
            Ok(bytes) => {
                if let Some(draw) = Self::parse(&bytes) {
                    return Ok(draw);
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let draw = Self::draw()?;
        let partial = Self::write_partial(path, draw)?;
        let kept = Self::keep(path, &partial, draw);
        let _ = fs::remove_file(&partial);
        if kept.is_ok() {
            Self::sync_directory(path);
        }
        kept
    }

    /// Flushes the directory holding `path` to the disk, so that the name a
    /// new draw was just kept under survives a power loss, and the install
    /// does not draw again after one. It is done on Unix alone, where the
    /// standard library opens a directory to flush it; elsewhere the file
    /// system's own journal is relied on. A failure here is not the draw's:
    /// the value is kept either way.
    fn sync_directory(path: &Path) {
        if !cfg!(unix) {
            return;
        }
        if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            let _ = fs::File::open(dir).and_then(|dir| dir.sync_all());
        }
    }

    /// Keeps `draw`, written at `partial`, at `path`, and returns the value
    /// kept there.
    fn keep(path: &Path, partial: &Path, draw: Self) -> io::Result<Self> {
        match fs::hard_link(partial, path) {
            Ok(()) => Ok(draw),
            // A file is there already. If it is a value, another process
            // kept its draw first, and that is the value; if it is not, it
            // is replaced whole. A file that cannot be read is neither, and
            // is left as it is.
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                match Self::parse(&fs::read(path)?) {
                    Some(kept) => Ok(kept),
                    None => {
                        fs::rename(partial, path)?;
                        Ok(draw)
                    }
                }
            }
            // A file system without hard links still gets the value.
            Err(_) => {
                fs::rename(partial, path)?;
                Ok(draw)
            }
        }
    }

    /// Writes `draw` to a new file beside `path`, flushed to the disk, and
    /// returns its path. A name another call left behind, from a process
    /// that ended before removing it, is passed over and left as it is.
    fn write_partial(path: &Path, draw: Self) -> io::Result<PathBuf> {
        loop {
            let partial = Self::partial_path(path);
            let mut file = match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&partial)
            {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            };
            let written = file
                .write_all(format!("{}\n", draw.0).as_bytes())
                .and_then(|()| file.sync_all());
            return match written {
                Ok(()) => Ok(partial),
                Err(error) => {
                    let _ = fs::remove_file(&partial);
                    Err(error)
                }
            };
        }
    }

    /// A name beside `path` that no other running process, nor another call
    /// in this one, writes to.
    fn partial_path(path: &Path) -> PathBuf {
        static CALLS: AtomicU32 = AtomicU32::new(0);
        let call = CALLS.fetch_add(1, Ordering::Relaxed);
        let mut name = path.file_name().unwrap_or_default().to_os_string();
        name.push(format!(".{}-{call}.partial", std::process::id()));
        path.with_file_name(name)
    }

    /// Whether an update rolled out to `rollout` millionths is offered to
    /// this install. Widening a rollout keeps every install already offered
    /// it, and a rollout of 0 offers it to none.
    pub fn included(self, rollout: u32) -> bool {
        self.0 < rollout
    }

    /// The value in `bytes`, read as bytes rather than text so that a file
    /// which is not even text is a file that does not read as a value.
    fn parse(bytes: &[u8]) -> Option<Self> {
        let digits = bytes.strip_suffix(b"\n").unwrap_or(bytes);
        if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
            return None;
        }
        let value: u32 = std::str::from_utf8(digits).ok()?.parse().ok()?;
        (value < ROLLOUT_WHOLE).then_some(Self(value))
    }

    /// A value drawn uniformly from 0 to 999,999.
    fn draw() -> io::Result<Self> {
        loop {
            let random = getrandom::u32().map_err(|error| io::Error::other(error.to_string()))?;
            if let Some(draw) = Self::from_random(random) {
                return Ok(draw);
            }
        }
    }

    /// The value a uniformly random `u32` gives, or none when it lies above
    /// the largest multiple of a million a `u32` holds. Rejecting those, and
    /// drawing again, is what keeps every value as likely as another.
    fn from_random(random: u32) -> Option<Self> {
        const LIMIT: u32 = u32::MAX - (u32::MAX % ROLLOUT_WHOLE);
        (random < LIMIT).then_some(Self(random % ROLLOUT_WHOLE))
    }
}

#[cfg(test)]
mod tests {
    use super::RolloutDraw;

    #[test]
    fn a_random_value_reduces_below_a_million_or_is_rejected() {
        let limit = 4_294_000_000;
        assert_eq!(RolloutDraw::from_random(0), Some(RolloutDraw(0)));
        assert_eq!(
            RolloutDraw::from_random(999_999),
            Some(RolloutDraw(999_999))
        );
        assert_eq!(RolloutDraw::from_random(1_000_000), Some(RolloutDraw(0)));
        assert_eq!(
            RolloutDraw::from_random(limit - 1),
            Some(RolloutDraw(999_999))
        );
        assert_eq!(RolloutDraw::from_random(limit), None);
        assert_eq!(RolloutDraw::from_random(u32::MAX), None);
    }
}
