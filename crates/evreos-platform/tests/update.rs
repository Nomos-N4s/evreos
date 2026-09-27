//! FR-014 self-update: the signed manifest, verification against the pinned
//! key, and the refusals `docs/formats/update-manifest.md` fixes.
//!
//! The manifests here are built from the format document by this file's own
//! encoder, not by the crate, so the tests hold the crate to the document.
#![forbid(unsafe_code)]

use ed25519_dalek::{Signer, SigningKey};
use evreos_platform::update::manifest::{Refusal, UpdateKey, VerifiedManifest, Version};

const SEED: [u8; 32] = [7; 32];
const OTHER_SEED: [u8; 32] = [9; 32];

/// One manifest's fields, encoded as the format document lays them out.
#[derive(Clone)]
struct Fields {
    domain: Vec<u8>,
    platform: Vec<u8>,
    version: [u32; 3],
    size: u64,
    digest: [u8; 32],
    rollout: u32,
    not_after: u64,
}

impl Default for Fields {
    fn default() -> Self {
        Self {
            domain: b"evreos.update.v1\0".to_vec(),
            platform: b"windows-x86_64".to_vec(),
            version: [1, 2, 3],
            size: 4096,
            digest: [0xab; 32],
            rollout: 250_000,
            not_after: 4_000_000_000,
        }
    }
}

impl Fields {
    fn preimage(&self) -> Vec<u8> {
        let mut out = self.domain.clone();
        out.extend_from_slice(&(self.platform.len() as u16).to_be_bytes());
        out.extend_from_slice(&self.platform);
        for part in self.version {
            out.extend_from_slice(&part.to_be_bytes());
        }
        out.extend_from_slice(&self.size.to_be_bytes());
        out.extend_from_slice(&self.digest);
        out.extend_from_slice(&self.rollout.to_be_bytes());
        out.extend_from_slice(&self.not_after.to_be_bytes());
        out
    }

    fn signed_with(&self, seed: &[u8; 32]) -> Vec<u8> {
        let preimage = self.preimage();
        let signature = SigningKey::from_bytes(seed).sign(&preimage);
        let mut out = preimage;
        out.extend_from_slice(&signature.to_bytes());
        out
    }

    fn signed(&self) -> Vec<u8> {
        self.signed_with(&SEED)
    }
}

fn key() -> UpdateKey {
    UpdateKey::from_bytes(SigningKey::from_bytes(&SEED).verifying_key().as_bytes()).unwrap()
}

#[test]
fn a_signed_manifest_verifies_and_reads_back_its_fields() {
    let manifest = VerifiedManifest::verify(&Fields::default().signed(), &key()).unwrap();
    assert_eq!(manifest.platform(), "windows-x86_64");
    assert_eq!(
        manifest.version(),
        Version {
            major: 1,
            minor: 2,
            patch: 3
        }
    );
    assert_eq!(manifest.artefact_size(), 4096);
    assert_eq!(manifest.artefact_digest(), &[0xab; 32]);
    assert_eq!(manifest.rollout(), 250_000);
    assert_eq!(manifest.not_after(), 4_000_000_000);
}

#[test]
fn verification_fails_for_any_changed_byte_or_another_key() {
    let signed = Fields::default().signed();
    for index in 0..signed.len() {
        let mut changed = signed.clone();
        changed[index] ^= 0x01;
        assert!(
            VerifiedManifest::verify(&changed, &key()).is_err(),
            "a change at byte {index} verified"
        );
    }
    assert_eq!(
        VerifiedManifest::verify(&Fields::default().signed_with(&OTHER_SEED), &key()),
        Err(Refusal::Signature)
    );
}

#[test]
fn a_signature_made_under_another_domain_is_refused() {
    // research §6.1's surface domain, signed with the right key: the domain
    // keeps it from ever passing as an update's.
    let surface = Fields {
        domain: b"evreos.surface.v1\0".to_vec(),
        ..Fields::default()
    };
    assert_eq!(
        VerifiedManifest::verify(&surface.signed(), &key()),
        Err(Refusal::Malformed("domain"))
    );
}

#[test]
fn a_forgery_under_a_weak_key_is_refused() {
    // With the identity point as the key, R the identity and s zero satisfy
    // Ed25519's equation for every message: plain verification accepts the
    // forgery, and strict verification, which rejects small-order points,
    // refuses it.
    let mut identity = [0u8; 32];
    identity[0] = 1;
    let weak = UpdateKey::from_bytes(&identity).unwrap();
    let mut forged = Fields::default().preimage();
    forged.extend_from_slice(&identity);
    forged.extend_from_slice(&[0u8; 32]);
    assert_eq!(
        VerifiedManifest::verify(&forged, &weak),
        Err(Refusal::Signature)
    );
}

#[test]
fn malformed_manifests_are_refused_before_their_signature_is_checked() {
    let cases: Vec<(Fields, &str)> = vec![
        (
            Fields {
                platform: Vec::new(),
                ..Fields::default()
            },
            "platform length",
        ),
        (
            Fields {
                platform: vec![b'x'; 65],
                ..Fields::default()
            },
            "platform length",
        ),
        (
            Fields {
                platform: b"windows x86".to_vec(),
                ..Fields::default()
            },
            "platform",
        ),
        (
            Fields {
                size: 0,
                ..Fields::default()
            },
            "artefact size",
        ),
        (
            Fields {
                rollout: 1_000_001,
                ..Fields::default()
            },
            "rollout",
        ),
    ];
    for (fields, field) in cases {
        assert_eq!(
            VerifiedManifest::verify(&fields.signed(), &key()),
            Err(Refusal::Malformed(field))
        );
    }

    let signed = Fields::default().signed();
    assert_eq!(
        VerifiedManifest::verify(&signed[..signed.len() - 1], &key()),
        Err(Refusal::Malformed("not after"))
    );
    let mut longer = Fields::default().preimage();
    longer.push(0);
    let signature = SigningKey::from_bytes(&SEED).sign(&longer);
    longer.extend_from_slice(&signature.to_bytes());
    assert_eq!(
        VerifiedManifest::verify(&longer, &key()),
        Err(Refusal::Malformed("length"))
    );
    // A preimage cut inside the version, before an intact signature.
    let mut cut = signed[..40].to_vec();
    cut.extend_from_slice(&signed[signed.len() - 64..]);
    assert_eq!(
        VerifiedManifest::verify(&cut, &key()),
        Err(Refusal::Malformed("version"))
    );
    assert_eq!(
        VerifiedManifest::verify(&[], &key()),
        Err(Refusal::Malformed("signature"))
    );
}

#[test]
fn the_widest_rollout_and_longest_platform_are_accepted() {
    let fields = Fields {
        platform: vec![b'a'; 64],
        rollout: 1_000_000,
        ..Fields::default()
    };
    let manifest = VerifiedManifest::verify(&fields.signed(), &key()).unwrap();
    assert_eq!(manifest.rollout(), 1_000_000);
}
