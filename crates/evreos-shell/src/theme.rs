//! Theme management and presentation resolution for the Evreos shell (FR-010).
//!
//! # Architecture and Invariants
//!
//! - **Light and Dark Presentation (FR-010)**: Evreos supports both [`ThemePresentation::Light`]
//!   and [`ThemePresentation::Dark`].
//! - **System Preference by Default**: A fresh profile follows the operating system's
//!   theme preference ([`ThemePreference::System`]).
//! - **Dynamic System Updates**: While the member's override is unset ([`ThemePreference::System`]),
//!   changes in the system preference immediately update the effective theme presentation.
//! - **Member Override & Persistence**: The member may explicitly override the theme
//!   to [`ThemePreference::Light`] or [`ThemePreference::Dark`]. This override is persisted
//!   in [`Profile`] and survives application restart.
//! - **Window Event Integration**: Changes in the system color scheme delivered via
//!   [`winit::event::WindowEvent::ThemeChanged`] update system preference dynamically.

#![forbid(unsafe_code)]

use std::fmt;

use crate::profile::{Profile, ProfileError, ThemePreference};
use winit::event::WindowEvent;
use winit::window::Theme as WinitTheme;

/// The active visual theme presentation rendered by the browser shell and chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ThemePresentation {
    /// Light appearance with light backgrounds and dark foreground text.
    #[default]
    Light,
    /// Dark appearance with dark backgrounds and light foreground text.
    Dark,
}

impl ThemePresentation {
    /// Whether this presentation is dark.
    pub const fn is_dark(&self) -> bool {
        matches!(self, Self::Dark)
    }

    /// Whether this presentation is light.
    pub const fn is_light(&self) -> bool {
        matches!(self, Self::Light)
    }

    /// String identifier for this theme presentation ("light" or "dark").
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

impl fmt::Display for ThemePresentation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<WinitTheme> for ThemePresentation {
    fn from(theme: WinitTheme) -> Self {
        match theme {
            WinitTheme::Light => Self::Light,
            WinitTheme::Dark => Self::Dark,
        }
    }
}

impl From<ThemePresentation> for WinitTheme {
    fn from(presentation: ThemePresentation) -> Self {
        match presentation {
            ThemePresentation::Light => WinitTheme::Light,
            ThemePresentation::Dark => WinitTheme::Dark,
        }
    }
}

/// Trait providing the current system theme preference.
pub trait SystemThemeSource {
    /// Return the current theme preferred by the operating system.
    fn current_system_theme(&self) -> ThemePresentation;
}

/// A controllable system theme source for unit and integration tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MockSystemThemeSource {
    current: ThemePresentation,
}

impl MockSystemThemeSource {
    /// Create a new mock source with the specified initial theme.
    pub const fn new(initial: ThemePresentation) -> Self {
        Self { current: initial }
    }

    /// Update the system theme preference simulated by this source.
    pub fn set_theme(&mut self, theme: ThemePresentation) {
        self.current = theme;
    }
}

impl SystemThemeSource for MockSystemThemeSource {
    fn current_system_theme(&self) -> ThemePresentation {
        self.current
    }
}

/// Default platform theme source falling back to light presentation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlatformSystemThemeSource;

impl SystemThemeSource for PlatformSystemThemeSource {
    fn current_system_theme(&self) -> ThemePresentation {
        ThemePresentation::Light
    }
}

/// Coordinates and resolves effective theme presentation from system preference and member override.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeCoordinator {
    system_preference: ThemePresentation,
    override_preference: ThemePreference,
}

impl ThemeCoordinator {
    /// Create a new theme coordinator with the given initial system preference and default override (System).
    pub fn new(system_preference: ThemePresentation) -> Self {
        Self {
            system_preference,
            override_preference: ThemePreference::System,
        }
    }

    /// Create a new theme coordinator from a [`Profile`] and an initial system preference.
    pub fn from_profile(profile: &Profile, system_preference: ThemePresentation) -> Self {
        Self {
            system_preference,
            override_preference: profile.theme_preference,
        }
    }

    /// The active/effective theme presentation.
    ///
    /// Follows the system preference by default ([`ThemePreference::System`]), or
    /// applies the member's explicit override ([`ThemePreference::Light`] or [`ThemePreference::Dark`]).
    pub fn presentation(&self) -> ThemePresentation {
        match self.override_preference {
            ThemePreference::System => self.system_preference,
            ThemePreference::Light => ThemePresentation::Light,
            ThemePreference::Dark => ThemePresentation::Dark,
        }
    }

    /// Alias for [`presentation`](Self::presentation).
    pub fn effective_theme(&self) -> ThemePresentation {
        self.presentation()
    }

    /// The current system theme preference.
    pub fn system_preference(&self) -> ThemePresentation {
        self.system_preference
    }

    /// The member's current theme override preference.
    pub fn override_preference(&self) -> ThemePreference {
        self.override_preference
    }

    /// Whether an explicit member override is set (i.e. not [`ThemePreference::System`]).
    pub fn has_override(&self) -> bool {
        self.override_preference != ThemePreference::System
    }

    /// Update the system preference.
    ///
    /// If no explicit member override is set, this immediately updates the effective
    /// theme presentation. Returns `true` if the effective presentation changed.
    pub fn update_system_preference(&mut self, system: ThemePresentation) -> bool {
        let before = self.presentation();
        self.system_preference = system;
        let after = self.presentation();
        before != after
    }

    /// Update the system preference from a [`SystemThemeSource`].
    ///
    /// Returns `true` if the effective presentation changed.
    pub fn poll_system_source<S: SystemThemeSource>(&mut self, source: &S) -> bool {
        self.update_system_preference(source.current_system_theme())
    }

    /// Handle a [`WindowEvent`], updating system preference if [`WindowEvent::ThemeChanged`] is received.
    ///
    /// Returns `true` if the effective presentation changed as a result of the event.
    pub fn handle_window_event(&mut self, event: &WindowEvent) -> bool {
        if let WindowEvent::ThemeChanged(winit_theme) = event {
            self.update_system_preference(ThemePresentation::from(*winit_theme))
        } else {
            false
        }
    }

    /// Set an explicit member override in memory without writing to disk.
    ///
    /// Returns `true` if the effective presentation changed.
    pub fn set_override(&mut self, pref: ThemePreference) -> bool {
        let before = self.presentation();
        self.override_preference = pref;
        let after = self.presentation();
        before != after
    }

    /// Clear any member override, reverting to following the system preference.
    ///
    /// Returns `true` if the effective presentation changed.
    pub fn clear_override(&mut self) -> bool {
        self.set_override(ThemePreference::System)
    }

    /// Set the member override and persist it immediately to the given [`Profile`].
    ///
    /// Writes the change atomically to disk.
    /// Returns `Ok(true)` if the effective presentation changed.
    pub fn set_override_and_persist(
        &mut self,
        pref: ThemePreference,
        profile: &mut Profile,
    ) -> Result<bool, ProfileError> {
        let changed = self.set_override(pref);
        profile.theme_preference = pref;
        profile.save()?;
        Ok(changed)
    }

    /// Clear the member override and persist the reversion to [`ThemePreference::System`] in [`Profile`].
    ///
    /// Writes the change atomically to disk.
    /// Returns `Ok(true)` if the effective presentation changed.
    pub fn clear_override_and_persist(
        &mut self,
        profile: &mut Profile,
    ) -> Result<bool, ProfileError> {
        self.set_override_and_persist(ThemePreference::System, profile)
    }
}

/// Type alias for [`ThemeCoordinator`] providing alternative terminology.
pub type ThemeManager = ThemeCoordinator;
