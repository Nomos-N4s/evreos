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

mod rollout {
    use std::fs;
    use std::path::PathBuf;

    use evreos_platform::update::rollout::RolloutDraw;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("evreos-rollout-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.join("rollout")
    }

    #[test]
    fn a_value_is_drawn_once_and_kept() {
        let path = scratch("once");
        let first = RolloutDraw::load_or_draw(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        let value: u32 = text.trim_end().parse().unwrap();
        assert!(value < 1_000_000, "{value}");
        assert_eq!(text, format!("{value}\n"));
        for _ in 0..3 {
            assert_eq!(RolloutDraw::load_or_draw(&path).unwrap(), first);
        }
        assert!(!path.with_extension("partial").exists());
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_kept_value_decides_inclusion() {
        let path = scratch("kept");
        fs::write(&path, "250000\n").unwrap();
        let draw = RolloutDraw::load_or_draw(&path).unwrap();
        // Included below the rollout, excluded at or above it.
        assert!(draw.included(250_001));
        assert!(!draw.included(250_000));
        assert!(!draw.included(0));
        assert!(draw.included(1_000_000));
        // Widening never drops an install already included.
        let mut was = false;
        for rollout in (0..=1_000_000).step_by(50_000) {
            let now = draw.included(rollout);
            assert!(now || !was, "rollout {rollout} dropped an included install");
            was = now;
        }
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn the_lowest_and_highest_values_decide_at_the_ends() {
        let path = scratch("ends");
        fs::write(&path, "0").unwrap();
        let lowest = RolloutDraw::load_or_draw(&path).unwrap();
        assert!(lowest.included(1) && !lowest.included(0));
        fs::write(&path, "999999\n").unwrap();
        let highest = RolloutDraw::load_or_draw(&path).unwrap();
        assert!(highest.included(1_000_000) && !highest.included(999_999));
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_file_that_is_not_a_value_is_replaced_by_a_new_draw() {
        let path = scratch("replaced");
        for text in [
            &b""[..],
            b"1000000\n",
            b"-1\n",
            b"12 34\n",
            b"0x10\n",
            b"250000\n\n",
            b" 5\n",
            b"\xff\xfe1\n",
            b"25\xff\n",
        ] {
            fs::write(&path, text).unwrap();
            RolloutDraw::load_or_draw(&path).unwrap();
            let kept = fs::read_to_string(&path).unwrap();
            let value: u32 = kept.trim_end().parse().unwrap();
            assert!(value < 1_000_000, "{text:?} left {kept:?}");
            assert_eq!(kept, format!("{value}\n"), "{text:?}");
        }
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn debug_output_leaves_the_value_out() {
        let path = scratch("debug");
        fs::write(&path, "123456\n").unwrap();
        let draw = RolloutDraw::load_or_draw(&path).unwrap();
        let shown = format!("{draw:?} {draw:#?}");
        assert!(!shown.bytes().any(|b| b.is_ascii_digit()), "{shown}");
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn draws_spread_across_the_range() {
        // Not a test of randomness, only that draws are not stuck: a
        // hundred draws land in more than one tenth of the range.
        let mut tenths = [false; 10];
        for index in 0..100 {
            let path = scratch(&format!("spread-{index}"));
            let draw = RolloutDraw::load_or_draw(&path).unwrap();
            let tenth = (0..10).find(|t| draw.included((t + 1) * 100_000)).unwrap();
            tenths[tenth as usize] = true;
            let _ = fs::remove_dir_all(path.parent().unwrap());
        }
        assert!(tenths.iter().filter(|hit| **hit).count() > 1);
    }
}

mod decide {
    use std::fs;
    use std::sync::atomic::{AtomicU32, Ordering};

    use evreos_platform::update::manifest::{Refusal, Version};
    use evreos_platform::update::rollout::RolloutDraw;
    use evreos_platform::update::{CheckRefusal, Decision, Installed, decide};

    use super::{Fields, OTHER_SEED, key};

    const NOW: u64 = 3_000_000_000;

    fn draw(value: u32) -> RolloutDraw {
        // Each call its own directory: tests run at once, and two drawing the
        // same value must not remove each other's file.
        static CALLS: AtomicU32 = AtomicU32::new(0);
        let call = CALLS.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("evreos-decide-{}-{call}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rollout");
        fs::write(&path, format!("{value}\n")).unwrap();
        let draw = RolloutDraw::load_or_draw(&path).unwrap();
        let _ = fs::remove_dir_all(&dir);
        draw
    }

    fn installed(version: [u32; 3]) -> Installed<'static> {
        Installed {
            platform: "windows-x86_64",
            version: Version {
                major: version[0],
                minor: version[1],
                patch: version[2],
            },
        }
    }

    fn decided(
        fields: &Fields,
        version: [u32; 3],
        draw_value: u32,
    ) -> Result<Decision, CheckRefusal> {
        decide(
            &fields.signed(),
            &key(),
            installed(version),
            NOW,
            draw(draw_value),
        )
    }

    #[test]
    fn a_newer_version_is_offered_to_an_install_inside_the_rollout() {
        // Rollout 250,000 millionths: the install drawing 249,999 is in,
        // the one drawing 250,000 is not.
        let fields = Fields::default();
        match decided(&fields, [1, 2, 2], 249_999).unwrap() {
            Decision::Offered(manifest) => assert_eq!(manifest.rollout(), 250_000),
            other => panic!("{other:?}"),
        }
        assert_eq!(
            decided(&fields, [1, 2, 2], 250_000),
            Ok(Decision::NotIncluded)
        );
        assert_eq!(
            decided(
                &Fields {
                    rollout: 0,
                    ..Fields::default()
                },
                [1, 0, 0],
                0
            ),
            Ok(Decision::NotIncluded)
        );
    }

    #[test]
    fn the_installed_version_is_up_to_date_whatever_the_rollout() {
        assert_eq!(
            decided(&Fields::default(), [1, 2, 3], 0),
            Ok(Decision::UpToDate)
        );
        assert_eq!(
            decided(&Fields::default(), [1, 2, 3], 999_999),
            Ok(Decision::UpToDate)
        );
    }

    #[test]
    fn an_older_version_is_refused_as_a_downgrade_even_rolled_out_to_all() {
        // Versions compare by major, then minor, then patch, as numbers:
        // 1.9.9 is older than 1.10.0 and 2.0.0, newer than 1.9.8.
        let manifest = Fields {
            version: [1, 9, 9],
            rollout: 1_000_000,
            ..Fields::default()
        };
        for installed in [[1, 9, 10], [1, 10, 0], [2, 0, 0]] {
            assert_eq!(
                decided(&manifest, installed, 0),
                Err(CheckRefusal::Downgrade),
                "{installed:?}"
            );
        }
        assert!(matches!(
            decided(&manifest, [1, 9, 8], 0),
            Ok(Decision::Offered(_))
        ));
    }

    #[test]
    fn a_manifest_for_another_platform_or_past_its_expiry_is_refused() {
        let other = Fields {
            platform: b"macos-aarch64".to_vec(),
            ..Fields::default()
        };
        assert_eq!(decided(&other, [1, 0, 0], 0), Err(CheckRefusal::Platform));
        let expiring = Fields {
            not_after: NOW,
            ..Fields::default()
        };
        assert!(decided(&expiring, [1, 0, 0], 0).is_ok());
        let expired = Fields {
            not_after: NOW - 1,
            ..Fields::default()
        };
        assert_eq!(decided(&expired, [1, 0, 0], 0), Err(CheckRefusal::Expired));
    }

    #[test]
    fn verification_failure_comes_before_every_other_check() {
        // Another key, and every other check failing too: the signature is
        // what is reported, since nothing unverified is looked at.
        let everything_wrong = Fields {
            platform: b"macos-aarch64".to_vec(),
            not_after: 0,
            version: [0, 0, 1],
            ..Fields::default()
        };
        assert_eq!(
            decide(
                &everything_wrong.signed_with(&OTHER_SEED),
                &key(),
                installed([1, 0, 0]),
                NOW,
                draw(0)
            ),
            Err(CheckRefusal::Manifest(Refusal::Signature))
        );
    }
}

mod artefact {
    use std::io::{self, Read};

    use evreos_platform::update::artefact::{ArtefactRefusal, verify};
    use evreos_platform::update::manifest::VerifiedManifest;
    use sha2::{Digest, Sha256};

    use super::{Fields, key};

    const ARTEFACT: &[u8] = b"an installer, standing in for the real one";

    fn manifest_for(bytes: &[u8]) -> VerifiedManifest {
        let fields = Fields {
            size: bytes.len() as u64,
            digest: Sha256::digest(bytes).into(),
            ..Fields::default()
        };
        VerifiedManifest::verify(&fields.signed(), &key()).unwrap()
    }

    #[test]
    fn the_published_artefact_verifies() {
        let manifest = manifest_for(ARTEFACT);
        let verified = verify(&manifest, ARTEFACT).unwrap();
        assert_eq!(verified.version(), manifest.version());
        // Read a byte at a time, it verifies the same.
        let one_at_a_time = io::BufReader::with_capacity(1, ARTEFACT);
        verify(&manifest, one_at_a_time).unwrap();
    }

    #[test]
    fn any_changed_byte_is_refused() {
        let manifest = manifest_for(ARTEFACT);
        for index in 0..ARTEFACT.len() {
            let mut changed = ARTEFACT.to_vec();
            changed[index] ^= 0x01;
            assert!(matches!(
                verify(&manifest, changed.as_slice()),
                Err(ArtefactRefusal::Digest)
            ));
        }
    }

    #[test]
    fn a_shorter_or_longer_artefact_is_refused() {
        let manifest = manifest_for(ARTEFACT);
        assert!(matches!(
            verify(&manifest, &ARTEFACT[..ARTEFACT.len() - 1]),
            Err(ArtefactRefusal::Size)
        ));
        let mut longer = ARTEFACT.to_vec();
        longer.push(0);
        assert!(matches!(
            verify(&manifest, longer.as_slice()),
            Err(ArtefactRefusal::Size)
        ));
    }

    #[test]
    fn a_far_longer_artefact_is_refused_once_past_its_length() {
        // A reader far longer than the manifest's length, which counts what
        // it gives; it ends after a mebibyte so that a verifier reading to
        // the end fails this test rather than hanging it.
        struct Endless(u64);
        impl Read for Endless {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                if self.0 >= 1 << 20 {
                    return Ok(0);
                }
                self.0 += buffer.len() as u64;
                buffer.fill(0);
                Ok(buffer.len())
            }
        }
        let manifest = manifest_for(ARTEFACT);
        let mut endless = Endless(0);
        assert!(matches!(
            verify(&manifest, &mut endless),
            Err(ArtefactRefusal::Size)
        ));
        assert!(endless.0 <= 64 * 1024, "read {} bytes", endless.0);
    }

    #[test]
    fn a_read_that_fails_is_refused() {
        struct Failing;
        impl Read for Failing {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("disk gone"))
            }
        }
        assert!(matches!(
            verify(&manifest_for(ARTEFACT), Failing),
            Err(ArtefactRefusal::Read(_))
        ));
    }
}

mod check_request {
    use std::fs;

    use evreos_net::{BrandResolved, Endpoint, NonHistory, Purpose};
    use evreos_platform::update::check_request;
    use evreos_platform::update::rollout::RolloutDraw;

    #[test]
    fn the_check_carries_its_purpose_and_endpoint_and_nothing_of_the_install() {
        let endpoint = || {
            Endpoint::resolve(BrandResolved::declared_in_brand_configuration(
                String::from("brand://update-host"),
            ))
        };
        let planned = check_request(endpoint());
        assert_eq!(
            planned.purpose(),
            &Purpose::NonHistory(NonHistory::UpdateCheck)
        );
        assert_eq!(planned.endpoint(), &endpoint());

        // The install's draw, a distinctive value, appears nowhere in what
        // the request holds.
        let dir = std::env::temp_dir().join(format!("evreos-check-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rollout");
        fs::write(&path, "731529\n").unwrap();
        let _draw = RolloutDraw::load_or_draw(&path).unwrap();
        let _ = fs::remove_dir_all(&dir);
        let held = format!("{planned:?}");
        assert!(!held.contains("731529"), "{held}");
        // The request is the same for every install: the same endpoint
        // plans an equal request whatever the install drew.
        assert_eq!(check_request(endpoint()), planned);
    }
}

mod schedule {
    use std::time::{Duration, SystemTime};

    use evreos_platform::update::UPDATE_CHECK_WAKE;
    use evreos_platform::update::schedule::Schedule;

    const HOUR: Duration = Duration::from_secs(3600);

    #[test]
    fn the_period_and_tolerance_come_from_the_budget_file() {
        let schedule = Schedule::new();
        assert_eq!(
            schedule.period(),
            Duration::from_secs(UPDATE_CHECK_WAKE.period_seconds)
        );
        assert_eq!(schedule.period(), 6 * HOUR);
        assert_eq!(schedule.tolerance(), Duration::from_secs(2160));
        assert_eq!(Schedule::default(), schedule);
    }

    #[test]
    fn a_first_check_is_due_at_once_and_each_later_one_a_period_on() {
        let schedule = Schedule::new();
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        assert_eq!(schedule.next_due(None, now), now);
        let last = now - HOUR;
        assert_eq!(schedule.next_due(Some(last), now), last + 6 * HOUR);
        // A check long overdue is due now, not a period from now.
        let long_ago = now - 48 * HOUR;
        assert!(schedule.next_due(Some(long_ago), now) <= now);
    }

    #[test]
    fn a_clock_moved_back_never_holds_checks_off_past_one_period() {
        let schedule = Schedule::new();
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        let future = now + 30 * 24 * HOUR;
        assert_eq!(schedule.next_due(Some(future), now), now + 6 * HOUR);
    }
}
