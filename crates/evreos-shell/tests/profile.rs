//! Integration tests for profile entity, persistence, and invariants (T040).
//!
//! # Covered Invariants
//! 1. The profile round-trips across close and reopen.
//! 2. No field fuses language and place (FR-035, Principle VII).
//! 3. No file under the profile root holds an account credential or anything from
//!    which one could be reconstructed (FR-023).

#![forbid(unsafe_code)]

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use evreos_i18n::{Language, Place};
use evreos_shell::{HandOffBrowser, Profile, ProfileError, SearchProviderSetting, ThemePreference};

fn unique_temp_dir() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("test_profile_{now:016x}_{count:08x}"));
    fs::create_dir_all(&dir).expect("create temp test dir");
    dir
}

#[test]
fn profile_round_trips_across_close_and_reopen() {
    let temp_dir = unique_temp_dir();

    let mut profile = Profile::new(&temp_dir);
    let original_id = profile.profile_id.clone();

    // Set non-default fields across all configurable settings
    profile.language = Language::De;
    profile.place = Place::new("DE").expect("valid place");
    profile.theme_preference = ThemePreference::Dark;
    profile.set_ui_scale(150).expect("valid scale");
    profile.default_search_provider =
        SearchProviderSetting::new("CustomPrivacySearch", "https://example.invalid/search");
    profile.hand_off_browser =
        HandOffBrowser::nominated("external-system-browser").expect("valid browser");
    profile.diagnostics_enabled = false;
    profile.crash_reporting_enabled = false;
    profile.home_surface_hidden = true;

    // Verify stores access
    assert_eq!(profile.stores().root_path(), temp_dir.as_path());

    // Close / save to disk
    profile.close().expect("profile save/close succeeds");

    // Reopen from disk
    let reopened = Profile::open(&temp_dir).expect("reopen profile succeeds");

    assert_eq!(reopened.profile_id, original_id);
    assert_eq!(reopened.root_path, temp_dir);
    assert_eq!(reopened.language, Language::De);
    assert_eq!(reopened.place, Place::new("DE").expect("valid place"));
    assert_eq!(reopened.theme_preference, ThemePreference::Dark);
    assert_eq!(reopened.ui_scale, 150);
    assert_eq!(
        reopened.default_search_provider.provider,
        "CustomPrivacySearch"
    );
    assert_eq!(
        reopened.default_search_provider.endpoint,
        "https://example.invalid/search"
    );
    assert_eq!(
        reopened.hand_off_browser,
        HandOffBrowser::Nominated {
            program: "external-system-browser".to_string()
        }
    );
    assert!(!reopened.diagnostics_enabled);
    assert!(!reopened.crash_reporting_enabled);
    assert!(reopened.home_surface_hidden);
    assert_eq!(reopened.stores().root_path(), temp_dir.as_path());

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn no_field_fuses_language_and_place() {
    let temp_dir = unique_temp_dir();

    // Verify in memory that language and place are strongly typed and distinct
    let mut profile = Profile::new(&temp_dir);
    profile.language = Language::El;
    profile.place = Place::new("GR").expect("valid place");
    profile.save().expect("profile save succeeds");

    let config_path = profile.config_path();
    let content = fs::read_to_string(&config_path).expect("read profile config");

    let mut found_language_line = false;
    let mut found_place_line = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("language =") {
            found_language_line = true;
            // Primary subtag alone
            assert_eq!(trimmed, "language = \"el\"");
        } else if trimmed.starts_with("place =") {
            found_place_line = true;
            // Place code alone
            assert_eq!(trimmed, "place = \"GR\"");
        }

        // Assert language and place are separate lines and never appear together on any line
        let sub = profile.language.subtag();
        let plc = profile.place.code();
        assert!(
            !(trimmed.contains(sub) && trimmed.contains(plc)),
            "line {trimmed:?} fuses language and place into one entry"
        );
        assert!(
            !trimmed.contains("locale="),
            "line {trimmed:?} contains fused locale key"
        );
    }

    assert!(found_language_line, "language setting line must exist");
    assert!(found_place_line, "place setting line must exist");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn profile_root_holds_no_account_credentials_or_derived_tokens() {
    let temp_dir = unique_temp_dir();

    let profile = Profile::new(&temp_dir);
    profile.save().expect("profile save succeeds");

    // Walk all files in the profile directory
    let mut checked_files = 0;
    for entry in fs::read_dir(&temp_dir).expect("read profile dir") {
        let entry = entry.expect("valid entry");
        let path = entry.path();
        if path.is_file() {
            checked_files += 1;
            let file_name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();

            // Filename assertion
            for forbidden_word in [
                "credential",
                "password",
                "secret",
                "auth_token",
                "bearer",
                "session_secret",
            ] {
                assert!(
                    !file_name.contains(forbidden_word),
                    "file name {file_name:?} contains forbidden credential marker {forbidden_word:?}"
                );
            }

            // File contents assertion
            let content = fs::read_to_string(&path).expect("read file content");
            let lower_content = content.to_lowercase();
            for forbidden_pattern in [
                "account_credential",
                "user_password",
                "auth_token",
                "bearer_token",
                "access_token",
                "refresh_token",
                "secret_key",
                "private_key",
            ] {
                assert!(
                    !lower_content.contains(forbidden_pattern),
                    "file content in {} contains forbidden pattern {:?}",
                    path.display(),
                    forbidden_pattern
                );
            }
        }
    }

    assert!(
        checked_files > 0,
        "at least profile.toml should have been checked"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn ui_scale_bounds_are_enforced() {
    let temp_dir = unique_temp_dir();
    let mut profile = Profile::new(&temp_dir);

    // 100% to 200% are valid
    assert!(profile.set_ui_scale(100).is_ok());
    assert!(profile.set_ui_scale(150).is_ok());
    assert!(profile.set_ui_scale(200).is_ok());

    // Out of bounds values are rejected
    assert!(matches!(
        profile.set_ui_scale(99),
        Err(ProfileError::InvalidUiScale(99))
    ));
    assert!(matches!(
        profile.set_ui_scale(201),
        Err(ProfileError::InvalidUiScale(201))
    ));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn handoff_browser_refuses_self_nomination() {
    // Cannot nominate "self"
    assert!(matches!(
        HandOffBrowser::nominated("self"),
        Err(ProfileError::InvalidHandOffBrowser(_))
    ));
    // Cannot be empty
    assert!(matches!(
        HandOffBrowser::nominated("   "),
        Err(ProfileError::InvalidHandOffBrowser(_))
    ));
    // Valid external program is accepted
    assert!(HandOffBrowser::nominated("chromium").is_ok());
}

#[test]
fn default_profile_flags_are_off() {
    let temp_dir = unique_temp_dir();
    let profile = Profile::new(&temp_dir);

    // Under FR-039 and FR-039c, telemetry and crash reporting default to off
    assert!(!profile.diagnostics_enabled);
    assert!(!profile.crash_reporting_enabled);
    assert!(!profile.home_surface_hidden);
    assert_eq!(profile.theme_preference, ThemePreference::System);
    assert_eq!(profile.ui_scale, 100);

    let _ = fs::remove_dir_all(&temp_dir);
}
