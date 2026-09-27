//! When the update check runs: the FR-014 wake `budgets.toml` enumerates,
//! and no other.
//!
//! SC-005 allows the update check one wake per `period_seconds`, and requires
//! every wake to be coalesced with the platform's own scheduler and never to
//! wake the machine from sleep. [`Schedule`] decides when a check is due and
//! how far the platform may move it to fire alongside other work: a tenth of
//! the period, so a six-hour period may fire up to 36 minutes late. It holds
//! no timer. Arming one belongs to the platform's scheduler, given the due
//! time and that tolerance, through a timer that does not wake the machine.

use std::time::{Duration, SystemTime};

use super::UPDATE_CHECK_WAKE;

/// The update check's schedule, from its wake in `budgets.toml`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Schedule {
    period: Duration,
}

impl Default for Schedule {
    fn default() -> Self {
        Self::new()
    }
}

impl Schedule {
    /// The schedule `budgets.toml`'s update-check wake states.
    pub const fn new() -> Self {
        Self {
            period: Duration::from_secs(UPDATE_CHECK_WAKE.period_seconds),
        }
    }

    /// The time between checks.
    pub const fn period(&self) -> Duration {
        self.period
    }

    /// How far past its due time the platform's scheduler may fire a check,
    /// to run it alongside other work: a tenth of the period. `budgets.toml`
    /// states the period alone; the tenth is this design's choice, which no
    /// requirement or budget states.
    pub const fn tolerance(&self) -> Duration {
        Duration::from_secs(self.period.as_secs() / 10)
    }

    /// When the next check is due, given when the last one ran, if one has.
    ///
    /// With none, it is due at once. Otherwise it is due a period after the
    /// last, and a last check that appears to be in the future, because the
    /// clock moved back, counts as having run now, so a clock change never
    /// holds checks off for longer than one period. A due time past the
    /// latest the platform's clock can hold is due at once instead.
    pub fn next_due(&self, last: Option<SystemTime>, now: SystemTime) -> SystemTime {
        match last {
            None => now,
            Some(last) => last.min(now).checked_add(self.period).unwrap_or(now),
        }
    }
}
