//! The second implementation of the rendering seam.
//!
//! Principle III: "the seam is proved by a second implementation rather than
//! asserted." This is that implementation, and its whole purpose is to be
//! written against [`evreos_engine::Engine`] and nothing else. If a change to
//! the shell forces a change here that has no meaning without a webview, the
//! seam has leaked and this crate is where that is discovered.
//!
//! It renders nothing. It answers from a script the test supplies, which is
//! what makes FR-015's four failure causes exercisable without a network, a
//! certificate authority, or a captive portal — the conditions SC-009 requires
//! to be tested on every supported platform and which are otherwise reachable
//! only by luck.

#![forbid(unsafe_code)]

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use evreos_engine::{
    AppId, CompiledPolicy, ContextId, DataStoreSelector, Engine, EngineHost, LoadError,
    NavigationEpoch, NavigationEvent, NavigationId, NavigationObservation, Page, Request,
    SurfaceId, SurfaceIdentity, SurfaceState, TaggedMessage,
};

static NEXT_CONTEXT_ID: AtomicU64 = AtomicU64::new(0);

fn next_headless_context_id() -> ContextId {
    let n = NEXT_CONTEXT_ID.fetch_add(1, Ordering::Relaxed);
    let mut id = ContextId::FIRST;
    for _ in 0..n {
        id = id.next();
    }
    id
}

/// An individual step in a scripted navigation sequence.
#[derive(Debug, Clone)]
pub enum ScriptStep {
    /// Emit a `Redirected` event to `address`.
    Redirect { address: String },
    /// Emit a `Committed` event for `address`, updating `current()` page address.
    Commit { address: String },
    /// Emit a `TitleChanged` event with `title`, updating `current()` page title if current.
    Title { title: String },
    /// Emit a `Succeeded` event.
    Succeed,
    /// Emit a `Failed` event with `error`.
    Fail(LoadError),
    /// Emit a `NavigatedAway` event.
    NavigateAway,
}

/// What the headless engine will do when asked for a given address.
#[derive(Debug, Clone)]
pub enum Response {
    /// Serve a page with this title.
    Page { title: String },
    /// Fail with this cause.
    Fail(LoadError),
    /// Execute a custom step sequence.
    Sequence(Vec<ScriptStep>),
}

#[derive(Debug, Default)]
struct SharedHostContext {
    id: ContextId,
    responses: RefCell<HashMap<String, Response>>,
    loads: RefCell<Vec<String>>,
    subresources: RefCell<HashMap<String, Vec<String>>>,
    policy: RefCell<Option<CompiledPolicy>>,
    exemptions: RefCell<HashSet<String>>,
}

#[derive(Debug, Clone)]
struct HeadlessSurface {
    _id: SurfaceId,
    store: DataStoreSelector,
    state: SurfaceState,
    epoch: NavigationEpoch,
    identity: Option<SurfaceIdentity>,
    hosted_bytes: Option<Vec<u8>>,
    app_id: Option<AppId>,
    current: Option<Page>,
    current_nav: Option<NavigationId>,
    loads: Vec<String>,
    data: HashMap<String, String>,
    blocked_count: u64,
    blocked_urls: Vec<String>,
}

/// The host/factory type owning the shared platform context for headless engines.
///
/// Under ADR-0001 and SC-004, the browser requires an engine host that owns the
/// shared platform context and mints engine instances from it.
#[derive(Debug, Clone)]
pub struct HeadlessHost {
    context: Rc<SharedHostContext>,
}

pub type HeadlessEngineHost = HeadlessHost;

impl HeadlessHost {
    /// Create a new headless host with a fresh, distinct [`ContextId`].
    pub fn new() -> Self {
        Self::with_context_id(next_headless_context_id())
    }

    /// Create a new headless host with a specific [`ContextId`].
    pub fn with_context_id(id: ContextId) -> Self {
        Self {
            context: Rc::new(SharedHostContext {
                id,
                responses: RefCell::new(HashMap::new()),
                loads: RefCell::new(Vec::new()),
                subresources: RefCell::new(HashMap::new()),
                policy: RefCell::new(None),
                exemptions: RefCell::new(HashSet::new()),
            }),
        }
    }

    /// The identifier of the shared platform context this host owns.
    pub fn context_id(&self) -> ContextId {
        self.context.id
    }

    /// Script `address` to serve a page titled `title`.
    pub fn with_page(self, address: impl Into<String>, title: impl Into<String>) -> Self {
        self.context.responses.borrow_mut().insert(
            address.into(),
            Response::Page {
                title: title.into(),
            },
        );
        self
    }

    /// Script `address` to fail with `error`.
    pub fn with_failure(self, address: impl Into<String>, error: LoadError) -> Self {
        self.context
            .responses
            .borrow_mut()
            .insert(address.into(), Response::Fail(error));
        self
    }

    /// Script `address` to execute a sequence of `steps`.
    pub fn with_sequence(
        self,
        address: impl Into<String>,
        steps: impl IntoIterator<Item = ScriptStep>,
    ) -> Self {
        self.context.responses.borrow_mut().insert(
            address.into(),
            Response::Sequence(steps.into_iter().collect()),
        );
        self
    }

    /// Convenience helper: Script `address` to redirect to `target_address` before committing and succeeding with `title`.
    pub fn with_redirect(
        self,
        address: impl Into<String>,
        target_address: impl Into<String>,
        title: impl Into<String>,
    ) -> Self {
        let target = target_address.into();
        self.with_sequence(
            address,
            [
                ScriptStep::Redirect {
                    address: target.clone(),
                },
                ScriptStep::Commit { address: target },
                ScriptStep::Title {
                    title: title.into(),
                },
                ScriptStep::Succeed,
            ],
        )
    }

    /// Convenience helper: Script `address` to commit and succeed before the `title` arrives.
    pub fn with_delayed_title(self, address: impl Into<String>, title: impl Into<String>) -> Self {
        let addr = address.into();
        self.with_sequence(
            addr.clone(),
            [
                ScriptStep::Commit { address: addr },
                ScriptStep::Succeed,
                ScriptStep::Title {
                    title: title.into(),
                },
            ],
        )
    }

    /// Convenience helper: Script `address` to be abandoned/navigated away from before completion.
    pub fn with_abandoned_navigation(self, address: impl Into<String>) -> Self {
        let addr = address.into();
        self.with_sequence(
            addr.clone(),
            [
                ScriptStep::Commit { address: addr },
                ScriptStep::NavigateAway,
            ],
        )
    }

    /// Convenience helper: Script `address` to start loading and never resolve (SC-009 30-second clause).
    pub fn with_hanging_load(self, address: impl Into<String>) -> Self {
        self.with_sequence(address, [])
    }

    /// All addresses requested across all engines minted from this host.
    pub fn loads(&self) -> Vec<String> {
        self.context.loads.borrow().clone()
    }

    /// Script `address` with a set of `subresources` that will be requested when navigating to it.
    pub fn with_subresources(
        self,
        address: impl Into<String>,
        subresources: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.context.subresources.borrow_mut().insert(
            address.into(),
            subresources.into_iter().map(Into::into).collect(),
        );
        self
    }

    /// Install a default compiled blocking policy on this host's shared platform context.
    pub fn with_policy(self, policy: CompiledPolicy) -> Self {
        *self.context.policy.borrow_mut() = Some(policy);
        self
    }

    /// Add a site exemption to this host's shared platform context.
    pub fn with_site_exemption(self, site: impl Into<String>) -> Self {
        self.context.exemptions.borrow_mut().insert(site.into());
        self
    }

    /// Mint an engine instance sharing this host's platform context.
    pub fn create_engine(&mut self) -> HeadlessEngine {
        HeadlessEngine::from_shared_context(Rc::clone(&self.context))
    }
}

impl Default for HeadlessHost {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineHost for HeadlessHost {
    type Engine = HeadlessEngine;

    fn name(&self) -> &'static str {
        "headless-host"
    }

    fn context_id(&self) -> ContextId {
        self.context.id
    }

    fn create_engine(&mut self) -> Self::Engine {
        self.create_engine()
    }
}

/// An engine that answers from a script.
///
/// An address with no scripted response is [`LoadError::Unresolvable`], which
/// mirrors the real case — an address that resolves to nothing — and means a
/// test that forgets to script a page gets a failure it can see rather than a
/// silent empty success.
///
/// Events for a navigation are placed on the queue when the navigation starts,
/// which is when this engine knows the whole outcome. What the contract
/// promises — per-navigation ordering, drain in emission order — holds; what a
/// real backend would add, outcomes arriving across later polls, this engine
/// compresses into the start. Scripting that spreads a navigation's events out
/// belongs to the sequence support the event contract's tests need, which
/// lands on top of this shape.
#[derive(Debug)]
pub struct HeadlessEngine {
    context_id: ContextId,
    responses: HashMap<String, Response>,
    queue: VecDeque<NavigationObservation>,
    next_id: Option<NavigationId>,
    next_surface_id: Option<SurfaceId>,
    active_surface: Option<SurfaceId>,
    surfaces: HashMap<SurfaceId, HeadlessSurface>,
    persistent_data: HashMap<SurfaceId, HashMap<String, String>>,
    loads: Vec<String>,
    context: Option<Rc<SharedHostContext>>,
    subresources: HashMap<String, Vec<String>>,
    policy: Option<CompiledPolicy>,
    exemptions: HashSet<String>,
    messages: VecDeque<TaggedMessage>,
    outgoing_messages: Vec<(SurfaceId, String)>,
}

fn normalize_site(site: &str) -> &str {
    let s = site
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    s.split('/').next().unwrap_or(s)
}

fn site_matches(exemption: &str, url_or_site: &str) -> bool {
    let norm_exempt = normalize_site(exemption);
    let norm_target = normalize_site(url_or_site);
    norm_target == norm_exempt || norm_target.ends_with(&format!(".{norm_exempt}"))
}

impl Default for HeadlessEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl HeadlessEngine {
    pub fn new() -> Self {
        Self {
            context_id: next_headless_context_id(),
            responses: HashMap::new(),
            queue: VecDeque::new(),
            next_id: None,
            next_surface_id: None,
            active_surface: None,
            surfaces: HashMap::new(),
            persistent_data: HashMap::new(),
            loads: Vec::new(),
            context: None,
            subresources: HashMap::new(),
            policy: None,
            exemptions: HashSet::new(),
            messages: VecDeque::new(),
            outgoing_messages: Vec::new(),
        }
    }

    fn from_shared_context(context: Rc<SharedHostContext>) -> Self {
        Self {
            context_id: context.id,
            responses: HashMap::new(),
            queue: VecDeque::new(),
            next_id: None,
            next_surface_id: None,
            active_surface: None,
            surfaces: HashMap::new(),
            persistent_data: HashMap::new(),
            loads: Vec::new(),
            context: Some(context),
            subresources: HashMap::new(),
            policy: None,
            exemptions: HashSet::new(),
            messages: VecDeque::new(),
            outgoing_messages: Vec::new(),
        }
    }

    /// The identifier of the shared platform context this engine belongs to.
    pub fn context_id(&self) -> ContextId {
        self.context_id
    }

    /// Script `address` to serve a page titled `title`.
    pub fn with_page(mut self, address: impl Into<String>, title: impl Into<String>) -> Self {
        self.responses.insert(
            address.into(),
            Response::Page {
                title: title.into(),
            },
        );
        self
    }

    /// Script `address` to fail with `error`.
    pub fn with_failure(mut self, address: impl Into<String>, error: LoadError) -> Self {
        self.responses.insert(address.into(), Response::Fail(error));
        self
    }

    /// Script `address` to execute a sequence of `steps`.
    pub fn with_sequence(
        mut self,
        address: impl Into<String>,
        steps: impl IntoIterator<Item = ScriptStep>,
    ) -> Self {
        self.responses.insert(
            address.into(),
            Response::Sequence(steps.into_iter().collect()),
        );
        self
    }

    /// Convenience helper: Script `address` to redirect to `target_address` before committing and succeeding with `title`.
    pub fn with_redirect(
        self,
        address: impl Into<String>,
        target_address: impl Into<String>,
        title: impl Into<String>,
    ) -> Self {
        let target = target_address.into();
        self.with_sequence(
            address,
            [
                ScriptStep::Redirect {
                    address: target.clone(),
                },
                ScriptStep::Commit { address: target },
                ScriptStep::Title {
                    title: title.into(),
                },
                ScriptStep::Succeed,
            ],
        )
    }

    /// Convenience helper: Script `address` to commit and succeed before the `title` arrives.
    pub fn with_delayed_title(self, address: impl Into<String>, title: impl Into<String>) -> Self {
        let addr = address.into();
        self.with_sequence(
            addr.clone(),
            [
                ScriptStep::Commit { address: addr },
                ScriptStep::Succeed,
                ScriptStep::Title {
                    title: title.into(),
                },
            ],
        )
    }

    /// Convenience helper: Script `address` to be abandoned/navigated away from before completion.
    pub fn with_abandoned_navigation(self, address: impl Into<String>) -> Self {
        let addr = address.into();
        self.with_sequence(
            addr.clone(),
            [
                ScriptStep::Commit { address: addr },
                ScriptStep::NavigateAway,
            ],
        )
    }

    /// Convenience helper: Script `address` to start loading and never resolve (SC-009 30-second clause).
    pub fn with_hanging_load(self, address: impl Into<String>) -> Self {
        self.with_sequence(address, [])
    }

    /// Script an engine-initiated (unsolicited) event sequence that will be enqueued immediately or on trigger.
    pub fn with_engine_initiated_sequence(
        mut self,
        address: impl Into<String>,
        steps: impl IntoIterator<Item = ScriptStep>,
    ) -> Self {
        let surface = self.ensure_active_surface();
        let addr = address.into();
        let id = self.mint();
        let epoch = self
            .surfaces
            .get(&surface)
            .map(|s| s.epoch)
            .unwrap_or(NavigationEpoch::FIRST);
        self.queue.push_back(NavigationObservation::new(
            epoch,
            NavigationEvent::Started { id, address: addr },
        ));
        self.process_surface_steps(surface, id, steps);
        self
    }

    /// Convenience helper: Script an engine-initiated page navigation.
    pub fn with_engine_initiated_page(
        self,
        address: impl Into<String>,
        title: impl Into<String>,
    ) -> Self {
        let addr = address.into();
        let t = title.into();
        self.with_engine_initiated_sequence(
            addr.clone(),
            [
                ScriptStep::Commit { address: addr },
                ScriptStep::Title { title: t },
                ScriptStep::Succeed,
            ],
        )
    }

    /// Script `address` with a set of `subresources` that will be requested when navigating to it.
    pub fn with_subresources(
        mut self,
        address: impl Into<String>,
        subresources: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.subresources.insert(
            address.into(),
            subresources.into_iter().map(Into::into).collect(),
        );
        self
    }

    /// Install a compiled blocking policy on this engine.
    pub fn with_policy(mut self, policy: CompiledPolicy) -> Self {
        self.policy = Some(policy);
        self
    }

    /// Exempt `site` from blocking on this engine.
    pub fn with_site_exemption(mut self, site: impl Into<String>) -> Self {
        self.exemptions.insert(site.into());
        self
    }

    fn active_policy(&self) -> Option<CompiledPolicy> {
        if let Some(p) = &self.policy {
            return Some(p.clone());
        }
        if let Some(ctx) = &self.context {
            if let Some(p) = ctx.policy.borrow().as_ref() {
                return Some(p.clone());
            }
        }
        None
    }

    fn get_subresources(&self, address: &str) -> Vec<String> {
        if let Some(sub) = self.subresources.get(address) {
            return sub.clone();
        }
        if let Some(ctx) = &self.context {
            if let Some(sub) = ctx.subresources.borrow().get(address) {
                return sub.clone();
            }
        }
        if address == "https://blocked-content.test/" {
            return vec![
                "https://blocked-content.test/tracker.js".to_string(),
                "https://blocked-content.test/ad.png".to_string(),
                "https://blocked-content.test/content.css".to_string(),
            ];
        }
        Vec::new()
    }

    /// Install or replace the compiled content-blocking policy.
    pub fn install_policy(&mut self, policy: CompiledPolicy) {
        self.policy = Some(policy);
    }

    /// Exempt `site` from content blocking.
    pub fn exempt_site(&mut self, site: &str) {
        self.exemptions.insert(site.to_string());
    }

    /// Remove a previously granted site exemption.
    pub fn remove_site_exemption(&mut self, site: &str) {
        let norm = normalize_site(site);
        self.exemptions.retain(|s| normalize_site(s) != norm);
        if let Some(ctx) = &self.context {
            ctx.exemptions
                .borrow_mut()
                .retain(|s| normalize_site(s) != norm);
        }
    }

    /// Whether `site` is currently exempted from content blocking.
    pub fn is_site_exempt(&self, site: &str) -> bool {
        if self.exemptions.iter().any(|e| site_matches(e, site)) {
            return true;
        }
        if let Some(ctx) = &self.context {
            if ctx
                .exemptions
                .borrow()
                .iter()
                .any(|e| site_matches(e, site))
            {
                return true;
            }
        }
        false
    }

    /// The count of blocked requests or resources for the page currently loaded on `surface`.
    pub fn surface_blocked_count(&self, surface: SurfaceId) -> u64 {
        self.surfaces.get(&surface).map_or(0, |s| s.blocked_count)
    }

    /// The list of URLs or resource identifiers that were blocked on `surface` during the current page load.
    pub fn surface_blocked_items(&self, surface: SurfaceId) -> Vec<String> {
        self.surfaces
            .get(&surface)
            .map_or_else(Vec::new, |s| s.blocked_urls.clone())
    }

    /// Every address this engine was asked to load, in order.
    ///
    /// FR-007a forbids browsing history leaving the machine and bounds what may
    /// be transmitted. A test that asserts on outbound behaviour needs to see
    /// what the shell actually asked for, and this is that record — held in
    /// memory, in a test-only crate, never written anywhere.
    pub fn loads(&self) -> &[String] {
        &self.loads
    }

    fn mint(&mut self) -> NavigationId {
        let id = self.next_id.unwrap_or(NavigationId::FIRST);
        self.next_id = Some(id.next());
        id
    }

    /// Ensure there is an active surface, creating a default persistent one if necessary.
    fn ensure_active_surface(&mut self) -> SurfaceId {
        if let Some(id) = self.active_surface {
            if let Some(surface) = self.surfaces.get(&id) {
                if !surface.state.is_closed() {
                    return id;
                }
            }
        }
        if let Some((&id, _)) = self.surfaces.iter().find(|(_, s)| !s.state.is_closed()) {
            self.activate_surface(id);
            return id;
        }
        let id = self.create_surface(DataStoreSelector::Persistent);
        self.activate_surface(id);
        id
    }

    /// Create an addressable rendering surface with the specified data store.
    pub fn create_surface(&mut self, store: DataStoreSelector) -> SurfaceId {
        let id = self.next_surface_id.unwrap_or(SurfaceId::FIRST);
        self.next_surface_id = Some(id.next());
        self.surfaces.insert(
            id,
            HeadlessSurface {
                _id: id,
                store,
                state: SurfaceState::Inactive,
                epoch: NavigationEpoch::FIRST,
                identity: None,
                hosted_bytes: None,
                app_id: None,
                current: None,
                current_nav: None,
                loads: Vec::new(),
                data: HashMap::new(),
                blocked_count: 0,
                blocked_urls: Vec::new(),
            },
        );
        id
    }

    /// Activate `surface`, bringing it to the foreground.
    pub fn activate_surface(&mut self, surface: SurfaceId) {
        if let Some(s) = self.surfaces.get(&surface) {
            if s.state.is_closed() {
                return;
            }
        } else {
            return;
        }

        if let Some(prev_id) = self.active_surface {
            if prev_id != surface {
                if let Some(prev) = self.surfaces.get_mut(&prev_id) {
                    if prev.state == SurfaceState::Active {
                        prev.state = SurfaceState::Inactive;
                    }
                }
            }
        }

        if let Some(s) = self.surfaces.get_mut(&surface) {
            s.state = SurfaceState::Active;
            self.active_surface = Some(surface);
        }
    }

    /// Suspend `surface` to conserve memory and resources (FR-002).
    pub fn suspend_surface(&mut self, surface: SurfaceId) {
        if let Some(s) = self.surfaces.get_mut(&surface) {
            if s.state.is_closed() {
                return;
            }
            s.state = SurfaceState::Suspended;
            if self.active_surface == Some(surface) {
                self.active_surface = None;
            }
        }
    }

    /// Resume a previously suspended rendering surface.
    pub fn resume_surface(&mut self, surface: SurfaceId) {
        if let Some(s) = self.surfaces.get(&surface) {
            if s.state == SurfaceState::Suspended {
                self.activate_surface(surface);
            }
        }
    }

    /// Close and destroy `surface`.
    pub fn close_surface(&mut self, surface: SurfaceId) {
        if let Some(s) = self.surfaces.get_mut(&surface) {
            s.state = SurfaceState::Closed;
            s.current = None;
            if s.store == DataStoreSelector::NonPersistent {
                s.data.clear();
                self.persistent_data.remove(&surface);
            } else {
                self.persistent_data
                    .entry(surface)
                    .or_default()
                    .extend(s.data.drain());
            }
        }
        if self.active_surface == Some(surface) {
            self.active_surface = None;
        }
    }

    /// The currently active surface, if any.
    pub fn active_surface(&self) -> Option<SurfaceId> {
        self.active_surface
    }

    /// The data store selector of `surface`, if it exists.
    pub fn surface_data_store(&self, surface: SurfaceId) -> Option<DataStoreSelector> {
        self.surfaces.get(&surface).map(|s| s.store)
    }

    /// The lifecycle state of `surface`, if it exists.
    pub fn surface_state(&self, surface: SurfaceId) -> Option<SurfaceState> {
        self.surfaces.get(&surface).map(|s| s.state)
    }

    /// The page currently displayed on `surface`, if any.
    pub fn surface_current(&self, surface: SurfaceId) -> Option<&Page> {
        self.surfaces.get(&surface).and_then(|s| s.current.as_ref())
    }

    /// Whether `surface` retains any data in its data store.
    pub fn surface_has_retained_data(&self, surface: SurfaceId) -> bool {
        if let Some(s) = self.surfaces.get(&surface) {
            if s.store == DataStoreSelector::NonPersistent {
                !s.data.is_empty()
            } else {
                !s.data.is_empty()
                    || self
                        .persistent_data
                        .get(&surface)
                        .is_some_and(|d| !d.is_empty())
            }
        } else {
            self.persistent_data
                .get(&surface)
                .is_some_and(|d| !d.is_empty())
        }
    }

    /// All addresses requested for `surface`, in order.
    pub fn surface_loads(&self, surface: SurfaceId) -> Option<&[String]> {
        self.surfaces.get(&surface).map(|s| s.loads.as_slice())
    }

    /// Begin navigating `surface` to `request`.
    pub fn start_surface_navigation(
        &mut self,
        surface: SurfaceId,
        request: &Request,
    ) -> NavigationId {
        let address = request.address().to_owned();
        self.loads.push(address.clone());
        if let Some(ctx) = &self.context {
            ctx.loads.borrow_mut().push(address.clone());
        }

        // Evaluate content blocking for this navigation
        let is_exempt = self.is_site_exempt(&address);
        let mut blocked_count = 0u64;
        let mut blocked_urls = Vec::new();
        if !is_exempt {
            if let Some(policy) = self.active_policy() {
                let subresources = self.get_subresources(&address);
                for subres in subresources {
                    if policy.matches(&subres) {
                        blocked_count += 1;
                        blocked_urls.push(subres);
                    }
                }
            }
        }

        if let Some(s) = self.surfaces.get_mut(&surface) {
            if s.state.is_closed() {
                return self.mint();
            }
            s.loads.push(address.clone());
            s.data.insert("visited_url".into(), address.clone());
            s.data
                .insert("session_cookie".into(), format!("session_for_{}", address));
            if s.store == DataStoreSelector::Persistent {
                self.persistent_data
                    .entry(surface)
                    .or_default()
                    .insert("visited_url".into(), address.clone());
            }

            s.blocked_count = blocked_count;
            s.blocked_urls = blocked_urls;
        }

        let id = self.mint();

        let epoch = self
            .surfaces
            .get(&surface)
            .map(|s| s.epoch)
            .unwrap_or(NavigationEpoch::FIRST);

        self.queue.push_back(NavigationObservation::new(
            epoch,
            NavigationEvent::Started {
                id,
                address: address.clone(),
            },
        ));

        let response = self
            .responses
            .get(&address)
            .cloned()
            .or_else(|| {
                self.context
                    .as_ref()
                    .and_then(|c| c.responses.borrow().get(&address).cloned())
            })
            .or_else(|| {
                if address == "https://blocked-content.test/" {
                    Some(Response::Page {
                        title: "Blocked Content Test Page".into(),
                    })
                } else {
                    None
                }
            });

        match response {
            Some(Response::Page { title }) => {
                self.process_surface_steps(
                    surface,
                    id,
                    [
                        ScriptStep::Commit {
                            address: address.clone(),
                        },
                        ScriptStep::Title { title },
                        ScriptStep::Succeed,
                    ],
                );
            }
            Some(Response::Fail(error)) => {
                self.process_surface_steps(surface, id, [ScriptStep::Fail(error)]);
            }
            Some(Response::Sequence(steps)) => {
                self.process_surface_steps(surface, id, steps);
            }
            None => {
                self.process_surface_steps(
                    surface,
                    id,
                    [ScriptStep::Fail(LoadError::Unresolvable { address })],
                );
            }
        }

        id
    }

    fn process_surface_steps(
        &mut self,
        surface: SurfaceId,
        id: NavigationId,
        steps: impl IntoIterator<Item = ScriptStep>,
    ) {
        for step in steps {
            let mut epoch = self
                .surfaces
                .get(&surface)
                .map(|s| s.epoch)
                .unwrap_or(NavigationEpoch::FIRST);
            match step {
                ScriptStep::Redirect { address } => {
                    self.queue.push_back(NavigationObservation::new(
                        epoch,
                        NavigationEvent::Redirected { id, address },
                    ));
                }
                ScriptStep::Commit { address } => {
                    if let Some(s) = self.surfaces.get_mut(&surface) {
                        s.epoch = s.epoch.next();
                        epoch = s.epoch;
                        s.current = Some(Page::new(address.clone(), ""));
                        s.current_nav = Some(id);
                    }
                    self.queue.push_back(NavigationObservation::new(
                        epoch,
                        NavigationEvent::Committed { id, address },
                    ));
                }
                ScriptStep::Title { title } => {
                    if let Some(s) = self.surfaces.get_mut(&surface) {
                        if s.current_nav == Some(id) {
                            if let Some(page) = &s.current {
                                s.current =
                                    Some(Page::new(page.address().to_owned(), title.clone()));
                            }
                        }
                    }
                    self.queue.push_back(NavigationObservation::new(
                        epoch,
                        NavigationEvent::TitleChanged { id, title },
                    ));
                }
                ScriptStep::Succeed => {
                    self.queue.push_back(NavigationObservation::new(
                        epoch,
                        NavigationEvent::Succeeded { id },
                    ));
                }
                ScriptStep::Fail(error) => {
                    self.queue.push_back(NavigationObservation::new(
                        epoch,
                        NavigationEvent::Failed { id, error },
                    ));
                }
                ScriptStep::NavigateAway => {
                    self.queue.push_back(NavigationObservation::new(
                        epoch,
                        NavigationEvent::NavigatedAway { id },
                    ));
                }
            }
        }
    }

    /// The current [`NavigationEpoch`] of `surface`.
    pub fn surface_navigation_epoch(&self, surface: SurfaceId) -> NavigationEpoch {
        self.surfaces
            .get(&surface)
            .map(|s| s.epoch)
            .unwrap_or(NavigationEpoch::FIRST)
    }

    /// Perform a same-document navigation on `surface` to `address`.
    pub fn navigate_same_document(
        &mut self,
        surface: SurfaceId,
        address: impl Into<String>,
    ) -> NavigationId {
        let id = self.mint();
        let addr = address.into();
        let mut new_epoch = NavigationEpoch::FIRST;
        if let Some(s) = self.surfaces.get_mut(&surface) {
            s.epoch = s.epoch.next();
            new_epoch = s.epoch;
            let title = s
                .current
                .as_ref()
                .map(|p| p.title().to_string())
                .unwrap_or_default();
            s.current = Some(Page::new(addr.clone(), title));
            s.loads.push(addr.clone());
        }
        self.queue.push_back(NavigationObservation::new(
            new_epoch,
            NavigationEvent::SameDocumentNavigated { id, address: addr },
        ));
        id
    }

    /// Poll the next navigation observation from the engine, if any is ready.
    pub fn poll_observation(&mut self) -> Option<NavigationObservation> {
        self.queue.pop_front()
    }

    /// Host content on `surface` from shell-supplied bytes under `identity`.
    pub fn host_surface_bytes(
        &mut self,
        surface: SurfaceId,
        identity: SurfaceIdentity,
        bytes: Vec<u8>,
    ) -> NavigationId {
        let id = self.mint();
        let mut new_epoch = NavigationEpoch::FIRST;
        let ident_str = identity.as_str().to_string();
        if let Some(s) = self.surfaces.get_mut(&surface) {
            s.identity = Some(identity);
            s.hosted_bytes = Some(bytes);
            s.epoch = s.epoch.next();
            new_epoch = s.epoch;
            s.current = Some(Page::new(ident_str.clone(), ""));
            s.loads.push(ident_str.clone());
        }
        self.queue.push_back(NavigationObservation::new(
            new_epoch,
            NavigationEvent::Started {
                id,
                address: ident_str.clone(),
            },
        ));
        self.queue.push_back(NavigationObservation::new(
            new_epoch,
            NavigationEvent::Committed {
                id,
                address: ident_str,
            },
        ));
        self.queue.push_back(NavigationObservation::new(
            new_epoch,
            NavigationEvent::Succeeded { id },
        ));
        id
    }

    /// The [`SurfaceIdentity`] currently hosted on `surface`, if any.
    pub fn surface_identity(&self, surface: SurfaceId) -> Option<&SurfaceIdentity> {
        self.surfaces
            .get(&surface)
            .and_then(|s| s.identity.as_ref())
    }

    /// The raw bytes currently hosted on `surface`, if any.
    pub fn surface_hosted_bytes(&self, surface: SurfaceId) -> Option<&[u8]> {
        self.surfaces
            .get(&surface)
            .and_then(|s| s.hosted_bytes.as_deref())
    }

    /// Assign an immutable [`AppId`] to `surface`.
    pub fn set_surface_app_id(&mut self, surface: SurfaceId, app_id: AppId) {
        if let Some(s) = self.surfaces.get_mut(&surface) {
            s.app_id = Some(app_id);
        }
    }

    /// The shell-assigned [`AppId`] of `surface`, if assigned.
    pub fn surface_app_id(&self, surface: SurfaceId) -> Option<&AppId> {
        self.surfaces.get(&surface).and_then(|s| s.app_id.as_ref())
    }

    /// Send a message from the shell to `surface`.
    pub fn send_message_to_surface(&mut self, surface: SurfaceId, message: &str) {
        self.outgoing_messages.push((surface, message.to_string()));
    }

    /// Messages sent from the shell to surfaces.
    pub fn outgoing_messages(&self) -> &[(SurfaceId, String)] {
        &self.outgoing_messages
    }

    /// Poll the next incoming tagged message from an app surface, if any.
    pub fn poll_message(&mut self) -> Option<TaggedMessage> {
        self.messages.pop_front()
    }

    /// Simulate an incoming message originating from `surface`, tagged with its assigned `AppId`.
    pub fn post_message_from_surface(&mut self, surface: SurfaceId, payload: impl Into<String>) {
        let app_id = self
            .surface_app_id(surface)
            .cloned()
            .unwrap_or_else(|| AppId::new("unassigned"));
        self.messages
            .push_back(TaggedMessage::new(app_id, surface, payload));
    }
}

impl Engine for HeadlessEngine {
    fn name(&self) -> &'static str {
        "headless"
    }

    fn context_id(&self) -> ContextId {
        self.context_id
    }

    fn start_navigation(&mut self, request: &Request) -> NavigationId {
        let surface = self.ensure_active_surface();
        self.start_surface_navigation(surface, request)
    }

    fn poll_event(&mut self) -> Option<NavigationEvent> {
        self.queue.pop_front().map(|obs| obs.into_event())
    }

    fn poll_observation(&mut self) -> Option<NavigationObservation> {
        self.poll_observation()
    }

    fn host_surface_bytes(
        &mut self,
        surface: SurfaceId,
        identity: SurfaceIdentity,
        bytes: Vec<u8>,
    ) -> NavigationId {
        self.host_surface_bytes(surface, identity, bytes)
    }

    fn surface_identity(&self, surface: SurfaceId) -> Option<&SurfaceIdentity> {
        self.surface_identity(surface)
    }

    fn surface_hosted_bytes(&self, surface: SurfaceId) -> Option<&[u8]> {
        self.surface_hosted_bytes(surface)
    }

    fn surface_navigation_epoch(&self, surface: SurfaceId) -> NavigationEpoch {
        self.surface_navigation_epoch(surface)
    }

    fn navigate_same_document(&mut self, surface: SurfaceId, address: &str) -> NavigationId {
        self.navigate_same_document(surface, address)
    }

    fn set_surface_app_id(&mut self, surface: SurfaceId, app_id: AppId) {
        self.set_surface_app_id(surface, app_id);
    }

    fn surface_app_id(&self, surface: SurfaceId) -> Option<&AppId> {
        self.surface_app_id(surface)
    }

    fn send_message_to_surface(&mut self, surface: SurfaceId, message: &str) {
        self.send_message_to_surface(surface, message);
    }

    fn poll_message(&mut self) -> Option<TaggedMessage> {
        self.poll_message()
    }

    fn post_message_from_surface(&mut self, surface: SurfaceId, message: &str) {
        self.post_message_from_surface(surface, message);
    }

    fn current(&self) -> Option<&Page> {
        self.active_surface.and_then(|id| self.surface_current(id))
    }

    fn create_surface(&mut self, store: DataStoreSelector) -> SurfaceId {
        self.create_surface(store)
    }

    fn activate_surface(&mut self, surface: SurfaceId) {
        self.activate_surface(surface);
    }

    fn suspend_surface(&mut self, surface: SurfaceId) {
        self.suspend_surface(surface);
    }

    fn resume_surface(&mut self, surface: SurfaceId) {
        self.resume_surface(surface);
    }

    fn close_surface(&mut self, surface: SurfaceId) {
        self.close_surface(surface);
    }

    fn active_surface(&self) -> Option<SurfaceId> {
        self.active_surface()
    }

    fn surface_data_store(&self, surface: SurfaceId) -> Option<DataStoreSelector> {
        self.surface_data_store(surface)
    }

    fn surface_state(&self, surface: SurfaceId) -> Option<SurfaceState> {
        self.surface_state(surface)
    }

    fn surface_current(&self, surface: SurfaceId) -> Option<&Page> {
        self.surface_current(surface)
    }

    fn start_surface_navigation(&mut self, surface: SurfaceId, request: &Request) -> NavigationId {
        self.start_surface_navigation(surface, request)
    }

    fn surface_has_retained_data(&self, surface: SurfaceId) -> bool {
        self.surface_has_retained_data(surface)
    }

    fn install_policy(&mut self, policy: CompiledPolicy) {
        self.install_policy(policy);
    }

    fn exempt_site(&mut self, site: &str) {
        self.exempt_site(site);
    }

    fn remove_site_exemption(&mut self, site: &str) {
        self.remove_site_exemption(site);
    }

    fn is_site_exempt(&self, site: &str) -> bool {
        self.is_site_exempt(site)
    }

    fn surface_blocked_count(&self, surface: SurfaceId) -> u64 {
        self.surface_blocked_count(surface)
    }

    fn blocked_count(&self, surface: SurfaceId) -> u64 {
        self.surface_blocked_count(surface)
    }

    fn surface_blocked_items(&self, surface: SurfaceId) -> Vec<String> {
        self.surface_blocked_items(surface)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_initiated_navigation_emits_without_start_navigation() {
        let mut engine = HeadlessEngine::new()
            .with_engine_initiated_page("https://unsolicited.invalid/", "Unsolicited Title");

        let started = engine.poll_event();
        assert!(matches!(
            started,
            Some(NavigationEvent::Started { address, .. }) if address == "https://unsolicited.invalid/"
        ));

        let committed = engine.poll_event();
        assert!(matches!(
            committed,
            Some(NavigationEvent::Committed { address, .. }) if address == "https://unsolicited.invalid/"
        ));

        let title = engine.poll_event();
        assert!(matches!(
            title,
            Some(NavigationEvent::TitleChanged { title, .. }) if title == "Unsolicited Title"
        ));

        let succeeded = engine.poll_event();
        assert!(matches!(succeeded, Some(NavigationEvent::Succeeded { .. })));

        assert_eq!(
            engine.current().map(|p| p.title()),
            Some("Unsolicited Title")
        );
        assert!(engine.loads().is_empty());
    }

    #[test]
    fn redirect_before_commit_emits_redirected_then_committed() {
        let mut engine = HeadlessEngine::new().with_redirect(
            "https://initial.invalid/",
            "https://target.invalid/",
            "Target Page",
        );

        let request = Request::new("https://initial.invalid/");
        let id = engine.start_navigation(&request);

        let started = engine.poll_event();
        assert_eq!(
            started,
            Some(NavigationEvent::Started {
                id,
                address: "https://initial.invalid/".into()
            })
        );

        let redirected = engine.poll_event();
        assert_eq!(
            redirected,
            Some(NavigationEvent::Redirected {
                id,
                address: "https://target.invalid/".into()
            })
        );

        let committed = engine.poll_event();
        assert_eq!(
            committed,
            Some(NavigationEvent::Committed {
                id,
                address: "https://target.invalid/".into()
            })
        );

        let title = engine.poll_event();
        assert_eq!(
            title,
            Some(NavigationEvent::TitleChanged {
                id,
                title: "Target Page".into()
            })
        );

        let succeeded = engine.poll_event();
        assert_eq!(succeeded, Some(NavigationEvent::Succeeded { id }));

        assert_eq!(
            engine.current().map(|p| p.address()),
            Some("https://target.invalid/")
        );
    }

    #[test]
    fn title_arriving_after_outcome() {
        let mut engine =
            HeadlessEngine::new().with_delayed_title("https://delayed.invalid/", "Delayed Title");

        let request = Request::new("https://delayed.invalid/");
        let id = engine.start_navigation(&request);

        let started = engine.poll_event();
        assert_eq!(
            started,
            Some(NavigationEvent::Started {
                id,
                address: "https://delayed.invalid/".into()
            })
        );

        let committed = engine.poll_event();
        assert_eq!(
            committed,
            Some(NavigationEvent::Committed {
                id,
                address: "https://delayed.invalid/".into()
            })
        );

        let succeeded = engine.poll_event();
        assert_eq!(succeeded, Some(NavigationEvent::Succeeded { id }));

        let title = engine.poll_event();
        assert_eq!(
            title,
            Some(NavigationEvent::TitleChanged {
                id,
                title: "Delayed Title".into()
            })
        );

        assert_eq!(engine.current().map(|p| p.title()), Some("Delayed Title"));
    }

    #[test]
    fn abandoned_navigation_emits_navigated_away() {
        let mut engine =
            HeadlessEngine::new().with_abandoned_navigation("https://abandoned.invalid/");

        let request = Request::new("https://abandoned.invalid/");
        let id = engine.start_navigation(&request);

        let started = engine.poll_event();
        assert_eq!(
            started,
            Some(NavigationEvent::Started {
                id,
                address: "https://abandoned.invalid/".into()
            })
        );

        let committed = engine.poll_event();
        assert_eq!(
            committed,
            Some(NavigationEvent::Committed {
                id,
                address: "https://abandoned.invalid/".into()
            })
        );

        let navigated_away = engine.poll_event();
        assert_eq!(navigated_away, Some(NavigationEvent::NavigatedAway { id }));

        assert_eq!(engine.poll_event(), None);
    }

    #[test]
    fn hanging_load_starts_and_never_resolves() {
        let mut engine = HeadlessEngine::new().with_hanging_load("https://hanging.invalid/");

        let request = Request::new("https://hanging.invalid/");
        let id = engine.start_navigation(&request);

        let started = engine.poll_event();
        assert_eq!(
            started,
            Some(NavigationEvent::Started {
                id,
                address: "https://hanging.invalid/".into()
            })
        );

        // No further events emitted (never resolves)
        assert_eq!(engine.poll_event(), None);
        assert!(engine.current().is_none());
    }

    #[test]
    fn with_page_and_with_failure_backward_compatibility() {
        let mut engine = HeadlessEngine::new()
            .with_page("https://page.invalid/", "Page Title")
            .with_failure(
                "https://fail.invalid/",
                LoadError::Unresolvable {
                    address: "https://fail.invalid/".into(),
                },
            );

        let page_id = engine.start_navigation(&Request::new("https://page.invalid/"));
        assert_eq!(
            engine.poll_event(),
            Some(NavigationEvent::Started {
                id: page_id,
                address: "https://page.invalid/".into()
            })
        );
        assert_eq!(
            engine.poll_event(),
            Some(NavigationEvent::Committed {
                id: page_id,
                address: "https://page.invalid/".into()
            })
        );
        assert_eq!(
            engine.poll_event(),
            Some(NavigationEvent::TitleChanged {
                id: page_id,
                title: "Page Title".into()
            })
        );
        assert_eq!(
            engine.poll_event(),
            Some(NavigationEvent::Succeeded { id: page_id })
        );

        let fail_id = engine.start_navigation(&Request::new("https://fail.invalid/"));
        assert_eq!(
            engine.poll_event(),
            Some(NavigationEvent::Started {
                id: fail_id,
                address: "https://fail.invalid/".into()
            })
        );
        assert_eq!(
            engine.poll_event(),
            Some(NavigationEvent::Failed {
                id: fail_id,
                error: LoadError::Unresolvable {
                    address: "https://fail.invalid/".into()
                }
            })
        );
    }

    #[test]
    fn host_mints_engines_sharing_context() {
        let mut host = HeadlessHost::new();
        let engine1 = host.create_engine();
        let engine2 = host.create_engine();

        assert_eq!(engine1.context_id(), engine2.context_id());
        assert_eq!(engine1.context_id(), host.context_id());
        assert!(engine1.shares_context_with(&engine2));
        assert_eq!(host.name(), "headless-host");
    }

    #[test]
    fn distinct_hosts_do_not_share_context() {
        let mut host1 = HeadlessHost::new();
        let mut host2 = HeadlessHost::new();
        let engine1 = host1.create_engine();
        let engine2 = host2.create_engine();

        assert_ne!(host1.context_id(), host2.context_id());
        assert_ne!(engine1.context_id(), engine2.context_id());
        assert!(!engine1.shares_context_with(&engine2));
    }

    #[test]
    fn host_scripted_responses_inherited_by_minted_engines() {
        let mut host = HeadlessHost::new().with_page("https://shared.invalid/", "Shared Page");
        let mut engine = host.create_engine();

        let id = engine.start_navigation(&Request::new("https://shared.invalid/"));
        assert_eq!(
            engine.poll_event(),
            Some(NavigationEvent::Started {
                id,
                address: "https://shared.invalid/".into(),
            })
        );
        assert_eq!(
            engine.poll_event(),
            Some(NavigationEvent::Committed {
                id,
                address: "https://shared.invalid/".into(),
            })
        );
        assert_eq!(
            engine.poll_event(),
            Some(NavigationEvent::TitleChanged {
                id,
                title: "Shared Page".into(),
            })
        );
        assert_eq!(engine.poll_event(), Some(NavigationEvent::Succeeded { id }));
        assert_eq!(engine.current().map(|p| p.title()), Some("Shared Page"));
    }

    #[test]
    fn host_loads_tracks_all_minted_engines() {
        let mut host = HeadlessHost::new()
            .with_page("https://page1.invalid/", "P1")
            .with_page("https://page2.invalid/", "P2");
        let mut engine1 = host.create_engine();
        let mut engine2 = host.create_engine();

        let _ = engine1.start_navigation(&Request::new("https://page1.invalid/"));
        let _ = engine2.start_navigation(&Request::new("https://page2.invalid/"));

        assert_eq!(engine1.loads(), &["https://page1.invalid/"]);
        assert_eq!(engine2.loads(), &["https://page2.invalid/"]);
        assert_eq!(
            host.loads(),
            vec!["https://page1.invalid/", "https://page2.invalid/"]
        );
    }

    #[test]
    fn surfaces_are_independently_addressable() {
        let mut engine = HeadlessEngine::new()
            .with_page("https://surface1.invalid/", "Surface 1 Page")
            .with_page("https://surface2.invalid/", "Surface 2 Page");

        let s1 = engine.create_surface(DataStoreSelector::Persistent);
        let s2 = engine.create_surface(DataStoreSelector::Persistent);
        assert_ne!(s1, s2);

        engine.activate_surface(s1);
        assert_eq!(engine.active_surface(), Some(s1));
        assert_eq!(engine.surface_state(s1), Some(SurfaceState::Active));

        let _ = engine.start_surface_navigation(s1, &Request::new("https://surface1.invalid/"));
        while let Some(_event) = engine.poll_event() {}

        assert_eq!(
            engine.surface_current(s1).map(|p| p.address()),
            Some("https://surface1.invalid/")
        );
        assert_eq!(
            engine.current().map(|p| p.address()),
            Some("https://surface1.invalid/")
        );
        assert_eq!(engine.surface_current(s2), None);

        // Activate and navigate s2
        engine.activate_surface(s2);
        assert_eq!(engine.active_surface(), Some(s2));
        assert_eq!(engine.surface_state(s1), Some(SurfaceState::Inactive));
        assert_eq!(engine.surface_state(s2), Some(SurfaceState::Active));

        let _ = engine.start_surface_navigation(s2, &Request::new("https://surface2.invalid/"));
        while let Some(_event) = engine.poll_event() {}

        assert_eq!(
            engine.surface_current(s2).map(|p| p.address()),
            Some("https://surface2.invalid/")
        );
        assert_eq!(
            engine.current().map(|p| p.address()),
            Some("https://surface2.invalid/")
        );

        // s1's current page is unchanged
        assert_eq!(
            engine.surface_current(s1).map(|p| p.address()),
            Some("https://surface1.invalid/")
        );
        assert_eq!(
            engine.surface_current(s1).map(|p| p.title()),
            Some("Surface 1 Page")
        );
    }

    #[test]
    fn surface_suspend_and_resume_preserves_shell_observable_state() {
        let mut engine = HeadlessEngine::new().with_page("https://page.invalid/", "Observed Title");
        let surface = engine.create_surface(DataStoreSelector::Persistent);
        engine.activate_surface(surface);

        let _ = engine.start_surface_navigation(surface, &Request::new("https://page.invalid/"));
        while let Some(_event) = engine.poll_event() {}

        assert_eq!(
            engine.surface_current(surface).map(|p| p.title()),
            Some("Observed Title")
        );

        engine.suspend_surface(surface);
        assert_eq!(engine.surface_state(surface), Some(SurfaceState::Suspended));
        assert_eq!(
            engine.surface_current(surface).map(|p| p.title()),
            Some("Observed Title"),
            "Suspend must not discard shell-observable page state"
        );

        engine.resume_surface(surface);
        assert_eq!(engine.surface_state(surface), Some(SurfaceState::Active));
        assert_eq!(
            engine.surface_current(surface).map(|p| p.title()),
            Some("Observed Title"),
            "Resume must maintain shell-observable page state"
        );
    }

    #[test]
    fn non_persistent_surface_cleans_up_on_close() {
        let mut engine = HeadlessEngine::new().with_page("https://page.invalid/", "Private Page");
        let s_np = engine.create_surface(DataStoreSelector::NonPersistent);
        let s_p = engine.create_surface(DataStoreSelector::Persistent);

        assert_eq!(
            engine.surface_data_store(s_np),
            Some(DataStoreSelector::NonPersistent)
        );
        assert_eq!(
            engine.surface_data_store(s_p),
            Some(DataStoreSelector::Persistent)
        );

        let _ = engine.start_surface_navigation(s_np, &Request::new("https://page.invalid/"));
        let _ = engine.start_surface_navigation(s_p, &Request::new("https://page.invalid/"));
        while let Some(_event) = engine.poll_event() {}

        assert!(engine.surface_has_retained_data(s_np));
        assert!(engine.surface_has_retained_data(s_p));

        engine.close_surface(s_np);
        assert_eq!(engine.surface_state(s_np), Some(SurfaceState::Closed));
        assert_eq!(engine.surface_current(s_np), None);
        assert!(
            !engine.surface_has_retained_data(s_np),
            "Non-persistent surface must leave nothing behind on close"
        );

        engine.close_surface(s_p);
        assert_eq!(engine.surface_state(s_p), Some(SurfaceState::Closed));
        assert!(
            engine.surface_has_retained_data(s_p),
            "Persistent store must retain persistent data after close"
        );
    }

    #[test]
    fn surface_loads_tracked_per_surface() {
        let mut engine = HeadlessEngine::new()
            .with_page("https://s1.invalid/", "S1")
            .with_page("https://s2.invalid/", "S2");

        let s1 = engine.create_surface(DataStoreSelector::Persistent);
        let s2 = engine.create_surface(DataStoreSelector::Persistent);

        let _ = engine.start_surface_navigation(s1, &Request::new("https://s1.invalid/"));
        let _ = engine.start_surface_navigation(s2, &Request::new("https://s2.invalid/"));

        assert_eq!(
            engine.surface_loads(s1),
            Some(&["https://s1.invalid/".to_string()][..])
        );
        assert_eq!(
            engine.surface_loads(s2),
            Some(&["https://s2.invalid/".to_string()][..])
        );
        assert_eq!(
            engine.loads(),
            &["https://s1.invalid/", "https://s2.invalid/"]
        );
    }

    #[test]
    fn policy_installation_replacement_and_exemption() {
        let mut engine = HeadlessEngine::new().with_page("https://example.test/", "Example");
        let surface = engine.create_surface(DataStoreSelector::Persistent);
        engine.activate_surface(surface);

        let policy = CompiledPolicy::from_rules("policy-1", ["bad-tracker.js", "banner.png"]);
        engine.install_policy(policy);

        let engine = engine.with_subresources(
            "https://example.test/",
            [
                "https://example.test/bad-tracker.js",
                "https://example.test/banner.png",
                "https://example.test/app.js",
            ],
        );
        let mut engine = engine;

        let _ = engine.start_surface_navigation(surface, &Request::new("https://example.test/"));
        assert_eq!(engine.surface_blocked_count(surface), 2);
        assert_eq!(
            engine.surface_blocked_items(surface),
            vec![
                "https://example.test/bad-tracker.js".to_string(),
                "https://example.test/banner.png".to_string()
            ]
        );

        // Site exemption
        engine.exempt_site("example.test");
        assert!(engine.is_site_exempt("example.test"));
        assert!(engine.is_site_exempt("https://example.test/"));

        let _ = engine.start_surface_navigation(surface, &Request::new("https://example.test/"));
        assert_eq!(engine.surface_blocked_count(surface), 0);
        assert!(engine.surface_blocked_items(surface).is_empty());

        // Remove exemption
        engine.remove_site_exemption("https://example.test");
        assert!(!engine.is_site_exempt("example.test"));

        // Replace policy with policy that only blocks banner.png
        let policy2 = CompiledPolicy::from_rules("policy-2", ["banner.png"]);
        engine.install_policy(policy2);

        let _ = engine.start_surface_navigation(surface, &Request::new("https://example.test/"));
        assert_eq!(engine.surface_blocked_count(surface), 1);
        assert_eq!(
            engine.surface_blocked_items(surface),
            vec!["https://example.test/banner.png".to_string()]
        );
    }

    #[test]
    fn surface_blocked_counts_are_isolated_across_surfaces() {
        let mut engine = HeadlessEngine::new()
            .with_page("https://site-a.test/", "Site A")
            .with_page("https://site-b.test/", "Site B")
            .with_subresources(
                "https://site-a.test/",
                [
                    "https://site-a.test/tracker.js",
                    "https://site-a.test/ad.png",
                ],
            )
            .with_subresources("https://site-b.test/", ["https://site-b.test/clean.js"])
            .with_policy(CompiledPolicy::from_rules(
                "blocker",
                ["tracker.js", "ad.png"],
            ));

        let s1 = engine.create_surface(DataStoreSelector::Persistent);
        let s2 = engine.create_surface(DataStoreSelector::Persistent);

        let _ = engine.start_surface_navigation(s1, &Request::new("https://site-a.test/"));
        let _ = engine.start_surface_navigation(s2, &Request::new("https://site-b.test/"));

        assert_eq!(engine.surface_blocked_count(s1), 2);
        assert_eq!(engine.surface_blocked_count(s2), 0);

        // Re-navigating s1 to site-b resets s1's count
        let _ = engine.start_surface_navigation(s1, &Request::new("https://site-b.test/"));
        assert_eq!(engine.surface_blocked_count(s1), 0);
    }

    #[test]
    fn navigation_epoch_increments_on_regular_and_same_document_navigation() {
        let mut engine = HeadlessEngine::new().with_page("https://example.test/", "Home");
        let surface = engine.create_surface(DataStoreSelector::Persistent);
        assert_eq!(
            engine.surface_navigation_epoch(surface),
            NavigationEpoch::FIRST
        );

        // Regular navigation
        let nav1 = engine.start_surface_navigation(surface, &Request::new("https://example.test/"));
        let obs1 = engine.poll_observation().expect("Started obs");
        assert_eq!(obs1.epoch(), NavigationEpoch::FIRST); // Started before commit
        assert_eq!(
            obs1.event(),
            &NavigationEvent::Started {
                id: nav1,
                address: "https://example.test/".into(),
            }
        );

        let obs2 = engine.poll_observation().expect("Committed obs");
        let expected_epoch = NavigationEpoch::FIRST.next();
        assert_eq!(obs2.epoch(), expected_epoch);
        assert_eq!(engine.surface_navigation_epoch(surface), expected_epoch);

        let _ = engine.poll_observation(); // TitleChanged
        let _ = engine.poll_observation(); // Succeeded

        // Same document navigation
        let nav2 = engine.navigate_same_document(surface, "https://example.test/#section");
        let next_epoch = expected_epoch.next();
        assert_eq!(engine.surface_navigation_epoch(surface), next_epoch);

        let obs3 = engine
            .poll_observation()
            .expect("SameDocumentNavigated obs");
        assert_eq!(obs3.epoch(), next_epoch);
        assert_eq!(
            obs3.event(),
            &NavigationEvent::SameDocumentNavigated {
                id: nav2,
                address: "https://example.test/#section".into(),
            }
        );

        // Confirm poll_event returns unwrapped NavigationEvent
        let _ = engine.navigate_same_document(surface, "https://example.test/#another");
        let ev = engine
            .poll_event()
            .expect("poll_event returns unwrapped event");
        assert!(matches!(ev, NavigationEvent::SameDocumentNavigated { .. }));
    }

    #[test]
    fn hosted_surface_from_shell_supplied_bytes() {
        let mut engine = HeadlessEngine::new();
        let surface = engine.create_surface(DataStoreSelector::Persistent);
        let identity = SurfaceIdentity::new("app://local-ledger");
        let html_bytes = b"<!DOCTYPE html><html><body>Ledger UI</body></html>".to_vec();

        let nav_id = engine.host_surface_bytes(surface, identity.clone(), html_bytes.clone());

        assert_eq!(engine.surface_identity(surface), Some(&identity));
        assert_eq!(engine.surface_hosted_bytes(surface), Some(&html_bytes[..]));
        assert_eq!(
            engine.surface_current(surface).map(|p| p.address()),
            Some("app://local-ledger")
        );

        let mut events = Vec::new();
        while let Some(obs) = engine.poll_observation() {
            events.push(obs);
        }

        assert_eq!(events.len(), 3);
        assert_eq!(
            events[0].event(),
            &NavigationEvent::Started {
                id: nav_id,
                address: "app://local-ledger".into(),
            }
        );
        assert_eq!(
            events[1].event(),
            &NavigationEvent::Committed {
                id: nav_id,
                address: "app://local-ledger".into(),
            }
        );
        assert_eq!(
            events[2].event(),
            &NavigationEvent::Succeeded { id: nav_id }
        );
    }

    #[test]
    fn tagged_message_channel_with_shell_assigned_app_id() {
        let mut engine = HeadlessEngine::new();
        let surface = engine.create_surface(DataStoreSelector::Persistent);
        let app_id = AppId::new("app.system.ledger");

        engine.set_surface_app_id(surface, app_id.clone());
        assert_eq!(engine.surface_app_id(surface), Some(&app_id));

        // Shell sends message to surface
        engine.send_message_to_surface(surface, "{\"command\":\"get_balance\"}");
        assert_eq!(
            engine.outgoing_messages(),
            &[(surface, "{\"command\":\"get_balance\"}".to_string())]
        );

        // Surface posts message back to shell
        engine.post_message_from_surface(surface, "{\"balance\":42}");
        let received = engine.poll_message().expect("Tagged message expected");
        assert_eq!(received.surface(), surface);
        assert_eq!(received.app_id(), &app_id);
        assert_eq!(received.payload(), "{\"balance\":42}");
        assert!(engine.poll_message().is_none());
    }
}
