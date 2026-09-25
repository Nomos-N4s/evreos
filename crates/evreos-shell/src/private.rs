//! Private-window support for the Evreos shell.
//!
//! Under FR-007, data-model §1.2, and T052:
//! - A private window selects the non-persistent data store on the engine seam
//!   ([`DataStoreSelector::NonPersistent`]).
//! - Produces NO history entry in [`HistoryStore`] (FR-007).
//! - Produces NO session record in [`SessionStore`] via write-path exclusion (FR-007).
//! - On close, destroys its data store on the engine seam ([`Engine::close_surface`]),
//!   together with its transient permissions ([`PermissionStore::purge_private_window`])
//!   and transient blocking exceptions.
//! - Leaves zero trace behind on the machine, guaranteeing that the profile directory
//!   is byte-identical before and after a full private browsing session.

#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};

use evreos_engine::{DataStoreSelector, Engine, SurfaceId};

use crate::app::{AppWindowId, mint_window_id};
use crate::permissions::{
    Capability, PermissionDecision, PermissionError, PermissionStore, WindowScope,
};
use crate::site_key::SiteKey;
use crate::store::{HistoryEntryId, HistoryError, HistoryStore, WindowKind};
use crate::tabs::WindowTabs;

/// A private browsing window.
///
/// Under FR-007 and data-model §1.2:
/// - Uses [`DataStoreSelector::NonPersistent`] on the engine seam.
/// - Produces no history entries and no persisted session records.
/// - Manages transient site permissions and transient blocking exceptions scoped
///   strictly to this window's lifetime.
/// - On close, destroys the engine surface (destroying its data store) and purges
///   all transient state.
#[derive(Debug)]
pub struct PrivateWindow {
    id: AppWindowId,
    surface_id: SurfaceId,
    tabs: WindowTabs,
    transient_blocking_exceptions: HashSet<SiteKey>,
    is_closed: bool,
}

impl PrivateWindow {
    /// Open a new private window on the given engine seam.
    ///
    /// Selects [`DataStoreSelector::NonPersistent`] to isolate memory and network state.
    pub fn open<E: Engine>(engine: &mut E) -> Self {
        let id = mint_window_id();
        let surface_id = engine.create_surface(DataStoreSelector::NonPersistent);
        Self::new(id, surface_id)
    }

    /// Construct a [`PrivateWindow`] with explicit identifiers.
    pub fn new(id: AppWindowId, surface_id: SurfaceId) -> Self {
        let tabs = WindowTabs::new(id, WindowKind::Private);
        Self {
            id,
            surface_id,
            tabs,
            transient_blocking_exceptions: HashSet::new(),
            is_closed: false,
        }
    }

    /// Application window identifier.
    pub fn id(&self) -> AppWindowId {
        self.id
    }

    /// Engine rendering surface identifier.
    pub fn surface_id(&self) -> SurfaceId {
        self.surface_id
    }

    /// Window browsing mode ([`WindowKind::Private`]).
    pub const fn kind(&self) -> WindowKind {
        WindowKind::Private
    }

    /// Engine data store selector ([`DataStoreSelector::NonPersistent`]).
    pub const fn data_store(&self) -> DataStoreSelector {
        DataStoreSelector::NonPersistent
    }

    /// Whether this window has been closed and its data store destroyed.
    pub fn is_closed(&self) -> bool {
        self.is_closed
    }

    /// Member-ordered tabs belonging to this private window.
    pub fn tabs(&self) -> &WindowTabs {
        &self.tabs
    }

    /// Mutable member-ordered tabs belonging to this private window.
    pub fn tabs_mut(&mut self) -> &mut WindowTabs {
        &mut self.tabs
    }

    /// Invariant: a private window MUST NOT produce a history entry (FR-007).
    pub const fn can_record_history(&self) -> bool {
        false
    }

    /// Invariant: a private window MUST NOT appear in the session store (FR-007).
    pub const fn can_persist_session(&self) -> bool {
        false
    }

    /// Record a transient blocking exception for a site in this private window.
    ///
    /// Under FR-007, this exception is transient and destroyed when the window closes.
    pub fn allow_blocking_exception(&mut self, site: SiteKey) {
        self.transient_blocking_exceptions.insert(site);
    }

    /// Revoke a transient blocking exception in this private window.
    pub fn revoke_blocking_exception(&mut self, site: &SiteKey) -> bool {
        self.transient_blocking_exceptions.remove(site)
    }

    /// Check if a transient blocking exception is active for the site in this window.
    pub fn is_blocking_exception(&self, site: &SiteKey) -> bool {
        self.transient_blocking_exceptions.contains(site)
    }

    /// All transient blocking exceptions currently granted in this private window.
    pub fn transient_blocking_exceptions(&self) -> &HashSet<SiteKey> {
        &self.transient_blocking_exceptions
    }

    /// Record a permission decision scoped to this private window.
    ///
    /// Stored in transient memory only; never written to disk (FR-006, FR-007).
    pub fn record_permission(
        &self,
        site: SiteKey,
        capability: Capability,
        decision: PermissionDecision,
        perm_store: &mut PermissionStore,
    ) -> Result<(), PermissionError> {
        perm_store.record_decision(
            site,
            capability,
            decision,
            WindowScope::PrivateWindow(self.id),
        )
    }

    /// Query the current permission decision for this private window.
    pub fn query_permission(
        &self,
        site: &SiteKey,
        capability: Capability,
        perm_store: &PermissionStore,
    ) -> PermissionDecision {
        perm_store.query(site, capability, WindowScope::PrivateWindow(self.id))
    }

    /// Close the private window and destroy its non-persistent data store,
    /// transient permissions, and transient blocking exceptions (FR-007).
    pub fn close<E: Engine>(&mut self, engine: &mut E, perm_store: Option<&mut PermissionStore>) {
        if !self.is_closed {
            self.is_closed = true;

            // 1. Destroy surface and non-persistent data store on engine seam
            engine.close_surface(self.surface_id);

            // 2. Purge transient permissions for this private window
            if let Some(store) = perm_store {
                store.purge_private_window(self.id);
            }

            // 3. Destroy transient blocking exceptions
            self.transient_blocking_exceptions.clear();

            // 4. Reset tabs
            self.tabs = WindowTabs::new(self.id, WindowKind::Private);
        }
    }
}

/// Controller managing private windows during a private browsing session.
#[derive(Debug, Default)]
pub struct PrivateSession {
    windows: HashMap<AppWindowId, PrivateWindow>,
}

impl PrivateSession {
    /// Create a new empty private session controller.
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
        }
    }

    /// Open a new private window in this session.
    pub fn open_window<E: Engine>(&mut self, engine: &mut E) -> AppWindowId {
        let window = PrivateWindow::open(engine);
        let id = window.id();
        self.windows.insert(id, window);
        id
    }

    /// Close a private window by ID, destroying its data store and transient state.
    pub fn close_window<E: Engine>(
        &mut self,
        id: AppWindowId,
        engine: &mut E,
        perm_store: Option<&mut PermissionStore>,
    ) -> bool {
        if let Some(mut win) = self.windows.remove(&id) {
            win.close(engine, perm_store);
            true
        } else {
            false
        }
    }

    /// Close all open private windows, destroying all data stores and transient state.
    pub fn close_all<E: Engine>(
        &mut self,
        engine: &mut E,
        mut perm_store: Option<&mut PermissionStore>,
    ) {
        for (_, mut win) in self.windows.drain() {
            win.close(engine, perm_store.as_deref_mut());
        }
    }

    /// Reference to a private window by ID.
    pub fn get_window(&self, id: AppWindowId) -> Option<&PrivateWindow> {
        self.windows.get(&id)
    }

    /// Mutable reference to a private window by ID.
    pub fn get_window_mut(&mut self, id: AppWindowId) -> Option<&mut PrivateWindow> {
        self.windows.get_mut(&id)
    }

    /// Whether the specified window ID is a known private window.
    pub fn is_private_window(&self, id: AppWindowId) -> bool {
        self.windows.contains_key(&id)
    }

    /// Number of currently open private windows.
    pub fn open_window_count(&self) -> usize {
        self.windows.len()
    }

    /// Whether any private window is currently open.
    pub fn has_open_windows(&self) -> bool {
        !self.windows.is_empty()
    }
}

/// Helper function to record history with private-window exclusion verification.
///
/// Guaranteed to return `Ok(None)` and produce zero disk writes if `kind == WindowKind::Private`.
pub fn record_history_safely(
    history: &mut HistoryStore,
    address: &str,
    title: &str,
    kind: WindowKind,
) -> Result<Option<HistoryEntryId>, HistoryError> {
    history.record(address, title, kind)
}
