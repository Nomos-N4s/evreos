//! Tab and window model for the Evreos shell.
//!
//! Under FR-001, FR-002, FR-015, FR-018a, and SC-006:
//! - Implements tab lifecycle (`Loading`, `Live`, `Suspended`, `RestoredNotLoaded`, `Failed`).
//! - Enforces mutual exclusion: a failed load and a rendered page are strictly mutually exclusive.
//! - Tracks monotonic `navigation_epoch`, incrementing on every change to the displayed address
//!   (including same-document navigations) and correlating with engine observation epochs.
//! - Preserves member-ordered tab positions stably across suspension, reactivation, and restarts.
//! - Resolves stalled loads past the 30-second bound through the shell's existing in-flight policy
//!   and injectable clock source, rather than inventing synthetic `LoadError` variants.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use evreos_engine::{
    LoadError, NavigationEpoch, NavigationEvent, NavigationId, NavigationObservation,
};

use crate::app::AppWindowId;
use crate::scaling::FindInPageState;
use crate::store::WindowKind;
use crate::suggest::OpenTab;

/// Default navigation timeout bound (30 seconds per SC-009).
pub const DEFAULT_NAVIGATION_TIMEOUT: Duration = Duration::from_secs(30);

/// Source of monotonic time for navigation timeout tracking.
pub trait Clock: Send + Sync + 'static {
    /// Return the current monotonic time.
    fn now(&self) -> Instant;
}

/// Real time clock implementation using [`Instant::now`].
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// Simulated clock for testing timeout policy deterministically.
#[derive(Debug, Clone)]
pub struct MockClock {
    now: Instant,
}

impl MockClock {
    pub fn new(start: Instant) -> Self {
        Self { now: start }
    }

    pub fn advance(&mut self, duration: Duration) {
        self.now += duration;
    }

    pub fn set(&mut self, now: Instant) {
        self.now = now;
    }
}

impl Clock for MockClock {
    fn now(&self) -> Instant {
        self.now
    }
}

/// State of an in-flight or completed navigation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationState {
    /// Navigation started and is awaiting commit or outcome.
    Loading { start_time: Instant },
    /// Navigation committed to a specific address.
    Committed { address: String },
    /// Navigation successfully finished.
    Succeeded { address: String, title: String },
    /// Navigation failed with an engine or timeout error.
    Failed { error_message: String },
    /// Navigation was superseded or abandoned.
    NavigatedAway,
}

/// Tracks in-flight navigations by [`NavigationId`] and enforces shell policy.
#[derive(Debug)]
pub struct NavigationTracker<C: Clock> {
    clock: C,
    timeout_bound: Duration,
    navigations: HashMap<NavigationId, NavigationState>,
    requested_addresses: HashMap<NavigationId, String>,
    titles: HashMap<NavigationId, String>,
}

impl<C: Clock> NavigationTracker<C> {
    pub fn new(clock: C) -> Self {
        Self::with_timeout_bound(clock, DEFAULT_NAVIGATION_TIMEOUT)
    }

    pub fn with_timeout_bound(clock: C, timeout_bound: Duration) -> Self {
        Self {
            clock,
            timeout_bound,
            navigations: HashMap::new(),
            requested_addresses: HashMap::new(),
            titles: HashMap::new(),
        }
    }

    pub fn clock(&self) -> &C {
        &self.clock
    }

    pub fn clock_mut(&mut self) -> &mut C {
        &mut self.clock
    }

    pub fn timeout_bound(&self) -> Duration {
        self.timeout_bound
    }

    /// Record the start of a navigation.
    pub fn start_navigation(&mut self, id: NavigationId, requested_address: String) {
        self.requested_addresses.insert(id, requested_address);
        self.navigations.insert(
            id,
            NavigationState::Loading {
                start_time: self.clock.now(),
            },
        );
    }

    /// Process a single [`NavigationEvent`] from the engine.
    pub fn process_event(&mut self, event: NavigationEvent) {
        let id = event.id();
        match event {
            NavigationEvent::Started { address, .. } => {
                self.requested_addresses.entry(id).or_insert(address);
                self.navigations
                    .entry(id)
                    .or_insert(NavigationState::Loading {
                        start_time: self.clock.now(),
                    });
            }
            NavigationEvent::Redirected { .. } => {
                // Address redirected before commit; status remains Loading until Committed.
            }
            NavigationEvent::Committed { address, .. }
            | NavigationEvent::SameDocumentNavigated { address, .. } => {
                self.navigations
                    .insert(id, NavigationState::Committed { address });
            }
            NavigationEvent::Succeeded { .. } => {
                let loaded_address = match self.navigations.get(&id) {
                    Some(NavigationState::Committed { address })
                    | Some(NavigationState::Succeeded { address, .. }) => address.clone(),
                    _ => self
                        .requested_addresses
                        .get(&id)
                        .cloned()
                        .unwrap_or_default(),
                };
                let title = self.titles.get(&id).cloned().unwrap_or_default();
                self.navigations.insert(
                    id,
                    NavigationState::Succeeded {
                        address: loaded_address,
                        title,
                    },
                );
            }
            NavigationEvent::Failed { error, .. } => {
                let formatted = format_load_error(&error);
                self.navigations.insert(
                    id,
                    NavigationState::Failed {
                        error_message: formatted,
                    },
                );
            }
            NavigationEvent::TitleChanged {
                title: new_title, ..
            } => {
                self.titles.insert(id, new_title.clone());
                if let Some(NavigationState::Succeeded { title, .. }) =
                    self.navigations.get_mut(&id)
                {
                    *title = new_title;
                }
            }
            NavigationEvent::NavigatedAway { .. } => {
                self.navigations.insert(id, NavigationState::NavigatedAway);
            }
        }
    }

    /// Check for in-flight navigations past the timeout bound and resolve them into timeout error states.
    pub fn check_timeouts(&mut self) -> Vec<(NavigationId, String)> {
        let now = self.clock.now();
        let mut timed_out = Vec::new();
        for (id, state) in self.navigations.iter_mut() {
            if let NavigationState::Loading { start_time } = state {
                if now.duration_since(*start_time) >= self.timeout_bound {
                    let req_addr = self
                        .requested_addresses
                        .get(id)
                        .cloned()
                        .unwrap_or_else(|| "unknown address".to_owned());
                    let timeout_secs = self.timeout_bound.as_secs();
                    let msg = format!(
                        "navigation to {req_addr} timed out after {timeout_secs}s: check your network connection or try reloading"
                    );
                    *state = NavigationState::Failed {
                        error_message: msg.clone(),
                    };
                    timed_out.push((*id, msg));
                }
            }
        }
        timed_out
    }

    /// Get current state of a navigation.
    pub fn get_state(&self, id: NavigationId) -> Option<&NavigationState> {
        self.navigations.get(&id)
    }

    /// Format current display string for a navigation.
    pub fn display_status(&self, id: NavigationId) -> String {
        let req_addr = self
            .requested_addresses
            .get(&id)
            .map(|s| s.as_str())
            .unwrap_or("unknown");

        match self.navigations.get(&id) {
            Some(NavigationState::Loading { .. }) => format!("{req_addr} is still loading"),
            Some(NavigationState::Committed { address }) => {
                format!("loading response from {address}...")
            }
            Some(NavigationState::Succeeded { address, title }) => {
                if title.is_empty() {
                    address.clone()
                } else {
                    format!("{title} — {address}")
                }
            }
            Some(NavigationState::Failed { error_message }) => error_message.clone(),
            Some(NavigationState::NavigatedAway) => {
                format!("{req_addr} was abandoned before it resolved")
            }
            None => format!("{req_addr} has no recorded navigation state"),
        }
    }
}

/// Format engine [`LoadError`] naming cause and offering next step.
pub fn format_load_error(error: &LoadError) -> String {
    match error {
        LoadError::Unresolvable { address } => {
            format!(
                "could not load {address}: server address could not be found. Next step: check the address for typos or verify network connection."
            )
        }
        LoadError::Certificate { address, detail } => {
            format!(
                "could not load {address}: security certificate error ({detail}). Next step: verify your system clock or do not proceed if on a public network."
            )
        }
        LoadError::Intercepted { address } => {
            format!(
                "could not load {address}: connection intercepted by a captive portal or proxy. Next step: log in to the network or check proxy settings."
            )
        }
        LoadError::AuthenticationRequired { address } => {
            format!(
                "could not load {address}: HTTP authentication required. Next step: enter valid credentials when prompted."
            )
        }
    }
}

/// Unique identifier for an open browser tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TabId(u64);

impl TabId {
    /// Initial tab identifier.
    pub const FIRST: Self = Self(1);

    /// Construct a [`TabId`] from a raw `u64`.
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Return the raw identifier number.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Return the next sequential [`TabId`].
    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

impl fmt::Display for TabId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tab-{}", self.0)
    }
}

static NEXT_TAB_ID: AtomicU64 = AtomicU64::new(1);

/// Mint a globally unique [`TabId`] across the shell session.
pub fn mint_tab_id() -> TabId {
    TabId(NEXT_TAB_ID.fetch_add(1, Ordering::Relaxed))
}

/// Lifecycle state of a browser tab.
///
/// Under FR-001, FR-002, and FR-015:
/// - `Loading`: tab has an in-flight navigation.
/// - `Live`: tab has committed and succeeded; page content is actively rendered.
/// - `Suspended`: tab was backgrounded and suspended following stated policy,
///   reversible without losing visible page state.
/// - `RestoredNotLoaded`: tab restored from session with navigation deferred until activation.
/// - `Failed(LoadError)`: tab navigation failed with an engine load error; strictly
///   mutually exclusive with a rendered page (FR-015).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabLifecycle {
    /// Tab is loading an address.
    Loading,
    /// Tab is live with rendered page content.
    Live,
    /// Tab is suspended to conserve memory/CPU; page state is preserved.
    Suspended,
    /// Tab is restored across session restart; load is deferred until activation.
    RestoredNotLoaded,
    /// Tab failed to load. Mutually exclusive with Live (rendered page).
    Failed(LoadError),
}

impl TabLifecycle {
    /// Whether the tab is currently loading.
    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }

    /// Whether the tab has rendered live page content.
    pub fn is_live(&self) -> bool {
        matches!(self, Self::Live)
    }

    /// Whether the tab is currently suspended.
    pub fn is_suspended(&self) -> bool {
        matches!(self, Self::Suspended)
    }

    /// Whether the tab is restored without its content loaded yet.
    pub fn is_restored_not_loaded(&self) -> bool {
        matches!(self, Self::RestoredNotLoaded)
    }

    /// Whether the tab failed to load.
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }

    /// The engine [`LoadError`] if in the failed state.
    pub fn failed_error(&self) -> Option<&LoadError> {
        match self {
            Self::Failed(err) => Some(err),
            _ => None,
        }
    }
}

/// Errors that can occur during tab lifecycle and window tab operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabError {
    /// The specified tab was not found in the window.
    TabNotFound(TabId),
    /// The target position is outside the valid range `0..=len`.
    InvalidPosition { requested: usize, count: usize },
    /// Illegal lifecycle transition.
    InvalidTransition {
        from: TabLifecycle,
        action: &'static str,
    },
}

impl fmt::Display for TabError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TabNotFound(id) => write!(f, "tab not found: {id}"),
            Self::InvalidPosition { requested, count } => {
                write!(
                    f,
                    "target position {requested} out of bounds (count is {count})"
                )
            }
            Self::InvalidTransition { from, action } => {
                write!(f, "cannot execute action '{action}' from state {from:?}")
            }
        }
    }
}

impl std::error::Error for TabError {}

/// An open browser tab.
///
/// Owns its lifecycle, displayed address, title, position, and monotonic [`NavigationEpoch`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tab {
    id: TabId,
    window_id: AppWindowId,
    position: usize,
    displayed_address: String,
    title: String,
    navigation_epoch: NavigationEpoch,
    lifecycle: TabLifecycle,
    loading_started_at: Option<Instant>,
    in_flight_id: Option<NavigationId>,
    page_zoom: u32,
    find_state: FindInPageState,
}

impl Tab {
    /// Create a new tab.
    pub fn new(
        id: TabId,
        window_id: AppWindowId,
        position: usize,
        address: impl Into<String>,
        lifecycle: TabLifecycle,
        loading_started_at: Option<Instant>,
    ) -> Self {
        Self {
            id,
            window_id,
            position,
            displayed_address: address.into(),
            title: String::new(),
            navigation_epoch: NavigationEpoch::FIRST,
            lifecycle,
            loading_started_at,
            in_flight_id: None,
            page_zoom: 100,
            find_state: FindInPageState::new(),
        }
    }

    /// Tab unique identifier.
    pub fn id(&self) -> TabId {
        self.id
    }

    /// Owning application window identifier.
    pub fn window_id(&self) -> AppWindowId {
        self.window_id
    }

    /// Tab index within the window's tab bar.
    pub fn position(&self) -> usize {
        self.position
    }

    /// The address currently displayed in the omnibox for this tab.
    pub fn displayed_address(&self) -> &str {
        &self.displayed_address
    }

    /// Page title for this tab.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Monotonic navigation epoch for this tab.
    ///
    /// Increments on **every** address change (including same-document navigation).
    pub fn navigation_epoch(&self) -> NavigationEpoch {
        self.navigation_epoch
    }

    /// Current lifecycle state.
    pub fn lifecycle(&self) -> &TabLifecycle {
        &self.lifecycle
    }

    /// Monotonic start time if currently loading.
    pub fn loading_started_at(&self) -> Option<Instant> {
        self.loading_started_at
    }

    /// The active in-flight navigation ID if loading.
    pub fn in_flight_id(&self) -> Option<NavigationId> {
        self.in_flight_id
    }

    /// Current page zoom percentage (default: 100).
    pub fn page_zoom(&self) -> u32 {
        self.page_zoom
    }

    /// Set page zoom percentage.
    pub fn set_page_zoom(&mut self, zoom: u32) {
        self.page_zoom = zoom;
    }

    /// Tab find-in-page state (FR-005).
    pub fn find_state(&self) -> &FindInPageState {
        &self.find_state
    }

    /// Mutable reference to tab find-in-page state (FR-005).
    pub fn find_state_mut(&mut self) -> &mut FindInPageState {
        &mut self.find_state
    }

    /// Mutual exclusion invariant (FR-015):
    /// A failed load and a rendered page are strictly mutually exclusive.
    pub fn is_rendered(&self) -> bool {
        self.lifecycle.is_live()
    }

    /// Project this tab into an [`OpenTab`] descriptor for the suggestion index.
    pub fn to_open_tab(&self) -> OpenTab {
        OpenTab::new(&self.displayed_address, &self.title)
    }

    /// Set tab title without advancing epoch or changing address.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    /// Transition to loading state with a new in-flight navigation.
    pub fn start_loading(&mut self, in_flight_id: NavigationId, now: Instant) {
        self.lifecycle = TabLifecycle::Loading;
        self.loading_started_at = Some(now);
        self.in_flight_id = Some(in_flight_id);
    }

    /// Update displayed address and increment navigation epoch.
    ///
    /// Per FR-018a: Increments on **every** change to the displayed address,
    /// including same-document navigations without fetching a new document.
    pub fn update_address(
        &mut self,
        address: impl Into<String>,
        explicit_epoch: Option<NavigationEpoch>,
    ) {
        let new_addr = address.into();
        self.displayed_address = new_addr;
        self.navigation_epoch = explicit_epoch.unwrap_or_else(|| self.navigation_epoch.next());
    }

    /// Mark a same-document navigation, advancing the epoch without replacing page state.
    pub fn same_document_navigate(
        &mut self,
        address: impl Into<String>,
        explicit_epoch: Option<NavigationEpoch>,
    ) {
        self.update_address(address, explicit_epoch);
    }

    /// Mark navigation succeeded, transitioning tab to [`TabLifecycle::Live`].
    pub fn mark_succeeded(&mut self, loaded_address: &str) {
        self.displayed_address = loaded_address.to_owned();
        self.lifecycle = TabLifecycle::Live;
        self.loading_started_at = None;
        self.in_flight_id = None;
    }

    /// Mark navigation failed, transitioning tab to [`TabLifecycle::Failed`].
    ///
    /// Under FR-015, a failed load and a rendered page are mutually exclusive.
    pub fn mark_failed(&mut self, error: LoadError) {
        self.lifecycle = TabLifecycle::Failed(error);
        self.loading_started_at = None;
        self.in_flight_id = None;
    }

    /// Suspend a live tab following stated policy (FR-002).
    ///
    /// Preserves tab position, address, title, and epoch.
    pub fn suspend(&mut self) -> Result<(), TabError> {
        if !self.lifecycle.is_live() {
            return Err(TabError::InvalidTransition {
                from: self.lifecycle.clone(),
                action: "suspend",
            });
        }
        self.lifecycle = TabLifecycle::Suspended;
        Ok(())
    }

    /// Resume a suspended tab back to [`TabLifecycle::Live`].
    ///
    /// Reversible without losing visible page state.
    pub fn resume(&mut self) -> Result<(), TabError> {
        if !self.lifecycle.is_suspended() {
            return Err(TabError::InvalidTransition {
                from: self.lifecycle.clone(),
                action: "resume",
            });
        }
        self.lifecycle = TabLifecycle::Live;
        Ok(())
    }

    /// Resolve in-flight loading if timed out under shell policy.
    ///
    /// Returns the timeout error message if this tab's in-flight navigation timed out.
    pub fn resolve_in_flight_timeout<C: Clock>(
        &mut self,
        tracker: &mut NavigationTracker<C>,
    ) -> Option<String> {
        let timed_outs = tracker.check_timeouts();
        if let Some(in_flight) = self.in_flight_id {
            for (id, msg) in &timed_outs {
                if *id == in_flight {
                    self.loading_started_at = None;
                    return Some(msg.clone());
                }
            }
        }
        None
    }
}

/// Collection of tabs belonging to an application window.
///
/// Owns the tab order, active tab selection, reordering, and lifecycle dispatch.
#[derive(Debug)]
pub struct WindowTabs {
    window_id: AppWindowId,
    kind: WindowKind,
    tabs: Vec<Tab>,
    active_tab_id: Option<TabId>,
}

impl WindowTabs {
    /// Create an empty tab collection for an application window.
    pub fn new(window_id: AppWindowId, kind: WindowKind) -> Self {
        Self {
            window_id,
            kind,
            tabs: Vec::new(),
            active_tab_id: None,
        }
    }

    /// Owning application window identifier.
    pub fn window_id(&self) -> AppWindowId {
        self.window_id
    }

    /// Window kind (`Normal` or `Private`).
    pub fn kind(&self) -> WindowKind {
        self.kind
    }

    /// Slice of all open tabs in member-ordered sequence.
    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    /// Number of open tabs.
    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    /// Whether the tab collection is empty.
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    /// Currently active [`TabId`], if any.
    pub fn active_tab_id(&self) -> Option<TabId> {
        self.active_tab_id
    }

    /// Reference to the currently active [`Tab`], if any.
    pub fn active_tab(&self) -> Option<&Tab> {
        self.active_tab_id.and_then(|id| self.get_tab(id))
    }

    /// Mutable reference to the currently active [`Tab`], if any.
    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        self.active_tab_id.and_then(|id| self.get_tab_mut(id))
    }

    /// Find tab by [`TabId`].
    pub fn get_tab(&self, id: TabId) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id() == id)
    }

    /// Find mutable tab by [`TabId`].
    pub fn get_tab_mut(&mut self, id: TabId) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|t| t.id() == id)
    }

    /// Open a new tab in the window in [`TabLifecycle::Loading`] state.
    pub fn open_tab<C: Clock>(&mut self, address: impl Into<String>, clock: &C) -> TabId {
        let id = mint_tab_id();
        let position = self.tabs.len();
        let tab = Tab::new(
            id,
            self.window_id,
            position,
            address,
            TabLifecycle::Loading,
            Some(clock.now()),
        );
        self.tabs.push(tab);
        if self.active_tab_id.is_none() {
            self.active_tab_id = Some(id);
        }
        id
    }

    /// Restore a tab from session in [`TabLifecycle::RestoredNotLoaded`] state.
    ///
    /// Under FR-001 and SC-002, loading is deferred until first activation.
    pub fn restore_tab(
        &mut self,
        address: impl Into<String>,
        title: impl Into<String>,
        position: usize,
        active: bool,
    ) -> TabId {
        let id = mint_tab_id();
        let mut tab = Tab::new(
            id,
            self.window_id,
            position,
            address,
            TabLifecycle::RestoredNotLoaded,
            None,
        );
        tab.set_title(title);
        self.tabs.push(tab);
        self.tabs.sort_by_key(|t| t.position());
        self.reindex_positions();

        if active || self.active_tab_id.is_none() {
            self.active_tab_id = Some(id);
        }
        id
    }

    /// Activate a tab by identifier.
    ///
    /// If the tab was [`TabLifecycle::RestoredNotLoaded`], transitions to [`TabLifecycle::Loading`].
    /// If the tab was [`TabLifecycle::Suspended`], resumes to [`TabLifecycle::Live`].
    pub fn activate_tab<C: Clock>(&mut self, id: TabId, clock: &C) -> Result<(), TabError> {
        let tab = self.get_tab_mut(id).ok_or(TabError::TabNotFound(id))?;
        if tab.lifecycle().is_restored_not_loaded() {
            tab.lifecycle = TabLifecycle::Loading;
            tab.loading_started_at = Some(clock.now());
        } else if tab.lifecycle().is_suspended() {
            tab.lifecycle = TabLifecycle::Live;
        }
        self.active_tab_id = Some(id);
        Ok(())
    }

    /// Suspend a tab following stated policy (FR-002).
    ///
    /// Tab position and member ordering are preserved.
    pub fn suspend_tab(&mut self, id: TabId) -> Result<(), TabError> {
        let tab = self.get_tab_mut(id).ok_or(TabError::TabNotFound(id))?;
        tab.suspend()
    }

    /// Resume a suspended tab back to [`TabLifecycle::Live`].
    ///
    /// Tab position and member ordering are preserved.
    pub fn resume_tab(&mut self, id: TabId) -> Result<(), TabError> {
        let tab = self.get_tab_mut(id).ok_or(TabError::TabNotFound(id))?;
        tab.resume()
    }

    /// Close a tab by identifier, re-indexing remaining tabs.
    pub fn close_tab(&mut self, id: TabId) -> Result<Tab, TabError> {
        let idx = self
            .tabs
            .iter()
            .position(|t| t.id() == id)
            .ok_or(TabError::TabNotFound(id))?;

        let removed = self.tabs.remove(idx);
        self.reindex_positions();

        if self.active_tab_id == Some(id) {
            self.active_tab_id = if self.tabs.is_empty() {
                None
            } else if idx < self.tabs.len() {
                Some(self.tabs[idx].id())
            } else {
                Some(self.tabs[self.tabs.len() - 1].id())
            };
        }

        Ok(removed)
    }

    /// Reorder a tab to a target position.
    ///
    /// Preserves stable ordering across suspension and reactivation.
    pub fn reorder_tab(&mut self, id: TabId, target_position: usize) -> Result<(), TabError> {
        if target_position >= self.tabs.len() {
            return Err(TabError::InvalidPosition {
                requested: target_position,
                count: self.tabs.len(),
            });
        }

        let current_idx = self
            .tabs
            .iter()
            .position(|t| t.id() == id)
            .ok_or(TabError::TabNotFound(id))?;

        if current_idx == target_position {
            return Ok(());
        }

        let tab = self.tabs.remove(current_idx);
        self.tabs.insert(target_position, tab);
        self.reindex_positions();
        Ok(())
    }

    /// Correlate engine [`NavigationObservation`] with the tab's state and epoch.
    pub fn handle_navigation_observation(
        &mut self,
        tab_id: TabId,
        obs: &NavigationObservation,
    ) -> Result<(), TabError> {
        let tab = self
            .get_tab_mut(tab_id)
            .ok_or(TabError::TabNotFound(tab_id))?;
        let epoch = obs.epoch();

        match obs.event() {
            NavigationEvent::Started { address, .. } => {
                tab.displayed_address = address.clone();
                tab.lifecycle = TabLifecycle::Loading;
            }
            NavigationEvent::Redirected { .. } => {}
            NavigationEvent::Committed { address, .. } => {
                tab.update_address(address, Some(epoch));
            }
            NavigationEvent::SameDocumentNavigated { address, .. } => {
                tab.same_document_navigate(address, Some(epoch));
            }
            NavigationEvent::Succeeded { .. } => {
                tab.lifecycle = TabLifecycle::Live;
                tab.loading_started_at = None;
                tab.in_flight_id = None;
            }
            NavigationEvent::Failed { error, .. } => {
                tab.mark_failed(error.clone());
            }
            NavigationEvent::TitleChanged { title, .. } => {
                tab.set_title(title);
            }
            NavigationEvent::NavigatedAway { .. } => {
                tab.loading_started_at = None;
                tab.in_flight_id = None;
            }
        }
        Ok(())
    }

    /// Check all tabs in the window for in-flight timeout resolution.
    pub fn check_in_flight_timeouts<C: Clock>(
        &mut self,
        tracker: &mut NavigationTracker<C>,
    ) -> Vec<(TabId, String)> {
        let timed_outs = tracker.check_timeouts();
        let mut results = Vec::new();

        for tab in &mut self.tabs {
            if let Some(in_flight) = tab.in_flight_id {
                for (id, msg) in &timed_outs {
                    if *id == in_flight {
                        tab.loading_started_at = None;
                        results.push((tab.id(), msg.clone()));
                    }
                }
            }
        }

        results
    }

    fn reindex_positions(&mut self) {
        for (i, tab) in self.tabs.iter_mut().enumerate() {
            tab.position = i;
        }
    }
}
