//! The member browsing profile.
//!
//! # Architecture and Invariants
//!
//! - **Language & Place Separation (FR-035, Principle VII)**:
//!   `language` is [`Language`] and `place` is [`Place`]. They are distinct
//!   values and are never serialized as a fused tag.
//! - **Theme & Interface Scaling (FR-005, FR-010)**:
//!   `theme_preference` defaults to system preference; `ui_scale` is an integer
//!   percentage between 100% and 200%.
//! - **Privacy, Egress & Telemetry (FR-023, FR-036a, FR-039, FR-039c)**:
//!   `profile_id` is an opaque local ID never derived from device characteristics.
//!   `diagnostics_enabled` and `crash_reporting_enabled` default to `false`.
//!   No file under the profile root directory holds an account credential, a
//!   token derived from one, or any value from which either can be reconstructed.

#![forbid(unsafe_code)]

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use evreos_i18n::{Language, Place, PlaceError};

use crate::store::StoreRegistry;

/// Theme presentation preference.
///
/// Default is [`ThemePreference::System`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemePreference {
    /// Follow the platform / system preference.
    #[default]
    System,
    /// Explicit light theme.
    Light,
    /// Explicit dark theme.
    Dark,
}

impl ThemePreference {
    /// Serialized representation as a lowercase string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    /// Parse theme preference from string.
    pub fn parse(s: &str) -> Result<Self, ProfileError> {
        match s.trim().to_ascii_lowercase().as_str() {
            "system" => Ok(Self::System),
            "light" => Ok(Self::Light),
            "dark" => Ok(Self::Dark),
            other => Err(ProfileError::InvalidFormat(format!(
                "invalid theme preference: {other}"
            ))),
        }
    }
}

pub use crate::search_provider::SearchProviderSetting;

/// Hand-off browser selection.
///
/// Configures which external browser application receives hand-offs.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum HandOffBrowser {
    /// The operating system's default browser.
    #[default]
    SystemDefault,
    /// A specific external application nominated by the user.
    Nominated {
        /// The executable or application name.
        program: String,
    },
}

impl HandOffBrowser {
    /// Nominate a specific external browser executable or command.
    ///
    /// Validates that the application does not nominate itself (Key Entities rule).
    pub fn nominated(program: impl Into<String>) -> Result<Self, ProfileError> {
        let prog = program.into();
        let trimmed = prog.trim();
        if trimmed.is_empty() {
            return Err(ProfileError::InvalidHandOffBrowser(
                "nominated browser program cannot be empty".to_string(),
            ));
        }

        let product = crate::brand::brand().product_name.to_lowercase();
        if (!product.is_empty() && trimmed.eq_ignore_ascii_case(&product))
            || trimmed.eq_ignore_ascii_case("self")
        {
            return Err(ProfileError::InvalidHandOffBrowser(
                "the shell cannot nominate itself as a hand-off browser".to_string(),
            ));
        }

        Ok(Self::Nominated {
            program: trimmed.to_string(),
        })
    }
}

/// Errors occurring during profile manipulation or serialization.
#[derive(Debug)]
pub enum ProfileError {
    /// Filesystem I/O failure.
    Io(io::Error),
    /// Invalid geographic place representation.
    Place(PlaceError),
    /// Unrecognised language primary subtag.
    InvalidLanguage(String),
    /// UI scale out of bounds (100% to 200%).
    InvalidUiScale(u32),
    /// Invalid hand-off browser nomination.
    InvalidHandOffBrowser(String),
    /// Configuration format or syntax error.
    InvalidFormat(String),
    /// Missing required profile field.
    MissingField(&'static str),
    /// Configuration file not found.
    NotFound(PathBuf),
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "profile I/O error: {err}"),
            Self::Place(err) => write!(f, "profile place error: {err}"),
            Self::InvalidLanguage(lang) => write!(f, "invalid profile language subtag: {lang}"),
            Self::InvalidUiScale(scale) => {
                write!(
                    f,
                    "invalid UI scale: {scale}% (must be between 100% and 200%)"
                )
            }
            Self::InvalidHandOffBrowser(msg) => write!(f, "invalid hand-off browser: {msg}"),
            Self::InvalidFormat(msg) => write!(f, "malformed profile configuration: {msg}"),
            Self::MissingField(field) => write!(f, "missing profile field: {field}"),
            Self::NotFound(path) => write!(f, "profile file not found at {}", path.display()),
        }
    }
}

impl std::error::Error for ProfileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Place(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for ProfileError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<PlaceError> for ProfileError {
    fn from(err: PlaceError) -> Self {
        Self::Place(err)
    }
}

/// The member browsing profile.
#[derive(Debug)]
pub struct Profile {
    /// Local opaque identifier (FR-036a).
    pub profile_id: String,
    /// Filesystem directory where this profile and its stores reside.
    pub root_path: PathBuf,
    /// Interface language (primary subtag only).
    pub language: Language,
    /// User geographic location preference (2 uppercase ASCII letters, un-fused).
    pub place: Place,
    /// Member theme preference.
    pub theme_preference: ThemePreference,
    /// Interface scaling percentage (100..=200).
    pub ui_scale: u32,
    /// Configured default search provider and endpoint.
    pub default_search_provider: SearchProviderSetting,
    /// External browser for hand-offs.
    pub hand_off_browser: HandOffBrowser,
    /// Diagnostics relay telemetry flag (default off).
    pub diagnostics_enabled: bool,
    /// Crash reporting flag (default off).
    pub crash_reporting_enabled: bool,
    /// Whether the home surface is hidden (default off).
    pub home_surface_hidden: bool,
    /// Stores registry for history, bookmarks, and downloads.
    stores: StoreRegistry,
}

impl Profile {
    /// Create a new default profile rooted at `root_path`.
    pub fn new(root_path: impl Into<PathBuf>) -> Self {
        let root = root_path.into();
        let stores = StoreRegistry::open(&root);
        Self {
            profile_id: generate_profile_id(),
            root_path: root,
            language: Language::En,
            place: Place::new("US").expect("valid default place"),
            theme_preference: ThemePreference::System,
            ui_scale: 100,
            default_search_provider: SearchProviderSetting::default(),
            hand_off_browser: HandOffBrowser::SystemDefault,
            diagnostics_enabled: false,
            crash_reporting_enabled: false,
            home_surface_hidden: false,
            stores,
        }
    }

    /// The profile's stores registry.
    pub fn stores(&self) -> &StoreRegistry {
        &self.stores
    }

    /// Mutable access to the profile's stores registry.
    pub fn stores_mut(&mut self) -> &mut StoreRegistry {
        &mut self.stores
    }

    /// Set UI scale, validating that it lies within 100%..=200%.
    pub fn set_ui_scale(&mut self, scale: u32) -> Result<(), ProfileError> {
        if (100..=200).contains(&scale) {
            self.ui_scale = scale;
            Ok(())
        } else {
            Err(ProfileError::InvalidUiScale(scale))
        }
    }

    /// Path to the primary profile configuration file.
    pub fn config_path(&self) -> PathBuf {
        self.root_path.join("profile.toml")
    }

    /// Save profile configuration atomically to disk under `root_path`.
    pub fn save(&self) -> Result<(), ProfileError> {
        if !self.root_path.exists() {
            fs::create_dir_all(&self.root_path)?;
        }

        if !(100..=200).contains(&self.ui_scale) {
            return Err(ProfileError::InvalidUiScale(self.ui_scale));
        }

        let serialized = self.serialize();
        let target_path = self.config_path();
        let tmp_path = self.root_path.join(".profile.toml.tmp");

        fs::write(&tmp_path, serialized.as_bytes())?;
        fs::rename(&tmp_path, &target_path)?;

        Ok(())
    }

    /// Open an existing profile configuration from disk under `root_path`.
    pub fn open(root_path: impl Into<PathBuf>) -> Result<Self, ProfileError> {
        let root = root_path.into();
        let config_file = root.join("profile.toml");
        if !config_file.is_file() {
            return Err(ProfileError::NotFound(config_file));
        }

        let content = fs::read_to_string(&config_file)?;
        let mut profile = Self::deserialize(&content, &root)?;
        profile.stores = StoreRegistry::open(&root);
        Ok(profile)
    }

    /// Open an existing profile, or create and save a new default profile if none exists.
    pub fn open_or_create(root_path: impl Into<PathBuf>) -> Result<Self, ProfileError> {
        let root = root_path.into();
        match Self::open(&root) {
            Ok(profile) => Ok(profile),
            Err(ProfileError::NotFound(_)) => {
                let profile = Self::new(root);
                profile.save()?;
                Ok(profile)
            }
            Err(err) => Err(err),
        }
    }

    /// Close the profile, flushing state to disk.
    pub fn close(self) -> Result<(), ProfileError> {
        self.save()
    }

    fn serialize(&self) -> String {
        let mut out = String::new();
        out.push_str("# Profile configuration\n");
        out.push_str(&format!("profile_id = \"{}\"\n", self.profile_id));
        out.push_str(&format!("language = \"{}\"\n", self.language.subtag()));
        out.push_str(&format!("place = \"{}\"\n", self.place.code()));
        out.push_str(&format!(
            "theme_preference = \"{}\"\n",
            self.theme_preference.as_str()
        ));
        out.push_str(&format!("ui_scale = {}\n", self.ui_scale));
        out.push_str(&format!(
            "default_search_provider = \"{}\"\n",
            escape_string(&self.default_search_provider.provider)
        ));
        out.push_str(&format!(
            "search_endpoint = \"{}\"\n",
            escape_string(&self.default_search_provider.endpoint)
        ));
        match &self.hand_off_browser {
            HandOffBrowser::SystemDefault => {
                out.push_str("hand_off_browser = \"system_default\"\n");
            }
            HandOffBrowser::Nominated { program } => {
                out.push_str(&format!(
                    "hand_off_browser = \"nominated:{}\"\n",
                    escape_string(program)
                ));
            }
        }
        out.push_str(&format!(
            "diagnostics_enabled = {}\n",
            self.diagnostics_enabled
        ));
        out.push_str(&format!(
            "crash_reporting_enabled = {}\n",
            self.crash_reporting_enabled
        ));
        out.push_str(&format!(
            "home_surface_hidden = {}\n",
            self.home_surface_hidden
        ));
        out
    }

    fn deserialize(content: &str, root: &Path) -> Result<Self, ProfileError> {
        let mut profile_id = None;
        let mut language = None;
        let mut place = None;
        let mut theme_preference = None;
        let mut ui_scale = None;
        let mut search_provider = None;
        let mut search_endpoint = None;
        let mut hand_off_browser = None;
        let mut diagnostics_enabled = None;
        let mut crash_reporting_enabled = None;
        let mut home_surface_hidden = None;

        for (line_idx, raw_line) in content.lines().enumerate() {
            let line_num = line_idx + 1;
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let Some((k, v)) = line.split_once('=') else {
                return Err(ProfileError::InvalidFormat(format!(
                    "line {line_num}: missing '=' separator"
                )));
            };

            let key = k.trim();
            let val = v.trim();

            match key {
                "profile_id" => {
                    profile_id = Some(unquote(val, line_num)?);
                }
                "language" => {
                    let subtag = unquote(val, line_num)?;
                    let parsed_lang = match subtag.as_str() {
                        "de" => Language::De,
                        "el" => Language::El,
                        "en" => Language::En,
                        other => return Err(ProfileError::InvalidLanguage(other.to_string())),
                    };
                    language = Some(parsed_lang);
                }
                "place" => {
                    let code = unquote(val, line_num)?;
                    let parsed_place = Place::new(&code)?;
                    place = Some(parsed_place);
                }
                "theme_preference" => {
                    let s = unquote(val, line_num)?;
                    theme_preference = Some(ThemePreference::parse(&s)?);
                }
                "ui_scale" => {
                    let scale: u32 = val.parse().map_err(|_| {
                        ProfileError::InvalidFormat(format!(
                            "line {line_num}: invalid integer for ui_scale"
                        ))
                    })?;
                    if !(100..=200).contains(&scale) {
                        return Err(ProfileError::InvalidUiScale(scale));
                    }
                    ui_scale = Some(scale);
                }
                "default_search_provider" => {
                    search_provider = Some(unquote(val, line_num)?);
                }
                "search_endpoint" => {
                    search_endpoint = Some(unquote(val, line_num)?);
                }
                "hand_off_browser" => {
                    let raw = unquote(val, line_num)?;
                    if raw == "system_default" {
                        hand_off_browser = Some(HandOffBrowser::SystemDefault);
                    } else if let Some(prog) = raw.strip_prefix("nominated:") {
                        hand_off_browser = Some(HandOffBrowser::nominated(prog)?);
                    } else {
                        return Err(ProfileError::InvalidFormat(format!(
                            "line {line_num}: invalid hand_off_browser value: {raw}"
                        )));
                    }
                }
                "diagnostics_enabled" => {
                    let b: bool = val.parse().map_err(|_| {
                        ProfileError::InvalidFormat(format!(
                            "line {line_num}: invalid boolean for diagnostics_enabled"
                        ))
                    })?;
                    diagnostics_enabled = Some(b);
                }
                "crash_reporting_enabled" => {
                    let b: bool = val.parse().map_err(|_| {
                        ProfileError::InvalidFormat(format!(
                            "line {line_num}: invalid boolean for crash_reporting_enabled"
                        ))
                    })?;
                    crash_reporting_enabled = Some(b);
                }
                "home_surface_hidden" => {
                    let b: bool = val.parse().map_err(|_| {
                        ProfileError::InvalidFormat(format!(
                            "line {line_num}: invalid boolean for home_surface_hidden"
                        ))
                    })?;
                    home_surface_hidden = Some(b);
                }
                _ => {
                    // Unknown keys ignored for forward compatibility
                }
            }
        }

        let pid = profile_id.ok_or(ProfileError::MissingField("profile_id"))?;
        let lang = language.ok_or(ProfileError::MissingField("language"))?;
        let plc = place.ok_or(ProfileError::MissingField("place"))?;
        let theme = theme_preference.unwrap_or(ThemePreference::System);
        let scale = ui_scale.unwrap_or(100);
        let provider = search_provider.unwrap_or_else(|| "DuckDuckGo".to_string());
        let endpoint =
            search_endpoint.unwrap_or_else(|| crate::brand::brand().search_endpoint.clone());
        let handoff = hand_off_browser.unwrap_or(HandOffBrowser::SystemDefault);
        let diag = diagnostics_enabled.unwrap_or(false);
        let crash = crash_reporting_enabled.unwrap_or(false);
        let home_hidden = home_surface_hidden.unwrap_or(false);

        Ok(Self {
            profile_id: pid,
            root_path: root.to_path_buf(),
            language: lang,
            place: plc,
            theme_preference: theme,
            ui_scale: scale,
            default_search_provider: SearchProviderSetting { provider, endpoint },
            hand_off_browser: handoff,
            diagnostics_enabled: diag,
            crash_reporting_enabled: crash,
            home_surface_hidden: home_hidden,
            stores: StoreRegistry::open(root),
        })
    }
}

fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn unquote(s: &str, line_num: usize) -> Result<String, ProfileError> {
    let s = s.trim();
    if !s.starts_with('"') || !s.ends_with('"') || s.len() < 2 {
        return Err(ProfileError::InvalidFormat(format!(
            "line {line_num}: expected quoted string literal"
        )));
    }
    let inner = &s[1..s.len() - 1];
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    Ok(out)
}

fn generate_profile_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("prof_{now:016x}{count:08x}")
}
