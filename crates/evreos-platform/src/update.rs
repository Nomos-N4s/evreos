//! FR-014: the browser updating itself.
//!
//! The update service publishes a signed manifest, whose bytes
//! `docs/formats/update-manifest.md` fixes. [`manifest`] reads it and
//! verifies it against the key pinned in the shipped binary. [`rollout`]
//! holds the value this install draws, once, to decide its own inclusion.

//! [`decide`] takes a manifest through every check in the order the format
//! document fixes, and says whether this install is offered the update.
//! [`artefact`] checks an offered update's artefact against its manifest
//! before anything applies it.

pub mod artefact;
pub mod manifest;
pub mod rollout;
pub mod wake;

use std::fmt;

use self::manifest::{Refusal, UpdateKey, VerifiedManifest, Version};
use self::rollout::RolloutDraw;
use self::wake::Wake;

/// The update check's wake, as `budgets.toml` states it: the check is armed
/// under this entry and no other, every `period_seconds`, and one check may
/// use at most `processor_time_bound_ms` of processor time.
pub const UPDATE_CHECK_WAKE: Wake = include!(concat!(env!("OUT_DIR"), "/update_wake.rs"));

/// The build that is running, which a manifest is checked against.
#[derive(Clone, Copy, Debug)]
pub struct Installed<'a> {
    /// The build's platform, as a manifest names it: `windows-x86_64`.
    pub platform: &'a str,
    /// The version installed.
    pub version: Version,
}

/// What a manifest that passed every check means for this install.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// The manifest names the version installed.
    UpToDate,
    /// The manifest names a newer version, rolled out to installs this one is
    /// not yet among.
    NotIncluded,
    /// The manifest names a newer version this install is offered. Its
    /// artefact is not applied until it matches the manifest.
    Offered(VerifiedManifest),
}

/// Why a manifest was refused, and nothing in it acted on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckRefusal {
    /// It is not a manifest signed under the pinned key.
    Manifest(Refusal),
    /// It is for another build than the one running.
    Platform,
    /// It is past its `not after`.
    Expired,
    /// It names an older version than the one installed.
    Downgrade,
}

impl fmt::Display for CheckRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Manifest(refusal) => refusal.fmt(f),
            Self::Platform => f.write_str("the update manifest is for another platform"),
            Self::Expired => f.write_str("the update manifest has expired"),
            Self::Downgrade => f.write_str("the update manifest names an older version"),
        }
    }
}

impl std::error::Error for CheckRefusal {}

/// Takes the manifest `bytes` through every check, in the order
/// `docs/formats/update-manifest.md` fixes: its signature under `key`, its
/// platform against `installed`, its expiry against `now`, in seconds since
/// 1970-01-01 UTC, its version against `installed`, and last this install's
/// inclusion under its rollout by `draw`. Nothing is taken from a manifest
/// before its signature verifies.
pub fn decide(
    bytes: &[u8],
    key: &UpdateKey,
    installed: Installed<'_>,
    now: u64,
    draw: RolloutDraw,
) -> Result<Decision, CheckRefusal> {
    let manifest = VerifiedManifest::verify(bytes, key).map_err(CheckRefusal::Manifest)?;
    if manifest.platform() != installed.platform {
        return Err(CheckRefusal::Platform);
    }
    if now > manifest.not_after() {
        return Err(CheckRefusal::Expired);
    }
    match manifest.version().cmp(&installed.version) {
        std::cmp::Ordering::Less => Err(CheckRefusal::Downgrade),
        std::cmp::Ordering::Equal => Ok(Decision::UpToDate),
        std::cmp::Ordering::Greater if draw.included(manifest.rollout()) => {
            Ok(Decision::Offered(manifest))
        }
        std::cmp::Ordering::Greater => Ok(Decision::NotIncluded),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_compiled_wake_is_the_files() {
        let budgets = include_str!("../../../budgets.toml");
        assert_eq!(
            Ok(UPDATE_CHECK_WAKE),
            wake::read(budgets, wake::UPDATE_CHECK)
        );
    }
}
