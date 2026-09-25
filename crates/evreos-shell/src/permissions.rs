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

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

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
    pub decided_at: Option<SystemTime>,
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
            decided_at: Some(SystemTime::now()),
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

type PermissionRecord = (PermissionDecision, Option<SystemTime>);
type PermissionMap = HashMap<(SiteKey, Capability), PermissionRecord>;

/// Persistent and transient store for site permissions.
#[derive(Debug)]
pub struct PermissionStore {
    root_path: Option<PathBuf>,
    persistent: PermissionMap,
    private: HashMap<AppWindowId, PermissionMap>,
    availability: HashMap<Capability, bool>,
}

impl PermissionStore {
    /// Open the permission store under a profile root directory.
    pub fn open(profile_root: impl Into<PathBuf>) -> Self {
        let root = profile_root.into();
        let file_path = root.join("permissions.toml");
        let persistent = if file_path.is_file() {
            match fs::read_to_string(&file_path) {
                Ok(content) => parse_permissions_file(&content).unwrap_or_default(),
                Err(_) => HashMap::new(),
            }
        } else {
            HashMap::new()
        };

        let mut availability = HashMap::new();
        for cap in Capability::ALL {
            availability.insert(cap, true);
        }

        Self {
            root_path: Some(root),
            persistent,
            private: HashMap::new(),
            availability,
        }
    }

    /// Create an in-memory permission store with no persistent disk backing.
    pub fn in_memory() -> Self {
        let mut availability = HashMap::new();
        for cap in Capability::ALL {
            availability.insert(cap, true);
        }

        Self {
            root_path: None,
            persistent: HashMap::new(),
            private: HashMap::new(),
            availability,
        }
    }

    /// Query the current permission decision for a site and capability.
    ///
    /// Defaults to [`PermissionDecision::Ask`] for every unconfigured pair.
    /// If the platform cannot deliver the capability, returns [`PermissionDecision::UnavailableOnThisPlatform`].
    pub fn query(
        &self,
        site: &SiteKey,
        capability: Capability,
        window_scope: WindowScope,
    ) -> PermissionDecision {
        // 1. If platform cannot deliver capability, it is Unavailable (FR-037).
        if !self.is_capability_available(capability) {
            return PermissionDecision::UnavailableOnThisPlatform;
        }

        // 2. If private window, check transient private store first.
        if let WindowScope::PrivateWindow(id) = window_scope {
            if let Some(dec) = self
                .private
                .get(&id)
                .and_then(|m| m.get(&(site.clone(), capability)))
            {
                return dec.0;
            }
            // Private windows default to Ask, isolating from persistent grants
            return PermissionDecision::Ask;
        }

        // 3. Persistent scope: check persistent map, defaulting to Ask.
        self.persistent
            .get(&(site.clone(), capability))
            .map(|(d, _)| *d)
            .unwrap_or(PermissionDecision::Ask)
    }

    /// Record a member decision for a site and capability under the given window scope.
    pub fn record_decision(
        &mut self,
        site: SiteKey,
        capability: Capability,
        decision: PermissionDecision,
        window_scope: WindowScope,
    ) -> Result<(), PermissionError> {
        let now = Some(SystemTime::now());
        match window_scope {
            WindowScope::PrivateWindow(id) => {
                // Stored exclusively in transient memory; never written to disk (FR-007)
                self.private
                    .entry(id)
                    .or_default()
                    .insert((site, capability), (decision, now));
                Ok(())
            }
            WindowScope::Persistent => {
                self.persistent.insert((site, capability), (decision, now));
                self.save()
            }
        }
    }

    /// Revoke a decision, returning it to default [`PermissionDecision::Ask`].
    pub fn revoke(
        &mut self,
        site: &SiteKey,
        capability: Capability,
        window_scope: WindowScope,
    ) -> Result<(), PermissionError> {
        match window_scope {
            WindowScope::PrivateWindow(id) => {
                if let Some(m) = self.private.get_mut(&id) {
                    m.remove(&(site.clone(), capability));
                }
                Ok(())
            }
            WindowScope::Persistent => {
                self.persistent.remove(&(site.clone(), capability));
                self.save()
            }
        }
    }

    /// Purge all transient permissions associated with a private window upon closure.
    pub fn purge_private_window(&mut self, window_id: AppWindowId) {
        self.private.remove(&window_id);
    }

    /// Set platform availability for a capability (for FR-037 handling).
    pub fn set_capability_available(&mut self, capability: Capability, available: bool) {
        self.availability.insert(capability, available);
    }

    /// Check if a capability is available on the current platform.
    pub fn is_capability_available(&self, capability: Capability) -> bool {
        *self.availability.get(&capability).unwrap_or(&true)
    }

    /// Return all persistent permissions currently stored.
    pub fn all_persistent(&self) -> Vec<SitePermission> {
        self.persistent
            .iter()
            .map(|((site, cap), (dec, ts))| SitePermission {
                site: site.clone(),
                capability: *cap,
                decision: *dec,
                decided_at: *ts,
                window_scope: WindowScope::Persistent,
            })
            .collect()
    }

    /// Evaluate an incoming permission request for a site and capability.
    ///
    /// - If the capability is unavailable on this platform, returns [`PromptOutcome::UnavailableOnThisPlatform`]
    ///   with `handoff_offered: true`, routing to FR-037 hand-off without presenting a prompt or denial.
    /// - If access is already granted, returns [`PromptOutcome::AlreadyGranted`].
    /// - If access is already denied, returns [`PromptOutcome::AlreadyDenied`].
    /// - If the decision is `Ask`, returns [`PromptOutcome::NeedsPrompt`] containing the prompt request.
    pub fn evaluate_request(
        &self,
        site: &SiteKey,
        capability: Capability,
        window_scope: WindowScope,
    ) -> PromptOutcome {
        let decision = self.query(site, capability, window_scope);
        match decision {
            PermissionDecision::UnavailableOnThisPlatform => {
                PromptOutcome::UnavailableOnThisPlatform {
                    handoff_offered: true,
                }
            }
            PermissionDecision::Granted => PromptOutcome::AlreadyGranted,
            PermissionDecision::Denied => PromptOutcome::AlreadyDenied,
            PermissionDecision::Ask => PromptOutcome::NeedsPrompt(PermissionPromptRequest {
                site: site.clone(),
                capability,
                window_scope,
            }),
        }
    }

    /// Process a member's response to an interactive permission prompt.
    pub fn handle_prompt_response(
        &mut self,
        site: SiteKey,
        capability: Capability,
        response: PromptResponse,
        window_scope: WindowScope,
    ) -> Result<PromptResult, PermissionError> {
        match response {
            PromptResponse::Grant { remember } => {
                if remember {
                    self.record_decision(
                        site,
                        capability,
                        PermissionDecision::Granted,
                        window_scope,
                    )?;
                }
                Ok(PromptResult::AccessGranted {
                    remembered: remember,
                })
            }
            PromptResponse::Deny { remember } => {
                if remember {
                    self.record_decision(
                        site,
                        capability,
                        PermissionDecision::Denied,
                        window_scope,
                    )?;
                }
                Ok(PromptResult::AccessDenied {
                    remembered: remember,
                })
            }
            PromptResponse::Dismiss => Ok(PromptResult::Dismissed),
        }
    }

    /// Save persistent permissions atomically to `permissions.toml`.
    fn save(&self) -> Result<(), PermissionError> {
        let root = match &self.root_path {
            Some(p) => p,
            None => return Ok(()),
        };

        if !root.exists() {
            fs::create_dir_all(root)?;
        }

        let serialized = serialize_permissions(&self.persistent);
        let target_path = root.join("permissions.toml");
        let tmp_path = root.join(".permissions.toml.tmp");

        fs::write(&tmp_path, serialized.as_bytes())?;
        fs::rename(&tmp_path, &target_path)?;

        Ok(())
    }
}

fn serialize_permissions(entries: &PermissionMap) -> String {
    let mut out = String::new();
    out.push_str("# Evreos site permissions store\n");
    out.push_str("# FR-006, Decision 0007\n\n");

    for ((site, cap), (dec, ts)) in entries {
        out.push_str("[[permission]]\n");
        out.push_str(&format!("site = \"{}\"\n", site.as_str()));
        out.push_str(&format!("capability = \"{}\"\n", cap.as_str()));
        out.push_str(&format!("decision = \"{}\"\n", dec.as_str()));
        if let Some(t) = ts {
            if let Ok(duration) = t.duration_since(UNIX_EPOCH) {
                out.push_str(&format!("decided_at = {}\n", duration.as_secs()));
            }
        }
        out.push('\n');
    }

    out
}

fn parse_permissions_file(content: &str) -> Result<PermissionMap, PermissionError> {
    let mut map = HashMap::new();
    let mut current_site: Option<SiteKey> = None;
    let mut current_cap: Option<Capability> = None;
    let mut current_dec: Option<PermissionDecision> = None;
    let mut current_ts: Option<SystemTime> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if trimmed == "[[permission]]" {
            if let (Some(site), Some(cap), Some(dec)) =
                (current_site.take(), current_cap.take(), current_dec.take())
            {
                map.insert((site, cap), (dec, current_ts.take()));
            }
            continue;
        }

        if let Some((key, val)) = trimmed.split_once('=') {
            let key = key.trim();
            let val = val.trim();
            match key {
                "site" => {
                    let unquoted = unquote_str(val)?;
                    current_site = Some(SiteKey::new(unquoted)?);
                }
                "capability" => {
                    let unquoted = unquote_str(val)?;
                    current_cap = Capability::parse(&unquoted);
                }
                "decision" => {
                    let unquoted = unquote_str(val)?;
                    current_dec = PermissionDecision::parse(&unquoted);
                }
                "decided_at" => {
                    if let Ok(secs) = val.parse::<u64>() {
                        current_ts = Some(UNIX_EPOCH + std::time::Duration::from_secs(secs));
                    }
                }
                _ => {}
            }
        }
    }

    if let (Some(site), Some(cap), Some(dec)) = (current_site, current_cap, current_dec) {
        map.insert((site, cap), (dec, current_ts));
    }

    Ok(map)
}

fn unquote_str(s: &str) -> Result<String, PermissionError> {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        Ok(s[1..s.len() - 1].to_string())
    } else {
        Err(PermissionError::InvalidFormat(format!(
            "expected quoted string: {s}"
        )))
    }
}

/// Outcome of evaluating an access request from a web page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptOutcome {
    /// Capability cannot be delivered by the engine or platform (FR-037).
    ///
    /// Distinct from `Denied`; never presented as a denial to the member.
    /// Routes directly to offering a hand-off to the external hand-off browser.
    UnavailableOnThisPlatform {
        /// Indicates that the browser should proactively offer the FR-037 hand-off.
        handoff_offered: bool,
    },
    /// Permission was previously granted for this site and capability.
    AlreadyGranted,
    /// Permission was previously denied for this site and capability.
    AlreadyDenied,
    /// Consent must be requested interactively from the member.
    NeedsPrompt(PermissionPromptRequest),
}

/// Request for interactive member consent for a site capability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionPromptRequest {
    /// The requesting site.
    pub site: SiteKey,
    /// The capability requested.
    pub capability: Capability,
    /// The window scope (persistent vs private).
    pub window_scope: WindowScope,
}

/// Member's choice in response to a permission prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptResponse {
    /// Grant access, optionally persisting the decision for future visits.
    Grant {
        /// If true, persist decision across sessions; if false, one-time grant.
        remember: bool,
    },
    /// Deny access, optionally persisting the decision for future visits.
    Deny {
        /// If true, persist denial across sessions; if false, one-time denial.
        remember: bool,
    },
    /// Dismiss the prompt without making a persistent decision.
    Dismiss,
}

/// Result of processing a prompt response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptResult {
    /// Access was granted.
    AccessGranted {
        /// Whether the decision was persisted.
        remembered: bool,
    },
    /// Access was denied.
    AccessDenied {
        /// Whether the decision was persisted.
        remembered: bool,
    },
    /// Prompt was dismissed with no permission change.
    Dismissed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_decision_is_ask() {
        let store = PermissionStore::in_memory();
        let site = SiteKey::from_host("example.invalid").unwrap();

        for cap in Capability::ALL {
            assert_eq!(
                store.query(&site, cap, WindowScope::Persistent),
                PermissionDecision::Ask
            );
        }
    }

    #[test]
    fn grant_deny_and_revoke_revisitable() {
        let mut store = PermissionStore::in_memory();
        let site = SiteKey::from_host("telehealth.invalid").unwrap();

        // Grant camera
        store
            .record_decision(
                site.clone(),
                Capability::Camera,
                PermissionDecision::Granted,
                WindowScope::Persistent,
            )
            .unwrap();
        assert_eq!(
            store.query(&site, Capability::Camera, WindowScope::Persistent),
            PermissionDecision::Granted
        );

        // Deny microphone
        store
            .record_decision(
                site.clone(),
                Capability::Microphone,
                PermissionDecision::Denied,
                WindowScope::Persistent,
            )
            .unwrap();
        assert_eq!(
            store.query(&site, Capability::Microphone, WindowScope::Persistent),
            PermissionDecision::Denied
        );

        // Revoke camera -> back to Ask without reinstalling
        store
            .revoke(&site, Capability::Camera, WindowScope::Persistent)
            .unwrap();
        assert_eq!(
            store.query(&site, Capability::Camera, WindowScope::Persistent),
            PermissionDecision::Ask
        );
    }

    #[test]
    fn unavailable_on_this_platform_never_presented_as_denial() {
        let mut store = PermissionStore::in_memory();
        let site = SiteKey::from_host("map.invalid").unwrap();

        // Mark location unavailable on this platform
        store.set_capability_available(Capability::Location, false);

        assert_eq!(
            store.query(&site, Capability::Location, WindowScope::Persistent),
            PermissionDecision::UnavailableOnThisPlatform
        );

        // Evaluate request returns Unavailable with handoff offered, NOT NeedsPrompt or Denied
        let outcome = store.evaluate_request(&site, Capability::Location, WindowScope::Persistent);
        assert_eq!(
            outcome,
            PromptOutcome::UnavailableOnThisPlatform {
                handoff_offered: true
            }
        );
    }

    #[test]
    fn private_window_decisions_die_with_the_window() {
        let mut store = PermissionStore::in_memory();
        let site = SiteKey::from_host("secret.invalid").unwrap();
        let win_id = AppWindowId::FIRST;
        let private_scope = WindowScope::PrivateWindow(win_id);

        // In private window, record grant
        store
            .record_decision(
                site.clone(),
                Capability::Camera,
                PermissionDecision::Granted,
                private_scope,
            )
            .unwrap();

        // In private window, it is granted
        assert_eq!(
            store.query(&site, Capability::Camera, private_scope),
            PermissionDecision::Granted
        );

        // In persistent window, it was never touched (stays Ask)
        assert_eq!(
            store.query(&site, Capability::Camera, WindowScope::Persistent),
            PermissionDecision::Ask
        );

        // Window closes -> purge private window
        store.purge_private_window(win_id);

        // Now private window also returns Ask
        assert_eq!(
            store.query(&site, Capability::Camera, private_scope),
            PermissionDecision::Ask
        );
    }

    #[test]
    fn subdomains_share_permissions_via_site_key() {
        let mut store = PermissionStore::in_memory();
        let bank_root = SiteKey::from_url("https://bank.invalid/").unwrap();
        let bank_login = SiteKey::from_url("https://login.bank.invalid/signin").unwrap();

        assert_eq!(bank_root, bank_login);

        store
            .record_decision(
                bank_root.clone(),
                Capability::Camera,
                PermissionDecision::Granted,
                WindowScope::Persistent,
            )
            .unwrap();

        // Grant on bank root is observed when querying bank login subdomain
        assert_eq!(
            store.query(&bank_login, Capability::Camera, WindowScope::Persistent),
            PermissionDecision::Granted
        );
    }
}
