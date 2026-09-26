//! Integration tests for FR-005 interface scaling and zoom.
//!
//! Under FR-005, SC-008, and research.md §5.6:
//! - `scaling.rs` is the workspace's single owner of the interface scale.
//! - A single member-facing value (100% to 200%) drives both the chrome layout scale
//!   and the engine's rasterisation scale from that same value.
//! - Three values exist and are never conflated:
//!   1. `ui_scale` (interface scale on Profile: 100%..=200%)
//!   2. `page_zoom` (per-site zoom in PerSiteZoomStore: 25%..=500%)
//!   3. `find_state` (per-tab search state in FindInPageState)
//! - At 100%, 150%, and 200%: nothing is clipped and content scales once rather
//!   than twice or not at all.
//! - Every chrome surface stays usable and legible at 200%.
//! - An interface-scale change leaves page zoom unchanged, while a page-zoom
//!   change alters page content alone.

#![forbid(unsafe_code)]

use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use evreos_shell::app::AppWindowId;
use evreos_shell::profile::Profile;
use evreos_shell::scaling::{
    ChromeSurfaceMetrics, FindInPageState, PageZoom, PerSiteZoomStore, ScalingError, UiScale,
};
use evreos_shell::site_key::SiteKey;
use evreos_shell::tabs::{Tab, TabId, TabLifecycle};

fn unique_temp_dir() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("test_scaling_{now:016x}_{count:08x}"));
    fs::create_dir_all(&dir).expect("create temp test dir");
    dir
}

#[test]
fn test_three_values_are_never_conflated() {
    let temp_dir = unique_temp_dir();
    let mut profile = Profile::new(&temp_dir);
    let mut zoom_store = PerSiteZoomStore::new();

    let mut tab = Tab::new(
        TabId::FIRST,
        AppWindowId::FIRST,
        0,
        "https://example.com/page",
        TabLifecycle::Live,
        None,
    );

    let site_a = SiteKey::from_str("https://example.com").expect("valid site key");
    let site_b = SiteKey::from_str("https://news.org").expect("valid site key");

    // 1. Initial independent state
    let initial_scale = UiScale::new(profile.ui_scale).expect("valid initial scale");
    assert_eq!(initial_scale.as_percentage(), 100);
    assert_eq!(zoom_store.get_zoom(&site_a).as_percentage(), 100);
    assert_eq!(tab.page_zoom(), 100);
    assert!(!tab.find_state().is_visible());
    assert_eq!(tab.find_state().query(), "");

    // 2. Configure per-site zoom for site_a and site_b
    let zoom_a = PageZoom::new(125).expect("valid zoom");
    let zoom_b = PageZoom::new(75).expect("valid zoom");
    zoom_store.set_zoom(site_a.clone(), zoom_a);
    zoom_store.set_zoom(site_b.clone(), zoom_b);
    tab.set_page_zoom(125);

    // 3. Configure per-tab find-in-page state
    tab.find_state_mut().open();
    tab.find_state_mut().set_query("privacy");
    tab.find_state_mut().set_matches(5, Some(1));

    // 4. Change interface scale (100% -> 150%) on profile
    profile.set_ui_scale(150).expect("valid 150 scale");
    let new_scale = UiScale::new(profile.ui_scale).expect("valid scale");
    assert_eq!(new_scale.as_percentage(), 150);

    // Assert that per-site zoom and per-tab find state remain completely untouched
    assert_eq!(
        zoom_store.get_zoom(&site_a).as_percentage(),
        125,
        "changing ui_scale must not alter per-site zoom for site_a"
    );
    assert_eq!(
        zoom_store.get_zoom(&site_b).as_percentage(),
        75,
        "changing ui_scale must not alter per-site zoom for site_b"
    );
    assert_eq!(
        tab.page_zoom(),
        125,
        "changing ui_scale must not alter tab page_zoom"
    );
    assert!(
        tab.find_state().is_visible(),
        "changing ui_scale must not alter find_state visibility"
    );
    assert_eq!(
        tab.find_state().query(),
        "privacy",
        "changing ui_scale must not alter find_state query"
    );
    assert_eq!(
        tab.find_state().match_count(),
        5,
        "changing ui_scale must not alter find_state matches"
    );
    assert_eq!(
        tab.find_state().active_match_ordinal(),
        Some(1),
        "changing ui_scale must not alter find_state active match"
    );

    // 5. Change per-site zoom for site_a (125% -> 175%)
    let zoom_a_new = PageZoom::new(175).expect("valid zoom");
    zoom_store.set_zoom(site_a.clone(), zoom_a_new);
    tab.set_page_zoom(175);

    // Assert that ui_scale and other site's zoom remain completely untouched
    assert_eq!(
        profile.ui_scale, 150,
        "changing per-site zoom must not alter profile ui_scale"
    );
    assert_eq!(
        zoom_store.get_zoom(&site_b).as_percentage(),
        75,
        "changing site_a zoom must not alter site_b zoom"
    );
    assert_eq!(
        tab.find_state().query(),
        "privacy",
        "changing page zoom must not alter find_state query"
    );

    // 6. Update find-in-page state on tab
    tab.find_state_mut().next_match();
    assert_eq!(tab.find_state().active_match_ordinal(), Some(2));
    assert_eq!(profile.ui_scale, 150);
    assert_eq!(zoom_store.get_zoom(&site_a).as_percentage(), 175);
}

#[test]
fn test_scaling_once_rather_than_twice_or_not_at_all() {
    // Both chrome layout scale and engine rasterisation scale are driven
    // from the same single UiScale instance
    for percentage in [100, 150, 200] {
        let scale = UiScale::new(percentage).expect("valid scale");
        let expected_factor = percentage as f64 / 100.0;

        assert_eq!(
            scale.factor(),
            expected_factor,
            "scale factor must match percentage ratio"
        );
        assert_eq!(
            scale.chrome_layout_scale(),
            expected_factor,
            "chrome layout scale must be driven directly from ui_scale"
        );
        assert_eq!(
            scale.engine_rasterization_scale(),
            expected_factor,
            "engine rasterisation scale must be driven directly from ui_scale"
        );

        // Crucial invariant: chrome layout scale and engine rasterisation scale
        // are identical. This guarantees content scales once, never twice or zero.
        assert_eq!(
            scale.chrome_layout_scale(),
            scale.engine_rasterization_scale(),
            "chrome layout scale and engine rasterisation scale must agree exactly"
        );

        // Demonstrate single scaling multiplier on dimensions:
        let logical_px = 120.0;
        let physical_px = scale.scale_logical(logical_px);
        assert_eq!(
            physical_px,
            logical_px * expected_factor,
            "dimension must scale by exactly the single scale factor"
        );
        assert_eq!(
            scale.unscale_physical(physical_px),
            logical_px,
            "unscale must roundtrip back to logical pixels"
        );
    }
}

#[test]
fn test_no_clipping_at_100_150_200_percent() {
    let unscaled_metrics = ChromeSurfaceMetrics::unscaled();
    let viewport_w = 1280.0;
    let viewport_h = 720.0;

    for percentage in [100, 150, 200] {
        let scale = UiScale::new(percentage).expect("valid scale");
        let scaled_metrics = unscaled_metrics.scaled_with(&scale);

        // Verify that chrome surfaces fit comfortably within viewport bounds with no clipping
        let clipping_result = scaled_metrics.check_clipping(viewport_w, viewport_h);
        assert!(
            clipping_result.is_ok(),
            "clipping detected at {percentage}%: {:?}",
            clipping_result.err()
        );

        // Elements remain non-clipped and correctly proportioned
        assert!(
            scaled_metrics.toolbar_height >= scaled_metrics.button_size,
            "toolbar height must accommodate button size without clipping"
        );
        assert!(
            scaled_metrics.toolbar_height >= scaled_metrics.omnibox_height,
            "toolbar height must accommodate omnibox without clipping"
        );
        assert!(
            scaled_metrics.tab_strip_height > 0.0,
            "tab strip height must be positive"
        );
    }
}

#[test]
fn test_every_chrome_surface_stays_usable_and_legible_at_200() {
    let scale_200 = UiScale::new(200).expect("valid 200 scale");
    let unscaled = ChromeSurfaceMetrics::unscaled();
    let metrics_200 = unscaled.scaled_with(&scale_200);

    // Usability: button hit targets
    assert_eq!(metrics_200.button_size, 56.0);
    assert!(
        metrics_200.button_size >= ChromeSurfaceMetrics::RECOMMENDED_TOUCH_TARGET_PX,
        "at 200%, button target size (56px) exceeds the recommended 44px touch target"
    );

    // Usability: omnibox height
    assert_eq!(metrics_200.omnibox_height, 64.0);
    assert!(
        metrics_200.omnibox_height >= ChromeSurfaceMetrics::RECOMMENDED_TOUCH_TARGET_PX,
        "at 200%, omnibox height (64px) exceeds the recommended 44px touch target"
    );

    // Legibility: typography font size
    assert_eq!(metrics_200.font_size, 28.0);
    assert!(
        metrics_200.font_size >= ChromeSurfaceMetrics::MIN_LEGIBLE_FONT_PX * 2.0,
        "at 200%, font size (28px) scales cleanly for high legibility"
    );

    assert!(
        metrics_200.is_usable_and_legible(),
        "chrome surface must be usable and legible at 200%"
    );
}

#[test]
fn test_ui_scale_bounds_enforcement() {
    // 100% to 200% are permitted
    assert!(UiScale::new(100).is_ok());
    assert!(UiScale::new(125).is_ok());
    assert!(UiScale::new(150).is_ok());
    assert!(UiScale::new(175).is_ok());
    assert!(UiScale::new(200).is_ok());

    // Out of bounds values are rejected
    assert_eq!(
        UiScale::new(99).unwrap_err(),
        ScalingError::InvalidUiScale(99)
    );
    assert_eq!(
        UiScale::new(50).unwrap_err(),
        ScalingError::InvalidUiScale(50)
    );
    assert_eq!(
        UiScale::new(201).unwrap_err(),
        ScalingError::InvalidUiScale(201)
    );
    assert_eq!(
        UiScale::new(300).unwrap_err(),
        ScalingError::InvalidUiScale(300)
    );
}

#[test]
fn test_page_zoom_bounds_and_stepping() {
    let mut zoom = PageZoom::default();
    assert_eq!(zoom.as_percentage(), 100);

    // Zoom in steps
    zoom.zoom_in();
    assert_eq!(zoom.as_percentage(), 110);
    zoom.zoom_in();
    assert_eq!(zoom.as_percentage(), 125);

    // Zoom out steps
    zoom.zoom_out();
    assert_eq!(zoom.as_percentage(), 110);
    zoom.zoom_out();
    assert_eq!(zoom.as_percentage(), 100);

    // Reset
    zoom.zoom_in();
    zoom.zoom_in();
    zoom.reset();
    assert_eq!(zoom.as_percentage(), 100);

    // Bounds enforcement (25% to 500%)
    assert!(PageZoom::new(25).is_ok());
    assert!(PageZoom::new(500).is_ok());
    assert_eq!(
        PageZoom::new(24).unwrap_err(),
        ScalingError::InvalidPageZoom(24)
    );
    assert_eq!(
        PageZoom::new(501).unwrap_err(),
        ScalingError::InvalidPageZoom(501)
    );
}

#[test]
fn test_find_in_page_navigation_and_lifecycle() {
    let mut find = FindInPageState::new();
    assert!(!find.is_visible());
    assert_eq!(find.query(), "");
    assert_eq!(find.match_count(), 0);
    assert_eq!(find.active_match_ordinal(), None);

    find.open();
    assert!(find.is_visible());

    find.set_query("freedom");
    assert_eq!(find.query(), "freedom");

    find.set_matches(3, Some(1));
    assert_eq!(find.match_count(), 3);
    assert_eq!(find.active_match_ordinal(), Some(1));

    // Next match cycles 1 -> 2 -> 3 -> 1
    find.next_match();
    assert_eq!(find.active_match_ordinal(), Some(2));
    find.next_match();
    assert_eq!(find.active_match_ordinal(), Some(3));
    find.next_match();
    assert_eq!(find.active_match_ordinal(), Some(1));

    // Previous match cycles 1 -> 3 -> 2 -> 1
    find.prev_match();
    assert_eq!(find.active_match_ordinal(), Some(3));
    find.prev_match();
    assert_eq!(find.active_match_ordinal(), Some(2));

    // Case sensitivity toggle
    assert!(!find.is_case_sensitive());
    find.set_case_sensitive(true);
    assert!(find.is_case_sensitive());

    // Close resets query and match count
    find.close();
    assert!(!find.is_visible());
    assert_eq!(find.query(), "");
    assert_eq!(find.match_count(), 0);
    assert_eq!(find.active_match_ordinal(), None);
}
