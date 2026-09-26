//! Interface scaling, page zoom, and find-in-page state.
//!
//! # Architecture and Invariants
//!
//! - **Single Owner of Interface Scale (FR-005, research.md §5.6)**:
//!   `scaling.rs` is the workspace's ONE owner of the interface scale.
//!   A single member-facing [`UiScale`] value from 100% to 200% drives both
//!   the chrome layout scale and — through each platform backend's view module,
//!   which holds no scale value of its own — the engine's rasterisation scale
//!   from that same value.
//! - **No Double Scaling or Under-Scaling**:
//!   Setting the shell's chrome layout scale and the engine's rasterisation scale
//!   together ensures content scales once rather than twice or not at all.
//! - **Three Distinct, Non-Conflated Values**:
//!   1. `ui_scale`: Single global member-facing interface scaling value (100% to 200%).
//!   2. `page_zoom`: Per-site content zoom percentage held in [`PerSiteZoomStore`].
//!   3. `find_state`: Per-tab search state held in [`FindInPageState`].
//!
//!   An interface-scale change leaves page zoom and find state unchanged, while a
//!   page-zoom change alters page content zoom alone.
//! - **No Platform Leakage**:
//!   No other crate holds an interface-scale value, reads a system DPI value,
//!   or configures monitor-scale-change behaviour. No platform calls land here.
//! - **Usability & Legibility at 200% (SC-008)**:
//!   At 100%, 150%, and 200%, nothing is clipped, touch/pointer targets remain
//!   accessible, and content reflows legibly.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fmt;

use crate::site_key::SiteKey;

/// Standard zoom percentage increments used by modern browsers.
pub const STANDARD_ZOOM_LEVELS: &[u32] = &[
    25, 33, 50, 67, 75, 80, 90, 100, 110, 125, 150, 175, 200, 250, 300, 400, 500,
];

/// Errors occurring during interface scaling or zoom calculation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScalingError {
    /// Interface scale percentage was out of the permitted 100% to 200% range.
    InvalidUiScale(u32),
    /// Page zoom percentage was out of the permitted 25% to 500% range.
    InvalidPageZoom(u32),
}

impl fmt::Display for ScalingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUiScale(val) => {
                write!(f, "UI scale {val}% is out of bounds (must be 100% to 200%)")
            }
            Self::InvalidPageZoom(val) => {
                write!(f, "page zoom {val}% is out of bounds (must be 25% to 500%)")
            }
        }
    }
}

impl std::error::Error for ScalingError {}

/// Errors detected during chrome layout bounds and clipping verification.
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutClippingError {
    /// Chrome surface exceeded available viewport dimensions.
    ViewportOverflow {
        dimension: &'static str,
        required: f64,
        available: f64,
    },
    /// Interactive control is clipped below its minimum required target size.
    TargetClipped {
        element: &'static str,
        actual_size: f64,
        minimum_required: f64,
    },
}

impl fmt::Display for LayoutClippingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ViewportOverflow {
                dimension,
                required,
                available,
            } => {
                write!(
                    f,
                    "chrome layout overflow in {dimension}: required {required}px, available {available}px"
                )
            }
            Self::TargetClipped {
                element,
                actual_size,
                minimum_required,
            } => {
                write!(
                    f,
                    "element '{element}' clipped to {actual_size}px (minimum accessible size is {minimum_required}px)"
                )
            }
        }
    }
}

impl std::error::Error for LayoutClippingError {}

/// Member-facing interface scale percentage (100% to 200%).
///
/// This is the workspace's single owner and source of truth for the interface scale.
/// It drives both the chrome layout scale and the embedded engine's rasterisation scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UiScale(u32);

impl Default for UiScale {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl UiScale {
    /// Minimum allowed interface scale percentage (100%).
    pub const MIN_PERCENTAGE: u32 = 100;
    /// Maximum allowed interface scale percentage (200% per FR-005).
    pub const MAX_PERCENTAGE: u32 = 200;
    /// Default interface scale percentage (100%).
    pub const DEFAULT: Self = Self(100);

    /// Construct a new `UiScale`, validating that `100 <= percentage <= 200`.
    pub fn new(percentage: u32) -> Result<Self, ScalingError> {
        if (Self::MIN_PERCENTAGE..=Self::MAX_PERCENTAGE).contains(&percentage) {
            Ok(Self(percentage))
        } else {
            Err(ScalingError::InvalidUiScale(percentage))
        }
    }

    /// Integer percentage (e.g. 100, 150, 200).
    #[inline]
    pub const fn as_percentage(&self) -> u32 {
        self.0
    }

    /// Scaling factor as a multiplier (e.g. 1.0, 1.5, 2.0).
    #[inline]
    pub fn factor(&self) -> f64 {
        self.0 as f64 / 100.0
    }

    /// Chrome layout scale factor.
    ///
    /// Driven directly from this single value.
    #[inline]
    pub fn chrome_layout_scale(&self) -> f64 {
        self.factor()
    }

    /// Engine rasterisation scale factor.
    ///
    /// Driven directly from this same single value, ensuring the chrome and engine
    /// view agree exactly and content scales once rather than twice or not at all.
    #[inline]
    pub fn engine_rasterization_scale(&self) -> f64 {
        self.factor()
    }

    /// Scale a logical pixel dimension to device physical pixels.
    #[inline]
    pub fn scale_logical(&self, logical_px: f64) -> f64 {
        logical_px * self.factor()
    }

    /// Unscale a physical device pixel dimension back to logical pixels.
    #[inline]
    pub fn unscale_physical(&self, physical_px: f64) -> f64 {
        physical_px / self.factor()
    }
}

impl fmt::Display for UiScale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}%", self.0)
    }
}

/// Per-site page zoom level (25% to 500%).
///
/// Under FR-005, page zoom is distinct from `UiScale` and alters page content
/// magnification alone, without modifying the chrome layout scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PageZoom(u32);

impl Default for PageZoom {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl PageZoom {
    /// Minimum allowed page zoom percentage (25%).
    pub const MIN_PERCENTAGE: u32 = 25;
    /// Maximum allowed page zoom percentage (500%).
    pub const MAX_PERCENTAGE: u32 = 500;
    /// Default page zoom percentage (100%).
    pub const DEFAULT: Self = Self(100);

    /// Construct a new `PageZoom`, validating that `25 <= percentage <= 500`.
    pub fn new(percentage: u32) -> Result<Self, ScalingError> {
        if (Self::MIN_PERCENTAGE..=Self::MAX_PERCENTAGE).contains(&percentage) {
            Ok(Self(percentage))
        } else {
            Err(ScalingError::InvalidPageZoom(percentage))
        }
    }

    /// Integer percentage (e.g. 100, 125, 200).
    #[inline]
    pub const fn as_percentage(&self) -> u32 {
        self.0
    }

    /// Zoom multiplier factor (e.g. 1.0, 1.25, 2.0).
    #[inline]
    pub fn factor(&self) -> f64 {
        self.0 as f64 / 100.0
    }

    /// Increase zoom to the next standard step.
    pub fn zoom_in(&mut self) {
        for &step in STANDARD_ZOOM_LEVELS {
            if step > self.0 {
                self.0 = step;
                return;
            }
        }
        self.0 = Self::MAX_PERCENTAGE;
    }

    /// Decrease zoom to the previous standard step.
    pub fn zoom_out(&mut self) {
        for &step in STANDARD_ZOOM_LEVELS.iter().rev() {
            if step < self.0 {
                self.0 = step;
                return;
            }
        }
        self.0 = Self::MIN_PERCENTAGE;
    }

    /// Reset zoom back to the 100% default.
    pub fn reset(&mut self) {
        self.0 = 100;
    }
}

impl fmt::Display for PageZoom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}%", self.0)
    }
}

/// In-memory store holding per-site page zoom preferences.
///
/// Keyed by canonical [`SiteKey`] (FR-005, Decision 0007).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PerSiteZoomStore {
    zooms: HashMap<SiteKey, PageZoom>,
}

impl PerSiteZoomStore {
    /// Create a new, empty zoom store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the effective page zoom for `site`, falling back to 100% if unset.
    pub fn get_zoom(&self, site: &SiteKey) -> PageZoom {
        self.zooms.get(site).copied().unwrap_or(PageZoom::DEFAULT)
    }

    /// Set a custom page zoom for `site`.
    pub fn set_zoom(&mut self, site: SiteKey, zoom: PageZoom) {
        if zoom == PageZoom::DEFAULT {
            self.zooms.remove(&site);
        } else {
            self.zooms.insert(site, zoom);
        }
    }

    /// Reset page zoom for `site` back to default (100%).
    pub fn reset_zoom(&mut self, site: &SiteKey) -> Option<PageZoom> {
        self.zooms.remove(site)
    }

    /// Number of explicitly remembered per-site zoom preferences.
    pub fn len(&self) -> usize {
        self.zooms.len()
    }

    /// Whether any site has a custom zoom recorded.
    pub fn is_empty(&self) -> bool {
        self.zooms.is_empty()
    }

    /// Clear all per-site zoom preferences.
    pub fn clear(&mut self) {
        self.zooms.clear();
    }
}

/// Find-in-page state held per tab (FR-005).
///
/// Completely independent from interface scale and page zoom.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FindInPageState {
    /// The search query text.
    query: String,
    /// Total count of matching occurrences found in the tab document.
    match_count: usize,
    /// 1-based index of the currently highlighted match (or 0 if none).
    active_match: usize,
    /// Whether find-in-page case sensitivity is enabled.
    case_sensitive: bool,
    /// Whether the find-in-page chrome bar is currently active/visible.
    is_visible: bool,
}

impl FindInPageState {
    /// Create a new, inactive find-in-page state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the find-in-page search bar is visible.
    pub fn is_visible(&self) -> bool {
        self.is_visible
    }

    /// Show the find-in-page interface.
    pub fn open(&mut self) {
        self.is_visible = true;
    }

    /// Dismiss and hide the find-in-page interface.
    pub fn close(&mut self) {
        self.is_visible = false;
        self.clear();
    }

    /// The current search query string.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Update search query, resetting match counts.
    pub fn set_query(&mut self, query: impl Into<String>) {
        let q = query.into();
        if q != self.query {
            self.query = q;
            self.match_count = 0;
            self.active_match = 0;
        }
    }

    /// Total count of matches found.
    pub fn match_count(&self) -> usize {
        self.match_count
    }

    /// 1-based ordinal of the active match, or `None` if zero matches.
    pub fn active_match_ordinal(&self) -> Option<usize> {
        if self.active_match > 0 && self.active_match <= self.match_count {
            Some(self.active_match)
        } else {
            None
        }
    }

    /// Update matches discovered by the rendering engine.
    pub fn set_matches(&mut self, count: usize, active_ordinal: Option<usize>) {
        self.match_count = count;
        if count == 0 {
            self.active_match = 0;
        } else {
            self.active_match = active_ordinal.unwrap_or(1).clamp(1, count);
        }
    }

    /// Step to the next match occurrence.
    pub fn next_match(&mut self) {
        if self.match_count > 0 {
            self.active_match = if self.active_match >= self.match_count {
                1
            } else {
                self.active_match + 1
            };
        }
    }

    /// Step to the previous match occurrence.
    pub fn prev_match(&mut self) {
        if self.match_count > 0 {
            self.active_match = if self.active_match <= 1 {
                self.match_count
            } else {
                self.active_match - 1
            };
        }
    }

    /// Whether case sensitivity is enabled.
    pub fn is_case_sensitive(&self) -> bool {
        self.case_sensitive
    }

    /// Set case sensitivity flag.
    pub fn set_case_sensitive(&mut self, sensitive: bool) {
        self.case_sensitive = sensitive;
    }

    /// Reset query and match counts.
    pub fn clear(&mut self) {
        self.query.clear();
        self.match_count = 0;
        self.active_match = 0;
    }
}

/// Baseline layout dimensions and touch target metrics for browser chrome surfaces.
///
/// Used to verify that at 100%, 150%, and 200% scaling:
/// - Controls never clip or overflow available viewport bounds.
/// - Minimum touch/pointer target sizes are maintained (WCAG 2.5.5 / 2.5.8).
/// - Fonts remain legible without truncated labels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChromeSurfaceMetrics {
    /// Navigation toolbar height in pixels.
    pub toolbar_height: f64,
    /// Omnibox address bar height in pixels.
    pub omnibox_height: f64,
    /// Tab strip height in pixels.
    pub tab_strip_height: f64,
    /// Tab item width in pixels.
    pub tab_width: f64,
    /// Interactive button size (width and height) in pixels.
    pub button_size: f64,
    /// Base text font size in pixels.
    pub font_size: f64,
}

impl ChromeSurfaceMetrics {
    /// Baseline unscaled metrics at 100% interface scale.
    pub const fn unscaled() -> Self {
        Self {
            toolbar_height: 40.0,
            omnibox_height: 32.0,
            tab_strip_height: 36.0,
            tab_width: 180.0,
            button_size: 28.0,
            font_size: 14.0,
        }
    }

    /// Project baseline metrics using the specified [`UiScale`].
    pub fn scaled_with(&self, scale: &UiScale) -> Self {
        let f = scale.factor();
        Self {
            toolbar_height: self.toolbar_height * f,
            omnibox_height: self.omnibox_height * f,
            tab_strip_height: self.tab_strip_height * f,
            tab_width: self.tab_width * f,
            button_size: self.button_size * f,
            font_size: self.font_size * f,
        }
    }

    /// Minimum accessible hit target for pointer operation (WCAG 2.5.8 minimum is 24px).
    pub const MIN_ACCESSIBLE_TARGET_PX: f64 = 24.0;
    /// Recommended comfortable touch target (WCAG 2.5.5 recommended is 44px).
    pub const RECOMMENDED_TOUCH_TARGET_PX: f64 = 44.0;
    /// Minimum legible font size in pixels.
    pub const MIN_LEGIBLE_FONT_PX: f64 = 12.0;

    /// Verify that this surface remains fully usable and legible.
    ///
    /// At 200%, button targets expand well beyond recommended sizes (e.g. 56px >= 44px)
    /// and font sizes scale cleanly (28px >= 12px).
    pub fn is_usable_and_legible(&self) -> bool {
        self.button_size >= Self::MIN_ACCESSIBLE_TARGET_PX
            && self.omnibox_height >= Self::MIN_ACCESSIBLE_TARGET_PX
            && self.font_size >= Self::MIN_LEGIBLE_FONT_PX
    }

    /// Check that chrome layout elements fit within the given viewport without clipping.
    pub fn check_clipping(
        &self,
        viewport_width: f64,
        viewport_height: f64,
    ) -> Result<(), LayoutClippingError> {
        let total_chrome_height = self.toolbar_height + self.tab_strip_height;
        if total_chrome_height >= viewport_height {
            return Err(LayoutClippingError::ViewportOverflow {
                dimension: "height",
                required: total_chrome_height,
                available: viewport_height,
            });
        }

        if self.button_size > self.toolbar_height {
            return Err(LayoutClippingError::TargetClipped {
                element: "button",
                actual_size: self.toolbar_height,
                minimum_required: self.button_size,
            });
        }

        if self.omnibox_height > self.toolbar_height {
            return Err(LayoutClippingError::TargetClipped {
                element: "omnibox",
                actual_size: self.toolbar_height,
                minimum_required: self.omnibox_height,
            });
        }

        if viewport_width < self.button_size * 4.0 {
            return Err(LayoutClippingError::ViewportOverflow {
                dimension: "width",
                required: self.button_size * 4.0,
                available: viewport_width,
            });
        }

        Ok(())
    }
}
