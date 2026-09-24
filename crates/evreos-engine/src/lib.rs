//! The rendering seam.
//!
//! Principle III requires rendering to go through an interface **the shell
//! defines as the consumer**, with the system web runtime as the default
//! implementation and a headless implementation kept working from day one, so
//! that the seam is proved by a second implementation rather than asserted.
//!
//! Two consequences shape everything here.
//!
//! The trait is written from what the shell needs, never from what a webview
//! offers. Nothing in this crate names a platform, a runtime, or a vendor, and
//! nothing returns a handle to one. A seam that leaks its default
//! implementation's vocabulary is a seam only until the second implementation
//! arrives, which is the failure this interface exists to prevent.
//!
//! Failure is a value, not a panic. FR-015 requires the browser to distinguish
//! a failed load from a successful one and to name the cause, and records that
//! "treating a failed load as a successful empty page is a defect, as is a
//! loading indicator that never resolves". A trait that can only succeed or
//! panic cannot carry that requirement, so a navigation's outcome arrives as
//! [`NavigationEvent`]s and [`LoadError`] enumerates the causes FR-015 names.

#![forbid(unsafe_code)]

#[cfg(feature = "conformance")]
pub mod conformance;

use core::fmt;

/// What the shell asks an engine to render.
///
/// An address is carried as a string rather than a parsed URL type: parsing and
/// policy belong to the shell, which owns FR-003's combined entry field and
/// FR-007a's rule on what may leave the machine. An engine that parsed
/// addresses would be making those decisions where they cannot be reviewed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    address: String,
}

impl Request {
    /// Build a request for `address`.
    pub fn new(address: impl Into<String>) -> Self {
        Self {
            address: address.into(),
        }
    }

    /// The address the shell asked for, unmodified.
    pub fn address(&self) -> &str {
        &self.address
    }
}

/// Why a load did not produce a page.
///
/// The four variants are the four failures FR-015 enumerates, and SC-009
/// requires each to be exercised on every supported platform. They are a closed
/// set on purpose: a catch-all would let an implementation report every failure
/// as one indistinguishable cause, which is the state FR-015 exists to forbid.
/// Adding a cause is a change to this enum, visible in review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// The address did not resolve.
    Unresolvable { address: String },
    /// The certificate was untrusted, expired, or did not match.
    Certificate { address: String, detail: String },
    /// Something between the shell and the site answered in its place.
    ///
    /// # Contract Clause: Platform Synthesis Forbidden
    ///
    /// No platform backend (e.g., WebView2 on Windows or WKWebView on macOS)
    /// may synthesise `Intercepted` from a platform error code or status.
    /// Platform error codes contain no value denoting interception because an
    /// intercepted request (such as a captive portal) completes successfully
    /// from the platform webview's network perspective.
    ///
    /// An engine implementation reporting `Intercepted` MUST do so from a
    /// shell-supplied classification rather than from a mapped platform status.
    /// The headless engine is currently the sole producer of this error variant
    /// for testing and simulation, keeping SC-009's fourth case exercisable
    /// while the intercepted-navigation founder decision recorded in
    /// `decisions/0005` remains open.
    Intercepted { address: String },
    /// The site demanded credentials before serving anything.
    AuthenticationRequired { address: String },
}

impl LoadError {
    /// The address the failure concerns.
    pub fn address(&self) -> &str {
        match self {
            Self::Unresolvable { address }
            | Self::Certificate { address, .. }
            | Self::Intercepted { address }
            | Self::AuthenticationRequired { address } => address,
        }
    }
}

impl fmt::Display for LoadError {
    /// Plain language, because FR-015 requires the error state to name the
    /// cause to the member. These strings are the fallback the shell uses when
    /// no catalogue under FR-035 has a translation; they are deliberately not
    /// the member-facing copy, which is localised.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unresolvable { address } => {
                write!(f, "{address} could not be found")
            }
            Self::Certificate { address, detail } => {
                write!(
                    f,
                    "the identity of {address} could not be verified: {detail}"
                )
            }
            Self::Intercepted { address } => {
                write!(f, "something answered in place of {address}")
            }
            Self::AuthenticationRequired { address } => {
                write!(f, "{address} requires a sign-in before it will load")
            }
        }
    }
}

impl core::error::Error for LoadError {}

/// A page an engine has loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    address: String,
    title: String,
}

impl Page {
    pub fn new(address: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            address: address.into(),
            title: title.into(),
        }
    }

    /// The address that actually loaded, which may differ from the requested
    /// one after a redirect. The shell shows this rather than what was typed,
    /// because showing the request while displaying the response is how an
    /// address bar lies.
    pub fn address(&self) -> &str {
        &self.address
    }

    pub fn title(&self) -> &str {
        &self.title
    }
}

/// Identifies one navigation for the lifetime of one engine instance.
///
/// Minted by the engine and opaque to the shell, which only ever stores,
/// compares and hashes it. There is no public constructor from an integer,
/// so no integer semantics enter the seam — the id is a correlation token,
/// not a capability, and [`NavigationId::FIRST`] with [`NavigationId::next`]
/// makes the minting sequence public rather than secret. An engine with
/// platform navigation identifiers of its own maps them to these rather than
/// exposing them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NavigationId(u64);

impl NavigationId {
    /// The first id an engine mints.
    pub const FIRST: NavigationId = NavigationId(0);

    /// The id minted after this one. An engine mints sequentially from
    /// [`NavigationId::FIRST`], one sequence per engine instance, covering
    /// shell-initiated and engine-initiated navigations alike — which is what
    /// makes every id unique within an instance.
    #[must_use]
    pub fn next(self) -> NavigationId {
        NavigationId(self.0 + 1)
    }
}

/// Identifies one shared platform context for the lifetime of an [`EngineHost`].
///
/// Minted by the host and opaque to the shell, which only ever stores,
/// compares and hashes it. There is no public constructor from an integer,
/// so no integer semantics enter the seam — the id is a correlation token,
/// not a capability, and [`ContextId::FIRST`] with [`ContextId::next`]
/// makes the minting sequence public rather than secret. An engine host
/// with platform context identifiers of its own maps them to these rather
/// than exposing them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ContextId(u64);

impl ContextId {
    /// The first context id a host sequence mints.
    pub const FIRST: ContextId = ContextId(0);

    /// The id minted after this one. An engine host mints sequentially from
    /// [`ContextId::FIRST`], one sequence per host environment.
    #[must_use]
    pub fn next(self) -> ContextId {
        ContextId(self.0 + 1)
    }
}

impl fmt::Display for ContextId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ctx-{}", self.0)
    }
}

/// Identifies one addressable rendering surface within an [`Engine`].
///
/// Minted by the engine and opaque to the shell, which only ever stores,
/// compares, and hashes it. There is no public constructor from an integer,
/// so no integer semantics enter the seam — the id is a correlation token,
/// not a capability, and [`SurfaceId::FIRST`] with [`SurfaceId::next`]
/// makes the minting sequence public rather than secret. An engine with
/// platform surface or window identifiers of its own maps them to these
/// rather than exposing them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SurfaceId(u64);

impl SurfaceId {
    /// The first surface id an engine mints.
    pub const FIRST: SurfaceId = SurfaceId(0);

    /// The id minted after this one. An engine mints sequentially from
    /// [`SurfaceId::FIRST`], one sequence per engine instance.
    #[must_use]
    pub fn next(self) -> SurfaceId {
        SurfaceId(self.0 + 1)
    }
}

impl fmt::Display for SurfaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "surface-{}", self.0)
    }
}

/// Distinguishes the persistent data store from a non-persistent one.
///
/// Under FR-007 and ADR-0001, a normal window or tab uses a persistent data
/// store, whereas private browsing requires a distinct non-persistent store
/// that leaves no browsing traces behind when its surface closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DataStoreSelector {
    /// The default persistent data store used for normal browsing.
    #[default]
    Persistent,
    /// A distinct, isolated, non-persistent data store for private browsing (FR-007).
    NonPersistent,
}

impl DataStoreSelector {
    /// Whether this selector designates a persistent data store.
    pub fn is_persistent(self) -> bool {
        matches!(self, Self::Persistent)
    }

    /// Whether this selector designates a non-persistent data store.
    pub fn is_non_persistent(self) -> bool {
        matches!(self, Self::NonPersistent)
    }
}

impl fmt::Display for DataStoreSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Persistent => write!(f, "persistent"),
            Self::NonPersistent => write!(f, "non-persistent"),
        }
    }
}

/// The lifecycle state of an addressable rendering surface.
///
/// Surfaces transition between these states through [`Engine::create_surface`],
/// [`Engine::activate_surface`], [`Engine::suspend_surface`],
/// [`Engine::resume_surface`], and [`Engine::close_surface`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfaceState {
    /// The surface is active and in the foreground.
    Active,
    /// The surface is inactive (e.g. a background tab).
    Inactive,
    /// The surface is suspended to conserve memory and resources (FR-002).
    Suspended,
    /// The surface has been closed and destroyed.
    Closed,
}

impl SurfaceState {
    /// Whether the surface is currently active.
    pub fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }

    /// Whether the surface is currently suspended.
    pub fn is_suspended(self) -> bool {
        matches!(self, Self::Suspended)
    }

    /// Whether the surface is closed.
    pub fn is_closed(self) -> bool {
        matches!(self, Self::Closed)
    }
}

impl fmt::Display for SurfaceState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Inactive => write!(f, "inactive"),
            Self::Suspended => write!(f, "suspended"),
            Self::Closed => write!(f, "closed"),
        }
    }
}

/// A compiled content-blocking policy installed into an [`Engine`].
///
/// Under FR-008 and ADR-0001, tracker and advert blocking is active from first
/// launch. Because enforcement mechanisms differ structurally across platforms —
/// in-process evaluation of an adblock matcher on tier 1 versus precompiled
/// rule lists in WebKit on tier 2 — the engine seam accepts a compiled policy
/// rather than exposing a per-request veto (`should_block`), preventing the seam
/// from becoming platform-shaped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledPolicy {
    name: String,
    data: Vec<u8>,
}

impl Default for CompiledPolicy {
    fn default() -> Self {
        Self {
            name: "default".into(),
            data: Vec::new(),
        }
    }
}

impl CompiledPolicy {
    /// Create a new compiled policy from `name` and raw policy `data`.
    pub fn new(name: impl Into<String>, data: impl Into<Vec<u8>>) -> Self {
        Self {
            name: name.into(),
            data: data.into(),
        }
    }

    /// Construct a compiled policy from a sequence of text rules.
    pub fn from_rules<S: AsRef<str>>(
        name: impl Into<String>,
        rules: impl IntoIterator<Item = S>,
    ) -> Self {
        let joined = rules
            .into_iter()
            .map(|r| r.as_ref().to_owned())
            .collect::<Vec<_>>()
            .join("\n");
        Self::new(name, joined.into_bytes())
    }

    /// The name or identifier of this policy.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The raw compiled policy payload.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Whether this policy matches `url`, blocking it.
    ///
    /// Evaluates line-delimited pattern rules against `url`.
    pub fn matches(&self, url: &str) -> bool {
        if self.data.is_empty() {
            return false;
        }
        if let Ok(text) = core::str::from_utf8(&self.data) {
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
                    continue;
                }
                if url.contains(line) {
                    return true;
                }
            }
        }
        false
    }
}

impl fmt::Display for CompiledPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "policy:{}({} bytes)", self.name, self.data.len())
    }
}

/// One observation about one navigation.
///
/// Every variant carries the [`NavigationId`] it belongs to. The title travels
/// on its own event and never inside an outcome, because a title is a property
/// of a document that can change long after the load finished, not a property
/// of the load.
///
/// # Ordering, per navigation
///
/// - `Started` is the first event for any id that produces events. For a
///   navigation the shell began, the id is the one [`Engine::start_navigation`]
///   returned. For a navigation the page content began — a link, a script, a
///   form, a refresh — `Started` is the first the shell hears of it, carrying
///   a fresh id no `start_navigation` call returned.
/// - `Redirected` appears zero or more times, strictly between `Started` and
///   `Committed`.
/// - `Committed` appears at most once. It is the moment the engine is rendering
///   the response and the moment [`Engine::current`] changes: from this event's
///   emission, `current()` returns this page — at the committed address, with
///   an empty title until the first `TitleChanged` — until a newer navigation
///   commits. A shell draining behind the engine reads about the change after
///   it happened; `current()` never waits for the drain. The address the event
///   carries is the one that actually loaded — after redirects, the final one —
///   and is what the shell displays.
/// - Each navigation ends with at most one of `Succeeded`, `Failed` or
///   `NavigatedAway` — or never ends, which is the load that never resolves.
///   The bound on how long the shell waits is the shell's policy under SC-009,
///   deliberately not an engine invariant, so no engine event expresses it.
/// - `Succeeded` only ever follows `Committed`. It carries no page and no
///   title: the committed page is read from `current()`, which still returns it
///   unless a newer navigation has since committed, and the title arrives on
///   `TitleChanged` — before or after `Succeeded`, with no ordering promised
///   between them.
/// - `Failed` and `Committed` are mutually exclusive for one id. Each of the
///   four causes [`LoadError`] enumerates is a condition established before
///   anything replaces the page being viewed, which is what makes "a failure
///   never replaces the current page" a mechanical property rather than an
///   aspiration. A failure after commit — a connection lost mid-render — has
///   no variant here; adding that cause is a change to [`LoadError`], made
///   under the rule that enum states.
/// - `TitleChanged` appears only after `Committed` for the same id, any number
///   of times, including after `Succeeded` — script retitles pages long after
///   they load. When the id is the navigation whose page `current()` returns,
///   the engine updates that page's title at the event's emission — `current()`
///   reflects it whether or not the event has been drained. A `TitleChanged`
///   for a superseded navigation never alters the current page.
/// - `NavigatedAway` means the engine abandoned the navigation without an
///   outcome — a newer navigation superseded it, or it was stopped. No further
///   events for that id ever follow.
///
/// Across navigations, events drain in emission order and events for different
/// ids may interleave; per-id ordering and FIFO drain are the only guarantees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationEvent {
    /// The engine began a navigation.
    Started { id: NavigationId, address: String },
    /// Before commit, the navigation was redirected to `address`.
    Redirected { id: NavigationId, address: String },
    /// The engine is now rendering the response from `address`.
    Committed { id: NavigationId, address: String },
    /// A same-document navigation occurred without full document re-render.
    SameDocumentNavigated { id: NavigationId, address: String },
    /// The committed navigation finished loading.
    Succeeded { id: NavigationId },
    /// The navigation did not commit, for the cause carried.
    Failed { id: NavigationId, error: LoadError },
    /// The document of navigation `id` has this title.
    TitleChanged { id: NavigationId, title: String },
    /// The engine abandoned the navigation without an outcome.
    NavigatedAway { id: NavigationId },
}

impl NavigationEvent {
    /// The navigation this event belongs to.
    pub fn id(&self) -> NavigationId {
        match self {
            Self::Started { id, .. }
            | Self::Redirected { id, .. }
            | Self::Committed { id, .. }
            | Self::SameDocumentNavigated { id, .. }
            | Self::Succeeded { id }
            | Self::Failed { id, .. }
            | Self::TitleChanged { id, .. }
            | Self::NavigatedAway { id } => *id,
        }
    }
}

/// An identifier for a surface hosted from shell-supplied bytes.
///
/// Under FR-019a, verification of signed app surface bytes precedes rendering
/// and caching. The shell hands the verified bytes directly to the engine under
/// a shell-chosen [`SurfaceIdentity`], ensuring that no custom URL scheme or
/// protocol vocabulary (e.g. `evreos-app://` or `file://`) leaks into the trait.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SurfaceIdentity(String);

impl SurfaceIdentity {
    /// Create a new surface identity.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// The string representation of this surface identity.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SurfaceIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for SurfaceIdentity {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for SurfaceIdentity {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl AsRef<str> for SurfaceIdentity {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// A monotonic counter bounding a navigation occasion.
///
/// Under FR-018a: "Every change of address the member observes is a navigation,
/// including one the page performs without fetching a new document."
/// The navigation epoch increments on every change of address (same-document
/// navigation included), scoping a member occasion, click-out completion,
/// and cashback offer control lifetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NavigationEpoch(u64);

impl NavigationEpoch {
    /// The initial navigation epoch for a newly created surface.
    pub const FIRST: Self = Self(1);

    /// Construct an epoch from a raw counter value.
    pub const fn new(val: u64) -> Self {
        Self(val)
    }

    /// The underlying raw monotonic counter value.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// The underlying raw monotonic counter value as `u64`.
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Return the next sequential epoch.
    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

impl fmt::Display for NavigationEpoch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "epoch:{}", self.0)
    }
}

/// An observation emitted by the engine pairing a [`NavigationEvent`] with the
/// active [`NavigationEpoch`].
///
/// Under FR-018a, the navigation epoch increments on every change to the address
/// the member is on, including same-document navigations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationObservation {
    epoch: NavigationEpoch,
    event: NavigationEvent,
}

impl NavigationObservation {
    /// Create a new navigation observation pairing an epoch with an event.
    pub fn new(epoch: NavigationEpoch, event: NavigationEvent) -> Self {
        Self { epoch, event }
    }

    /// The active navigation epoch at the moment this event was observed.
    pub fn epoch(&self) -> NavigationEpoch {
        self.epoch
    }

    /// The underlying navigation event.
    pub fn event(&self) -> &NavigationEvent {
        &self.event
    }

    /// Consume the observation, returning the underlying navigation event.
    pub fn into_event(self) -> NavigationEvent {
        self.event
    }
}

/// What the shell requires of anything that renders web content.
///
/// Implemented by the system-webview backend on each supported platform and by
/// [`evreos-engine-headless`] for tests. FR-044 requires both to exist and the
/// second to be kept working from milestone M0.
///
/// # Why there is no synchronous load
///
/// The system web runtime on either supported tier is affine to the thread
/// that runs the interface loop, and it delivers navigation outcomes through
/// callbacks on that same thread — so there is no second thread for a
/// synchronous call to block on. The only synchronous route is a nested
/// message pump on the interface thread, which dispatches input and paint
/// re-entrantly for the length of a page load; SC-006 admits no trial over
/// 16 ms, so that route breaches it by construction rather than by bad luck.
/// The evidence sits with the research behind the seam
/// (`specs/001-evreos-v1/research.md` §1), which also records what a
/// synchronous result could never express: a navigation the page content
/// started with no call from the shell, a load still in flight — SC-009's
/// indicator that must resolve — and a title arriving on its own event.
///
/// There is deliberately no `Send` bound anywhere on this path. Both shipping
/// backends are affine to the interface thread; a bound that let an engine
/// cross threads would promise what no implementation can keep.
pub trait Engine {
    /// A short, stable name for this implementation, used in diagnostics and in
    /// the benchmark records SC-013 requires to be reproducible.
    fn name(&self) -> &'static str;

    /// The opaque identifier of the shared platform context this engine belongs to.
    ///
    /// Engines minted from the same [`EngineHost`] share the same context ID;
    /// engines minted from different hosts have distinct context IDs.
    ///
    /// The default implementation returns [`ContextId::FIRST`], suitable for
    /// isolated or single-engine test mocks. Real implementations and host-minted
    /// engines supply the identifier from their host.
    fn context_id(&self) -> ContextId {
        ContextId::FIRST
    }

    /// Whether this engine shares its platform context with `other`.
    fn shares_context_with(&self, other: &impl Engine) -> bool {
        self.context_id() == other.context_id()
    }

    /// Begin navigating to `request`.
    ///
    /// Returns immediately with the id the engine minted for this navigation;
    /// the outcome arrives as [`NavigationEvent`]s carrying that id. Starting
    /// a navigation never blocks on the network and never reports an outcome
    /// itself — an implementation that could fail here would be deciding
    /// synchronously what FR-015 requires to be reported as a named state.
    fn start_navigation(&mut self, request: &Request) -> NavigationId;

    /// The next pending event, oldest first, or `None` when no event is
    /// pending right now.
    ///
    /// MUST NOT block. `None` means the queue is empty at this call, never
    /// that no event will come: a load that never resolves and a load whose
    /// outcome has not arrived yet look identical here, and telling them
    /// apart is the shell's policy under SC-009, on the shell's clock.
    fn poll_event(&mut self) -> Option<NavigationEvent>;

    /// The page currently displayed, if any.
    ///
    /// Reflects every event the engine has emitted, drained or not: the page
    /// changes when a navigation commits, not when the shell reads about it.
    fn current(&self) -> Option<&Page>;

    /// Create an addressable rendering surface with the specified data store.
    ///
    /// The returned [`SurfaceId`] is minted sequentially and is unique within
    /// this engine instance.
    fn create_surface(&mut self, _store: DataStoreSelector) -> SurfaceId {
        SurfaceId::FIRST
    }

    /// Activate `surface`, bringing it to the foreground.
    ///
    /// A surface switch changes visibility and bounds rather than re-navigating
    /// (SC-006 16 ms requirement).
    fn activate_surface(&mut self, _surface: SurfaceId) {}

    /// Suspend `surface` to conserve memory and resources (FR-002).
    ///
    /// Suspending a surface preserves all shell-observable state (such as the
    /// current page, document title, and address) without losing state when
    /// resumed.
    fn suspend_surface(&mut self, _surface: SurfaceId) {}

    /// Resume a previously suspended rendering surface.
    ///
    /// Restores the surface without losing any state the shell can observe.
    fn resume_surface(&mut self, _surface: SurfaceId) {}

    /// Close and destroy `surface`.
    ///
    /// If `surface` was created with a [`DataStoreSelector::NonPersistent`] data
    /// store, all stored data is destroyed and leaves nothing behind (FR-007).
    fn close_surface(&mut self, _surface: SurfaceId) {}

    /// The currently active surface, if any.
    fn active_surface(&self) -> Option<SurfaceId> {
        Some(SurfaceId::FIRST)
    }

    /// The data store selector of `surface`, if the surface exists.
    fn surface_data_store(&self, _surface: SurfaceId) -> Option<DataStoreSelector> {
        Some(DataStoreSelector::Persistent)
    }

    /// The current lifecycle state of `surface`, if the surface exists.
    fn surface_state(&self, _surface: SurfaceId) -> Option<SurfaceState> {
        Some(SurfaceState::Active)
    }

    /// The page currently displayed on `surface`, if any.
    fn surface_current(&self, _surface: SurfaceId) -> Option<&Page> {
        self.current()
    }

    /// Begin navigating `surface` to `request`.
    fn start_surface_navigation(&mut self, _surface: SurfaceId, request: &Request) -> NavigationId {
        self.start_navigation(request)
    }

    /// Whether `surface` retains any data in its data store.
    ///
    /// For a non-persistent data store, closing the surface destroys all
    /// browsing traces, so this returns `false` after [`close_surface`](Self::close_surface).
    fn surface_has_retained_data(&self, _surface: SurfaceId) -> bool {
        false
    }

    /// Install or replace the compiled content-blocking policy for this engine.
    ///
    /// Under FR-008 and ADR-0001, tracker and advert blocking is active from first
    /// launch. Because enforcement mechanisms differ structurally across platforms —
    /// in-process evaluation of an adblock matcher on tier 1 versus precompiled
    /// rule lists in WebKit on tier 2 — the engine seam accepts a compiled policy
    /// rather than exposing a per-request veto (`should_block`), preventing the seam
    /// from becoming platform-shaped.
    fn install_policy(&mut self, _policy: CompiledPolicy) {}

    /// Exempt `site` from content blocking.
    ///
    /// Under FR-008, the browser provides a visible per-site control to turn
    /// blocking off for a broken site. An exempted site allows all subresource
    /// requests without blocking.
    fn exempt_site(&mut self, _site: &str) {}

    /// Remove a previously granted site exemption.
    fn remove_site_exemption(&mut self, _site: &str) {}

    /// Whether `site` is currently exempted from content blocking.
    fn is_site_exempt(&self, _site: &str) -> bool {
        false
    }

    /// The count of blocked requests or resources for the page currently loaded on `surface`.
    ///
    /// Under FR-008, the chrome observes this count to display blocking status
    /// to the member and support parity verification in CI.
    fn surface_blocked_count(&self, _surface: SurfaceId) -> u64 {
        0
    }

    /// Synonym for [`surface_blocked_count`](Self::surface_blocked_count).
    fn blocked_count(&self, surface: SurfaceId) -> u64 {
        self.surface_blocked_count(surface)
    }

    /// The list of URLs or resource identifiers that were blocked on `surface` during the current page load.
    fn surface_blocked_items(&self, _surface: SurfaceId) -> Vec<String> {
        Vec::new()
    }

    // Seam Addition: Host surface from shell-supplied bytes (FR-019a)
    /// Host content on `surface` from shell-supplied `bytes` under a shell-chosen `identity`.
    ///
    /// Under FR-019a, the shell verifies signed app surfaces before rendering or
    /// writing to cache, supplying the verified bytes directly to the engine without
    /// leaking custom scheme or protocol vocabulary into the trait.
    fn host_surface_bytes(
        &mut self,
        _surface: SurfaceId,
        _identity: SurfaceIdentity,
        _bytes: Vec<u8>,
    ) -> NavigationId {
        NavigationId::FIRST
    }

    /// The [`SurfaceIdentity`] currently hosted on `surface`, if any.
    fn surface_identity(&self, _surface: SurfaceId) -> Option<&SurfaceIdentity> {
        None
    }

    /// The raw bytes currently hosted on `surface`, if any.
    fn surface_hosted_bytes(&self, _surface: SurfaceId) -> Option<&[u8]> {
        None
    }

    // Seam Addition: Navigation observation carrying an epoch (FR-018a)
    /// Poll the next navigation observation from the engine, if any is ready.
    ///
    /// Unlike [`poll_event`](Self::poll_event), this observation carries the [`NavigationEpoch`]
    /// active at the moment the event occurred.
    fn poll_observation(&mut self) -> Option<NavigationObservation> {
        self.poll_event()
            .map(|event| NavigationObservation::new(NavigationEpoch::FIRST, event))
    }

    /// The current [`NavigationEpoch`] of `surface`.
    ///
    /// Under FR-018a, the epoch increments on every change to the address,
    /// including same-document navigations.
    fn surface_navigation_epoch(&self, _surface: SurfaceId) -> NavigationEpoch {
        NavigationEpoch::FIRST
    }

    /// Perform a same-document navigation on `surface` to `address` (e.g. fragment identifier or history API).
    ///
    /// Under FR-018a, this updates the address without full document re-render and increments the navigation epoch.
    fn navigate_same_document(&mut self, _surface: SurfaceId, _address: &str) -> NavigationId {
        NavigationId::FIRST
    }
}

/// What the shell requires of anything that hosts engines and owns their shared
/// platform context.
///
/// Implemented by the system-webview backend on each supported platform
/// (e.g. WebView2 on Windows, WKWebView on macOS) and by
/// [`evreos-engine-headless`] for tests.
///
/// # Why there is a host seam above [`Engine`]
///
/// The system web runtime on both supported tiers is per-view-by-default:
/// WebView2 creates a fresh user data folder and browser process per view unless
/// an explicit `CoreWebView2Environment` is shared, and WKWebView creates a fresh
/// data store per web view unless a shared `WKWebViewConfiguration` /
/// `WKWebsiteDataStore` is provided. Ten tabs each minting their own context
/// loses SC-004's 150 MB memory budget before any product code exists.
///
/// The host owns the shared platform context and mints [`Engine`] instances from it,
/// ensuring that instances minted from the same host share the context and
/// instances from two hosts do not.
///
/// There is deliberately no `Send` bound anywhere on this path. Both shipping
/// backends are affine to the interface thread; a bound that let an engine or host
/// cross threads would promise what no implementation can keep.
pub trait EngineHost {
    /// The type of [`Engine`] this host mints.
    type Engine: Engine;

    /// A short, stable name for this host implementation, used in diagnostics and
    /// benchmark records.
    fn name(&self) -> &'static str;

    /// The opaque identifier of the shared platform context this host owns.
    fn context_id(&self) -> ContextId;

    /// Mint a new [`Engine`] instance that shares this host's platform context.
    fn create_engine(&mut self) -> Self::Engine;

    /// Synonym for [`create_engine`](Self::create_engine).
    fn mint_engine(&mut self) -> Self::Engine {
        self.create_engine()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[test]
    fn ids_mint_sequentially_and_distinctly() {
        let first = NavigationId::FIRST;
        let second = first.next();
        let third = second.next();
        assert_ne!(first, second);
        assert_ne!(second, third);
        assert_ne!(first, third);
    }

    #[test]
    fn context_ids_mint_sequentially_and_distinctly() {
        let first = ContextId::FIRST;
        let second = first.next();
        let third = second.next();
        assert_ne!(first, second);
        assert_ne!(second, third);
        assert_ne!(first, third);
        assert_eq!(first.to_string(), "ctx-0");
        assert_eq!(second.to_string(), "ctx-1");
    }

    #[test]
    fn surface_ids_mint_sequentially_and_distinctly() {
        let first = SurfaceId::FIRST;
        let second = first.next();
        let third = second.next();
        assert_ne!(first, second);
        assert_ne!(second, third);
        assert_ne!(first, third);
        assert_eq!(first.to_string(), "surface-0");
        assert_eq!(second.to_string(), "surface-1");
    }

    #[test]
    fn data_store_selector_variants_and_predicates() {
        let persistent = DataStoreSelector::Persistent;
        let non_persistent = DataStoreSelector::NonPersistent;
        assert!(persistent.is_persistent());
        assert!(!persistent.is_non_persistent());
        assert!(non_persistent.is_non_persistent());
        assert!(!non_persistent.is_persistent());
        assert_eq!(persistent.to_string(), "persistent");
        assert_eq!(non_persistent.to_string(), "non-persistent");
        assert_eq!(DataStoreSelector::default(), DataStoreSelector::Persistent);
    }

    #[test]
    fn surface_state_variants_and_predicates() {
        let active = SurfaceState::Active;
        let inactive = SurfaceState::Inactive;
        let suspended = SurfaceState::Suspended;
        let closed = SurfaceState::Closed;

        assert!(active.is_active());
        assert!(!active.is_suspended());
        assert!(!active.is_closed());

        assert!(!inactive.is_active());
        assert!(!inactive.is_suspended());
        assert!(!inactive.is_closed());

        assert!(!suspended.is_active());
        assert!(suspended.is_suspended());
        assert!(!suspended.is_closed());

        assert!(!closed.is_active());
        assert!(!closed.is_suspended());
        assert!(closed.is_closed());

        assert_eq!(active.to_string(), "active");
        assert_eq!(inactive.to_string(), "inactive");
        assert_eq!(suspended.to_string(), "suspended");
        assert_eq!(closed.to_string(), "closed");
    }

    #[test]
    fn compiled_policy_creation_and_matching() {
        let policy =
            CompiledPolicy::new("test-policy", b"tracker.test\n# comment\n\nanalytics.test");
        assert_eq!(policy.name(), "test-policy");
        assert_eq!(policy.data(), b"tracker.test\n# comment\n\nanalytics.test");
        assert!(policy.matches("https://tracker.test/pixel.gif"));
        assert!(policy.matches("https://sub.analytics.test/script.js"));
        assert!(!policy.matches("https://clean-site.test/style.css"));
        assert_eq!(
            policy.to_string(),
            format!("policy:test-policy({} bytes)", policy.data().len())
        );

        let from_rules = CompiledPolicy::from_rules("rules-policy", ["ads.test", "banner.test"]);
        assert_eq!(from_rules.name(), "rules-policy");
        assert!(from_rules.matches("https://ads.test/banner.jpg"));
        assert!(from_rules.matches("https://banner.test/"));
        assert!(!from_rules.matches("https://other.test/"));

        let empty = CompiledPolicy::default();
        assert_eq!(empty.name(), "default");
        assert!(!empty.matches("https://anything.test/"));
    }

    #[test]
    fn every_event_variant_names_its_navigation() {
        let id = NavigationId::FIRST.next();
        let events = [
            NavigationEvent::Started {
                id,
                address: "a".into(),
            },
            NavigationEvent::Redirected {
                id,
                address: "b".into(),
            },
            NavigationEvent::Committed {
                id,
                address: "b".into(),
            },
            NavigationEvent::Succeeded { id },
            NavigationEvent::Failed {
                id,
                error: LoadError::Unresolvable {
                    address: "a".into(),
                },
            },
            NavigationEvent::TitleChanged {
                id,
                title: "t".into(),
            },
            NavigationEvent::NavigatedAway { id },
        ];
        for event in events {
            assert_eq!(event.id(), id, "{event:?} lost its id");
        }
    }

    /// An engine that cannot cross threads, because it holds an `Rc`. If a
    /// `Send` bound ever lands on this trait — a supertrait or a method-level
    /// bound — this implementation stops compiling, and the bound is caught
    /// here rather than by the first platform backend that cannot satisfy it.
    /// This guards the trait alone: a bound added to a consumer's generic
    /// entry point is invisible to this crate, so each such entry point in
    /// the shell carries its own non-`Send` engine where it lives.
    struct ThreadAffine {
        _pinned: Rc<()>,
        context_id: ContextId,
        queue: Vec<NavigationEvent>,
        next: NavigationId,
    }

    impl Engine for ThreadAffine {
        fn name(&self) -> &'static str {
            "thread-affine"
        }

        fn context_id(&self) -> ContextId {
            self.context_id
        }

        fn start_navigation(&mut self, request: &Request) -> NavigationId {
            let id = self.next;
            self.next = id.next();
            self.queue.push(NavigationEvent::Started {
                id,
                address: request.address().to_owned(),
            });
            id
        }

        fn poll_event(&mut self) -> Option<NavigationEvent> {
            if self.queue.is_empty() {
                None
            } else {
                Some(self.queue.remove(0))
            }
        }

        fn current(&self) -> Option<&Page> {
            None
        }
    }

    struct ThreadAffineHost {
        _pinned: Rc<()>,
        context_id: ContextId,
    }

    impl EngineHost for ThreadAffineHost {
        type Engine = ThreadAffine;

        fn name(&self) -> &'static str {
            "thread-affine-host"
        }

        fn context_id(&self) -> ContextId {
            self.context_id
        }

        fn create_engine(&mut self) -> Self::Engine {
            ThreadAffine {
                _pinned: self._pinned.clone(),
                context_id: self.context_id,
                queue: Vec::new(),
                next: NavigationId::FIRST,
            }
        }
    }

    #[test]
    fn the_engine_path_carries_no_send_bound() {
        let mut engine = ThreadAffine {
            _pinned: Rc::new(()),
            context_id: ContextId::FIRST,
            queue: Vec::new(),
            next: NavigationId::FIRST,
        };
        let id = engine.start_navigation(&Request::new("https://a.invalid/"));
        assert_eq!(engine.poll_event().map(|event| event.id()), Some(id));
        assert_eq!(engine.poll_event(), None);
    }

    #[test]
    fn the_engine_host_path_carries_no_send_bound() {
        let mut host = ThreadAffineHost {
            _pinned: Rc::new(()),
            context_id: ContextId::FIRST,
        };
        let mut engine = host.create_engine();
        assert_eq!(engine.context_id(), host.context_id());
        let id = engine.start_navigation(&Request::new("https://a.invalid/"));
        assert_eq!(engine.poll_event().map(|event| event.id()), Some(id));
    }

    #[test]
    fn engines_from_same_host_share_context_and_from_different_hosts_do_not() {
        let mut host1 = ThreadAffineHost {
            _pinned: Rc::new(()),
            context_id: ContextId::FIRST,
        };
        let engine1_a = host1.create_engine();
        let engine1_b = host1.create_engine();

        let mut host2 = ThreadAffineHost {
            _pinned: Rc::new(()),
            context_id: ContextId::FIRST.next(),
        };
        let engine2 = host2.create_engine();

        assert_eq!(engine1_a.context_id(), engine1_b.context_id());
        assert_eq!(engine1_a.context_id(), host1.context_id());
        assert_ne!(engine1_a.context_id(), engine2.context_id());
        assert_ne!(host1.context_id(), host2.context_id());
        assert!(engine1_a.shares_context_with(&engine1_b));
        assert!(!engine1_a.shares_context_with(&engine2));
    }

    #[test]
    fn navigation_epoch_and_observation_types() {
        let epoch1 = NavigationEpoch::FIRST;
        assert_eq!(epoch1.get(), 1);
        assert_eq!(epoch1.as_u64(), 1);
        let epoch2 = epoch1.next();
        assert_eq!(epoch2.get(), 2);
        assert_eq!(epoch1.to_string(), "epoch:1");

        let event = NavigationEvent::SameDocumentNavigated {
            id: NavigationId::FIRST,
            address: "https://example.test/#section".into(),
        };
        assert_eq!(event.id(), NavigationId::FIRST);

        let obs = NavigationObservation::new(epoch2, event.clone());
        assert_eq!(obs.epoch(), epoch2);
        assert_eq!(obs.event(), &event);
        assert_eq!(obs.into_event(), event);
    }

    #[test]
    fn surface_identity_type() {
        let ident = SurfaceIdentity::new("app.home.v1");
        assert_eq!(ident.as_str(), "app.home.v1");
        assert_eq!(ident.to_string(), "app.home.v1");
        let ident2: SurfaceIdentity = "app.home.v1".into();
        assert_eq!(ident, ident2);
        assert_eq!(ident.as_ref(), "app.home.v1");
    }
}
