//! Site permission store and prompt flow.
//!
//! # Architecture and Invariants
//!
//! - **Per-Site Keying (FR-006, Decision 0007)**:
//!   Permissions are keyed by [`SiteKey`], ensuring consistent policies across
//!   subdomains (e.g. `login.bank.invalid` and `bank.invalid` share permissions).
//! - **Default to Ask (FR-006, data-model §1.10)**:
//!   Every `(SiteKey, Capability)` pair defaults to [`PermissionDecision::Ask`].
//!   Permissions are never pre-granted.
//! - **Revisitable Decisions (FR-006)**:
//!   All decisions are member-editable and revocable at any time without reinstalling.
//! - **Private Window Isolation (FR-007, FR-007a)**:
//!   Decisions made in a private browsing window are scoped to that window and
//!   die upon window closure, leaving zero trace on disk.
//! - **Platform Capability Honesty (FR-037, data-model §1.10)**:
//!   [`PermissionDecision::UnavailableOnThisPlatform`] is distinct from [`PermissionDecision::Denied`].
//!   When a capability cannot be delivered by the engine/platform, Evreos routes
//!   to the FR-037 hand-off offer rather than presenting a prompt or a false denial.

#![forbid(unsafe_code)]

use std::fmt;
use std::io;
use std::str::FromStr;

use crate::app::AppWindowId;
use crate::site_key::{SiteKey, SiteKeyError};

/// Closed set of capabilities requiring per-site member consent under FR-006.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Capability {
    /// Video capture hardware access.
    Camera,
    /// Audio capture hardware access.
    Microphone,
    /// Geographic position access.
    Location,
    /// System desktop notifications.
    Notification,
}

impl Capability {
    /// All four capabilities governed by FR-006.
    pub const ALL: [Capability; 4] = [
        Capability::Camera,
        Capability::Microphone,
        Capability::Location,
        Capability::Notification,
    ];

    /// Lowercase string identifier for serialization and catalogues.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Camera => "camera",
            Self::Microphone => "microphone",
            Self::Location => "location",
            Self::Notification => "notification",
        }
    }

    /// Parse capability from string.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "camera" => Some(Self::Camera),
            "microphone" => Some(Self::Microphone),
            "location" => Some(Self::Location),
            "notification" => Some(Self::Notification),
            _ => None,
        }
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Capability {
    type Err = PermissionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
            .ok_or_else(|| PermissionError::InvalidFormat(format!("unknown capability: {s}")))
    }
}

/// The decision state for a `(SiteKey, Capability)` pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PermissionDecision {
    /// Prompt the member for consent (default).
    #[default]
    Ask,
    /// Access explicitly granted by the member.
    Granted,
    /// Access explicitly denied by the member.
    Denied,
    /// The engine or platform cannot deliver this capability (FR-037).
    ///
    /// Distinct from `Denied`: not a member rejection, and triggers hand-off.
    UnavailableOnThisPlatform,
}

impl PermissionDecision {
    /// Lowercase string identifier for serialization.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::Granted => "granted",
            Self::Denied => "denied",
            Self::UnavailableOnThisPlatform => "unavailable_on_this_platform",
        }
    }

    /// Parse permission decision from string.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "ask" => Some(Self::Ask),
            "granted" => Some(Self::Granted),
            "denied" => Some(Self::Denied),
            "unavailable_on_this_platform" | "unavailable" => Some(Self::UnavailableOnThisPlatform),
            _ => None,
        }
    }

    /// Returns true if this decision permits access.
    pub const fn is_granted(&self) -> bool {
        matches!(self, Self::Granted)
    }

    /// Returns true if this decision denies access.
    pub const fn is_denied(&self) -> bool {
        matches!(self, Self::Denied)
    }

    /// Returns true if this capability is unavailable on this platform.
    pub const fn is_unavailable(&self) -> bool {
        matches!(self, Self::UnavailableOnThisPlatform)
    }
}

impl fmt::Display for PermissionDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for PermissionDecision {
    type Err = PermissionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| {
            PermissionError::InvalidFormat(format!("unknown permission decision: {s}"))
        })
    }
}

/// Scope of a permission decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowScope {
    /// Normal window; persists to the profile directory across restarts.
    Persistent,
    /// Private window; transient and purged on window close.
    PrivateWindow(AppWindowId),
}

impl WindowScope {
    /// Whether this scope represents a private window.
    pub const fn is_private(&self) -> bool {
        matches!(self, Self::PrivateWindow(_))
    }
}

impl fmt::Display for WindowScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Persistent => f.write_str("persistent"),
            Self::PrivateWindow(id) => write!(f, "private({id})"),
        }
    }
}

/// A permission decision recorded for a specific site and capability under FR-006.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SitePermission {
    /// The site this permission applies to (canonical registrable domain or host).
    pub site: SiteKey,
    /// The capability requested.
    pub capability: Capability,
    /// The member's decision or platform state.
    pub decision: PermissionDecision,
    /// The timestamp when the decision was recorded, if any.
    pub decided_at: Option<std::time::SystemTime>,
    /// The window scope (persistent vs private).
    pub window_scope: WindowScope,
}

impl SitePermission {
    /// Construct a new site permission record.
    pub fn new(
        site: SiteKey,
        capability: Capability,
        decision: PermissionDecision,
        window_scope: WindowScope,
    ) -> Self {
        Self {
            site,
            capability,
            decision,
            decided_at: Some(std::time::SystemTime::now()),
            window_scope,
        }
    }
}

/// Error type for site permission operations.
#[derive(Debug)]
pub enum PermissionError {
    /// File system I/O error.
    Io(io::Error),
    /// Invalid format in serialized store.
    InvalidFormat(String),
    /// Site key validation error.
    Site(SiteKeyError),
}

impl fmt::Display for PermissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "permission store I/O error: {err}"),
            Self::InvalidFormat(msg) => write!(f, "invalid permission format: {msg}"),
            Self::Site(err) => write!(f, "site key error: {err}"),
        }
    }
}

impl std::error::Error for PermissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Site(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for PermissionError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<SiteKeyError> for PermissionError {
    fn from(err: SiteKeyError) -> Self {
        Self::Site(err)
    }
}
