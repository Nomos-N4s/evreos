//! The signed update manifest, read and verified as
//! `docs/formats/update-manifest.md` fixes its bytes.

use std::fmt;

use ed25519_dalek::{Signature, VerifyingKey};

/// The domain string that opens every update manifest's preimage, so that a
/// signature made in another context, over an app surface say, is never
/// accepted as an update's.
pub const DOMAIN: &[u8] = b"evreos.update.v1\0";
/// The most an update may be rolled out to, in millionths: everyone.
pub const ROLLOUT_WHOLE: u32 = 1_000_000;
const SIGNATURE_LEN: usize = 64;
const PLATFORM_MAX: usize = 64;

/// The key update manifests are verified against, pinned in the shipped
/// binary as a build constant.
#[derive(Clone, Copy, Debug)]
pub struct UpdateKey(VerifyingKey);

impl UpdateKey {
    /// The key from its 32 bytes, or `None` if they are not a valid Ed25519
    /// public key.
    pub fn from_bytes(bytes: &[u8; 32]) -> Option<Self> {
        VerifyingKey::from_bytes(bytes).ok().map(Self)
    }
}

/// A version, compared by its major, then minor, then patch number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// A manifest whose signature verified under the pinned key. Only
/// [`VerifiedManifest::verify`] makes one, so holding one means the fields
/// are the signer's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedManifest {
    platform: String,
    version: Version,
    artefact_size: u64,
    artefact_digest: [u8; 32],
    rollout: u32,
    not_after: u64,
}

/// Why a manifest was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Its bytes are not an update manifest: the named field is missing,
    /// out of range, or followed by bytes no field accounts for.
    Malformed(&'static str),
    /// Its signature does not verify under the pinned key.
    Signature,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(field) => write!(f, "the update manifest's {field} is malformed"),
            Self::Signature => f.write_str("the update manifest's signature does not verify"),
        }
    }
}

impl std::error::Error for Refusal {}

impl VerifiedManifest {
    /// Reads `bytes` as an update manifest and verifies its signature under
    /// `key`, strictly. Nothing is taken from the manifest unless every byte
    /// is accounted for and the signature verifies.
    pub fn verify(bytes: &[u8], key: &UpdateKey) -> Result<Self, Refusal> {
        let split = bytes
            .len()
            .checked_sub(SIGNATURE_LEN)
            .ok_or(Refusal::Malformed("signature"))?;
        let (preimage, signature) = bytes.split_at(split);
        let manifest = Self::read(preimage)?;
        let signature =
            Signature::from_slice(signature).map_err(|_| Refusal::Malformed("signature"))?;
        key.0
            .verify_strict(preimage, &signature)
            .map_err(|_| Refusal::Signature)?;
        Ok(manifest)
    }

    fn read(preimage: &[u8]) -> Result<Self, Refusal> {
        let mut rest = preimage;
        if take(&mut rest, DOMAIN.len(), "domain")? != DOMAIN {
            return Err(Refusal::Malformed("domain"));
        }
        let platform_len = usize::from(u16::from_be_bytes(array(&mut rest, "platform length")?));
        if platform_len == 0 || platform_len > PLATFORM_MAX {
            return Err(Refusal::Malformed("platform length"));
        }
        let platform = take(&mut rest, platform_len, "platform")?;
        if !platform
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b'-' || *b == b'_')
        {
            return Err(Refusal::Malformed("platform"));
        }
        let version = Version {
            major: u32::from_be_bytes(array(&mut rest, "version")?),
            minor: u32::from_be_bytes(array(&mut rest, "version")?),
            patch: u32::from_be_bytes(array(&mut rest, "version")?),
        };
        let artefact_size = u64::from_be_bytes(array(&mut rest, "artefact size")?);
        if artefact_size == 0 {
            return Err(Refusal::Malformed("artefact size"));
        }
        let artefact_digest = array(&mut rest, "artefact digest")?;
        let rollout = u32::from_be_bytes(array(&mut rest, "rollout")?);
        if rollout > ROLLOUT_WHOLE {
            return Err(Refusal::Malformed("rollout"));
        }
        let not_after = u64::from_be_bytes(array(&mut rest, "not after")?);
        if !rest.is_empty() {
            return Err(Refusal::Malformed("length"));
        }
        Ok(Self {
            platform: String::from_utf8(platform.to_vec())
                .map_err(|_| Refusal::Malformed("platform"))?,
            version,
            artefact_size,
            artefact_digest,
            rollout,
            not_after,
        })
    }

    /// The build the update is for, as `windows-x86_64`.
    pub fn platform(&self) -> &str {
        &self.platform
    }

    /// The update's version.
    pub fn version(&self) -> Version {
        self.version
    }

    /// The artefact's length in bytes.
    pub fn artefact_size(&self) -> u64 {
        self.artefact_size
    }

    /// The artefact's SHA-256.
    pub fn artefact_digest(&self) -> &[u8; 32] {
        &self.artefact_digest
    }

    /// The share of installs offered the update, in millionths.
    pub fn rollout(&self) -> u32 {
        self.rollout
    }

    /// The last moment the manifest is accepted, in seconds since
    /// 1970-01-01 UTC.
    pub fn not_after(&self) -> u64 {
        self.not_after
    }
}

fn take<'a>(rest: &mut &'a [u8], len: usize, field: &'static str) -> Result<&'a [u8], Refusal> {
    if rest.len() < len {
        return Err(Refusal::Malformed(field));
    }
    let (taken, left) = rest.split_at(len);
    *rest = left;
    Ok(taken)
}

fn array<const N: usize>(rest: &mut &[u8], field: &'static str) -> Result<[u8; N], Refusal> {
    let mut out = [0; N];
    out.copy_from_slice(take(rest, N, field)?);
    Ok(out)
}
