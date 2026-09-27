//! FR-014's release to a proportion of users, decided on the machine.
//!
//! The manifest carries a rollout in millionths. Each install draws one
//! value, once, uniformly from 0 to 999,999, keeps it in a file of its own,
//! and is offered an update only when that value is below the rollout
//! (research §10.1). The value never leaves the machine: it is residence
//! class L in the data model, and nothing that plans an update check takes
//! it. It comes from the operating system's randomness, never from anything
//! about the machine, which FR-036a forbids deriving a correlator from.

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

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
        let partial = path.with_extension("partial");
        fs::write(&partial, format!("{}\n", draw.0))?;
        fs::rename(&partial, path)?;
        Ok(draw)
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

    /// A value drawn uniformly from 0 to 999,999, by rejecting the draws
    /// above the largest multiple of a million a `u32` holds, so that no
    /// value is likelier than another.
    fn draw() -> io::Result<Self> {
        const LIMIT: u32 = u32::MAX - (u32::MAX % ROLLOUT_WHOLE);
        loop {
            let value = getrandom::u32().map_err(|error| io::Error::other(error.to_string()))?;
            if value < LIMIT {
                return Ok(Self(value % ROLLOUT_WHOLE));
            }
        }
    }
}
