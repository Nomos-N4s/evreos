//! Integration tests for theme management, system preference tracking, and overrides (T055).
//!
//! # Covered Requirements (FR-010, User Story 1)
//! 1. The system preference is followed on a fresh profile by default.
//! 2. A change of system preference is picked up dynamically while the override is unset.
//! 3. An override survives restart (persisted in profile.toml across close and reopen).

#![forbid(unsafe_code)]

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use evreos_shell::{
    MockSystemThemeSource, Profile, ProfileError, SystemThemeSource, ThemeCoordinator,
    ThemeManager, ThemePreference, ThemePresentation,
};
use winit::event::WindowEvent;
use winit::window::Theme as WinitTheme;

fn unique_temp_dir() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("test_theme_{now:016x}_{count:08x}"));
    fs::create_dir_all(&dir).expect("create temp test dir");
    dir
}

#[test]
fn system_preference_is_followed_on_a_fresh_profile() {
    let temp_dir = unique_temp_dir();
    let profile = Profile::new(&temp_dir);

    // 1. Fresh profile defaults to ThemePreference::System
    assert_eq!(profile.theme_preference, ThemePreference::System);

    // 2. Direct Profile query follows system preference
    assert_eq!(
        profile.effective_theme(ThemePresentation::Light),
        ThemePresentation::Light
    );
    assert_eq!(
        profile.effective_theme(ThemePresentation::Dark),
        ThemePresentation::Dark
    );

    // 3. ThemeCoordinator initialized from fresh profile follows system Light
    let coordinator_light = ThemeCoordinator::from_profile(&profile, ThemePresentation::Light);
    assert_eq!(coordinator_light.presentation(), ThemePresentation::Light);
    assert_eq!(
        coordinator_light.effective_theme(),
        ThemePresentation::Light
    );
    assert_eq!(
        coordinator_light.system_preference(),
        ThemePresentation::Light
    );
    assert_eq!(
        coordinator_light.override_preference(),
        ThemePreference::System
    );
    assert!(!coordinator_light.has_override());

    // 4. ThemeCoordinator initialized from fresh profile follows system Dark
    let coordinator_dark = ThemeCoordinator::from_profile(&profile, ThemePresentation::Dark);
    assert_eq!(coordinator_dark.presentation(), ThemePresentation::Dark);
    assert_eq!(coordinator_dark.effective_theme(), ThemePresentation::Dark);
    assert_eq!(
        coordinator_dark.system_preference(),
        ThemePresentation::Dark
    );
    assert_eq!(
        coordinator_dark.override_preference(),
        ThemePreference::System
    );
    assert!(!coordinator_dark.has_override());

    // 5. Direct ThemeCoordinator::new follows system preference
    assert_eq!(
        ThemeCoordinator::new(ThemePresentation::Light).presentation(),
        ThemePresentation::Light
    );
    assert_eq!(
        ThemeCoordinator::new(ThemePresentation::Dark).presentation(),
        ThemePresentation::Dark
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn change_of_system_preference_is_picked_up_while_override_is_unset() {
    let temp_dir = unique_temp_dir();
    let profile = Profile::new(&temp_dir);
    assert_eq!(profile.theme_preference, ThemePreference::System);

    let mut coordinator = ThemeCoordinator::from_profile(&profile, ThemePresentation::Light);
    assert_eq!(coordinator.presentation(), ThemePresentation::Light);
    assert!(!coordinator.has_override());

    // Change system preference to Dark -> updates presentation to Dark
    let changed = coordinator.update_system_preference(ThemePresentation::Dark);
    assert!(changed, "effective presentation should have changed");
    assert_eq!(coordinator.system_preference(), ThemePresentation::Dark);
    assert_eq!(coordinator.presentation(), ThemePresentation::Dark);

    // Redundant system update to Dark -> no change
    let changed = coordinator.update_system_preference(ThemePresentation::Dark);
    assert!(!changed, "redundant update should report no change");
    assert_eq!(coordinator.presentation(), ThemePresentation::Dark);

    // Change system preference back to Light -> updates presentation to Light
    let changed = coordinator.update_system_preference(ThemePresentation::Light);
    assert!(changed, "effective presentation should have changed back");
    assert_eq!(coordinator.system_preference(), ThemePresentation::Light);
    assert_eq!(coordinator.presentation(), ThemePresentation::Light);

    // Verify polling via SystemThemeSource trait with MockSystemThemeSource
    let mut mock_source = MockSystemThemeSource::new(ThemePresentation::Dark);
    assert_eq!(mock_source.current_system_theme(), ThemePresentation::Dark);

    let changed = coordinator.poll_system_source(&mock_source);
    assert!(changed);
    assert_eq!(coordinator.presentation(), ThemePresentation::Dark);

    mock_source.set_theme(ThemePresentation::Light);
    let changed = coordinator.poll_system_source(&mock_source);
    assert!(changed);
    assert_eq!(coordinator.presentation(), ThemePresentation::Light);

    // Verify window event dispatch handling (WindowEvent::ThemeChanged)
    let event_dark = WindowEvent::ThemeChanged(WinitTheme::Dark);
    let changed = coordinator.handle_window_event(&event_dark);
    assert!(changed);
    assert_eq!(coordinator.presentation(), ThemePresentation::Dark);

    let event_light = WindowEvent::ThemeChanged(WinitTheme::Light);
    let changed = coordinator.handle_window_event(&event_light);
    assert!(changed);
    assert_eq!(coordinator.presentation(), ThemePresentation::Light);

    // Unrelated window events leave theme untouched
    let focused_event = WindowEvent::Focused(true);
    let changed = coordinator.handle_window_event(&focused_event);
    assert!(!changed);
    assert_eq!(coordinator.presentation(), ThemePresentation::Light);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn override_survives_restart() {
    let temp_dir = unique_temp_dir();

    // 1. Initial run: Create profile, set Dark override and persist
    {
        let mut profile = Profile::new(&temp_dir);
        profile.save().expect("initial profile save");

        let mut coordinator = ThemeCoordinator::from_profile(&profile, ThemePresentation::Light);
        assert_eq!(coordinator.presentation(), ThemePresentation::Light);

        // Set override to Dark and persist
        let changed = coordinator
            .set_override_and_persist(ThemePreference::Dark, &mut profile)
            .expect("persist override");
        assert!(changed);
        assert_eq!(coordinator.presentation(), ThemePresentation::Dark);
        assert_eq!(coordinator.override_preference(), ThemePreference::Dark);
        assert!(coordinator.has_override());

        // Verify that when override is set, system changes do NOT alter presentation
        let changed = coordinator.update_system_preference(ThemePresentation::Light);
        assert!(!changed, "override must mask system preference changes");
        assert_eq!(coordinator.presentation(), ThemePresentation::Dark);

        let changed = coordinator.update_system_preference(ThemePresentation::Dark);
        assert!(!changed);
        assert_eq!(coordinator.presentation(), ThemePresentation::Dark);

        // Profile closes / process terminates
        profile.close().expect("profile close");
    }

    // 2. Restart simulation: Reopen profile from disk
    {
        let mut reopened = Profile::open(&temp_dir).expect("reopen profile from disk");
        assert_eq!(reopened.theme_preference, ThemePreference::Dark);

        // Recreated coordinator must restore Dark presentation even with system Light
        let mut coordinator = ThemeCoordinator::from_profile(&reopened, ThemePresentation::Light);
        assert_eq!(coordinator.presentation(), ThemePresentation::Dark);
        assert_eq!(coordinator.override_preference(), ThemePreference::Dark);
        assert!(coordinator.has_override());

        // System preference change while override is active still preserves Dark
        coordinator.update_system_preference(ThemePresentation::Dark);
        assert_eq!(coordinator.presentation(), ThemePresentation::Dark);
        coordinator.update_system_preference(ThemePresentation::Light);
        assert_eq!(coordinator.presentation(), ThemePresentation::Dark);

        // WindowEvent::ThemeChanged also does not move presentation when override is active
        let event = WindowEvent::ThemeChanged(WinitTheme::Light);
        let changed = coordinator.handle_window_event(&event);
        assert!(!changed);
        assert_eq!(coordinator.presentation(), ThemePresentation::Dark);

        // Now change override to Light and persist
        let changed = coordinator
            .set_override_and_persist(ThemePreference::Light, &mut reopened)
            .expect("persist light override");
        assert!(changed);
        assert_eq!(coordinator.presentation(), ThemePresentation::Light);
        assert_eq!(coordinator.override_preference(), ThemePreference::Light);

        reopened.close().expect("profile close");
    }

    // 3. Second restart: Reopen profile and verify Light override survived
    {
        let mut reopened = Profile::open(&temp_dir).expect("reopen profile second time");
        assert_eq!(reopened.theme_preference, ThemePreference::Light);

        // With system preference Dark, Light override still governs presentation
        let mut coordinator = ThemeCoordinator::from_profile(&reopened, ThemePresentation::Dark);
        assert_eq!(coordinator.presentation(), ThemePresentation::Light);
        assert_eq!(coordinator.override_preference(), ThemePreference::Light);
        assert!(coordinator.has_override());

        // Now clear the override (revert to System) and persist
        let changed = coordinator
            .clear_override_and_persist(&mut reopened)
            .expect("persist clear override");
        assert!(
            changed,
            "clearing override when system is dark should flip presentation to dark"
        );
        assert_eq!(coordinator.presentation(), ThemePresentation::Dark);
        assert_eq!(coordinator.override_preference(), ThemePreference::System);
        assert!(!coordinator.has_override());

        reopened.close().expect("profile close");
    }

    // 4. Third restart: Reopen profile and verify reversion to System survived
    {
        let reopened = Profile::open(&temp_dir).expect("reopen profile third time");
        assert_eq!(reopened.theme_preference, ThemePreference::System);

        let mut coordinator = ThemeCoordinator::from_profile(&reopened, ThemePresentation::Light);
        assert_eq!(coordinator.presentation(), ThemePresentation::Light);
        assert!(!coordinator.has_override());

        // System changes work dynamically again
        let changed = coordinator.update_system_preference(ThemePresentation::Dark);
        assert!(changed);
        assert_eq!(coordinator.presentation(), ThemePresentation::Dark);

        let changed = coordinator.update_system_preference(ThemePresentation::Light);
        assert!(changed);
        assert_eq!(coordinator.presentation(), ThemePresentation::Light);
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn direct_profile_set_theme_preference_persists_override() {
    let temp_dir = unique_temp_dir();

    let mut profile = Profile::new(&temp_dir);
    profile.save().expect("initial save");

    profile
        .set_theme_preference(ThemePreference::Dark)
        .expect("set dark preference");

    let reopened = Profile::open(&temp_dir).expect("reopen profile");
    assert_eq!(reopened.theme_preference, ThemePreference::Dark);
    assert_eq!(
        reopened.effective_theme(ThemePresentation::Light),
        ThemePresentation::Dark
    );
    assert_eq!(
        reopened.effective_theme(ThemePresentation::Dark),
        ThemePresentation::Dark
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn theme_presentation_and_preference_properties_and_parsing() {
    // ThemePresentation properties
    assert!(ThemePresentation::Light.is_light());
    assert!(!ThemePresentation::Light.is_dark());
    assert_eq!(ThemePresentation::Light.as_str(), "light");
    assert_eq!(format!("{}", ThemePresentation::Light), "light");

    assert!(ThemePresentation::Dark.is_dark());
    assert!(!ThemePresentation::Dark.is_light());
    assert_eq!(ThemePresentation::Dark.as_str(), "dark");
    assert_eq!(format!("{}", ThemePresentation::Dark), "dark");

    assert_eq!(ThemePresentation::default(), ThemePresentation::Light);

    // ThemePreference defaults and string parsing
    assert_eq!(ThemePreference::default(), ThemePreference::System);
    assert_eq!(ThemePreference::System.as_str(), "system");
    assert_eq!(ThemePreference::Light.as_str(), "light");
    assert_eq!(ThemePreference::Dark.as_str(), "dark");

    assert_eq!(
        ThemePreference::parse("system").expect("valid parse"),
        ThemePreference::System
    );
    assert_eq!(
        ThemePreference::parse("  SYSTEM  ").expect("valid parse"),
        ThemePreference::System
    );
    assert_eq!(
        ThemePreference::parse("light").expect("valid parse"),
        ThemePreference::Light
    );
    assert_eq!(
        ThemePreference::parse("LiGhT").expect("valid parse"),
        ThemePreference::Light
    );
    assert_eq!(
        ThemePreference::parse("dark").expect("valid parse"),
        ThemePreference::Dark
    );
    assert_eq!(
        ThemePreference::parse("DARK").expect("valid parse"),
        ThemePreference::Dark
    );

    // Invalid format errors
    assert!(matches!(
        ThemePreference::parse("invalid"),
        Err(ProfileError::InvalidFormat(_))
    ));
    assert!(matches!(
        ThemePreference::parse(""),
        Err(ProfileError::InvalidFormat(_))
    ));

    // Conversions with winit::window::Theme
    assert_eq!(
        ThemePresentation::from(WinitTheme::Light),
        ThemePresentation::Light
    );
    assert_eq!(
        ThemePresentation::from(WinitTheme::Dark),
        ThemePresentation::Dark
    );
    assert_eq!(
        WinitTheme::from(ThemePresentation::Light),
        WinitTheme::Light
    );
    assert_eq!(WinitTheme::from(ThemePresentation::Dark), WinitTheme::Dark);

    // Type alias parity
    let manager = ThemeManager::new(ThemePresentation::Dark);
    assert_eq!(manager.presentation(), ThemePresentation::Dark);
}
