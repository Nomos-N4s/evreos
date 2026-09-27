//! An offered update's artefact, checked against its manifest before
//! anything applies it.

use std::fmt;
use std::io::{self, Read};

use sha2::{Digest, Sha256};

use super::Offer;
use super::manifest::Version;

/// An artefact whose length and SHA-256 matched its offered manifest. Only
/// [`verify`] makes one, so holding one means the bytes read were the ones
/// the manifest's signer published.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifiedArtefact {
    version: Version,
}

impl VerifiedArtefact {
    /// The version the artefact installs.
    pub fn version(&self) -> Version {
        self.version
    }
}

/// Why an artefact was refused.
#[derive(Debug)]
pub enum ArtefactRefusal {
    /// It is longer or shorter than the manifest says.
    Size,
    /// Its SHA-256 is not the manifest's.
    Digest,
    /// Its manifest is past its `not after` now, though it was not when it
    /// was offered.
    Expired,
    /// It could not be read.
    Read(io::Error),
}

impl fmt::Display for ArtefactRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Size => f.write_str("the update artefact's length is not the manifest's"),
            Self::Digest => f.write_str("the update artefact's SHA-256 is not the manifest's"),
            Self::Expired => f.write_str("the update artefact's manifest has expired"),
            Self::Read(error) => write!(f, "the update artefact could not be read: {error}"),
        }
    }
}

impl std::error::Error for ArtefactRefusal {}

/// Reads the artefact from `reader` to its end and checks its length and
/// SHA-256 against the manifest `offer` holds, which only a manifest
/// [`decide`](super::decide) offered to this install can reach. Reading stops as soon as it passes the
/// manifest's length, so an artefact far longer than stated is not read
/// whole.
///
/// `now`, in seconds since 1970-01-01 UTC, is checked against the
/// manifest's `not after` again before anything is read, since an offer
/// can be held while its artefact is fetched, and the manifest may expire
/// in that time.
///
/// The check covers the bytes read here. What applies the update must apply
/// those bytes, from a copy only the updater writes, and not reopen a path
/// another process could have written to since.
pub fn verify(
    offer: &Offer,
    now: u64,
    mut reader: impl Read,
) -> Result<VerifiedArtefact, ArtefactRefusal> {
    let manifest = offer.manifest();
    if now > manifest.not_after() {
        return Err(ArtefactRefusal::Expired);
    }
    let mut hasher = Sha256::new();
    let mut read: u64 = 0;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(ArtefactRefusal::Read(error)),
        };
        read += count as u64;
        if read > manifest.artefact_size() {
            return Err(ArtefactRefusal::Size);
        }
        hasher.update(&buffer[..count]);
    }
    if read != manifest.artefact_size() {
        return Err(ArtefactRefusal::Size);
    }
    if hasher.finalize().as_slice() != manifest.artefact_digest() {
        return Err(ArtefactRefusal::Digest);
    }
    Ok(VerifiedArtefact {
        version: manifest.version(),
    })
}
