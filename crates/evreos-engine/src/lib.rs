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
/// compares and hashes it. There is no public constructor from an integer:
/// a shell that could forge an id could ask about a navigation no engine
/// started, and an engine with platform navigation identifiers of its own
/// maps them to these rather than exposing them.
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
///   the response and the moment [`Engine::current`] changes: by the time the
///   shell drains it, `current()` is the new page, at the committed address,
///   with an empty title until the first `TitleChanged`. The address it carries
///   is the one that actually loaded — after redirects, the final one — and is
///   what the shell displays.
/// - Each navigation ends with at most one of `Succeeded`, `Failed` or
///   `NavigatedAway` — or never ends, which is the load that never resolves.
///   The bound on how long the shell waits is the shell's policy under SC-009,
///   deliberately not an engine invariant, so no engine event expresses it.
/// - `Succeeded` only ever follows `Committed`. It carries no page and no
///   title: the page is `current()`, and the title arrives on `TitleChanged`
///   — before or after `Succeeded`, with no ordering promised between them.
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
///   the engine updates that page's title before the event is drained. A
///   `TitleChanged` for a superseded navigation never alters the current page.
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
            | Self::Succeeded { id }
            | Self::Failed { id, .. }
            | Self::TitleChanged { id, .. }
            | Self::NavigatedAway { id } => *id,
        }
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
    /// `Send` bound ever lands on the engine path, this implementation stops
    /// compiling and the bound is caught here rather than by the first
    /// platform backend that cannot satisfy it.
    struct ThreadAffine {
        _pinned: Rc<()>,
        queue: Vec<NavigationEvent>,
        next: NavigationId,
    }

    impl Engine for ThreadAffine {
        fn name(&self) -> &'static str {
            "thread-affine"
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

    #[test]
    fn the_engine_path_carries_no_send_bound() {
        let mut engine = ThreadAffine {
            _pinned: Rc::new(()),
            queue: Vec::new(),
            next: NavigationId::FIRST,
        };
        let id = engine.start_navigation(&Request::new("https://a.invalid/"));
        assert_eq!(engine.poll_event().map(|event| event.id()), Some(id));
        assert_eq!(engine.poll_event(), None);
    }
}
