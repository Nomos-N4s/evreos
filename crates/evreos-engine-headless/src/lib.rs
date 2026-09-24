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
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use evreos_engine::{
    ContextId, Engine, EngineHost, LoadError, NavigationEvent, NavigationId, Page, Request,
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
    queue: VecDeque<NavigationEvent>,
    next_id: Option<NavigationId>,
    current: Option<Page>,
    current_nav: Option<NavigationId>,
    loads: Vec<String>,
    context: Option<Rc<SharedHostContext>>,
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
            current: None,
            current_nav: None,
            loads: Vec::new(),
            context: None,
        }
    }

    fn from_shared_context(context: Rc<SharedHostContext>) -> Self {
        Self {
            context_id: context.id,
            responses: HashMap::new(),
            queue: VecDeque::new(),
            next_id: None,
            current: None,
            current_nav: None,
            loads: Vec::new(),
            context: Some(context),
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
        let addr = address.into();
        let id = self.mint();
        self.queue
            .push_back(NavigationEvent::Started { id, address: addr });
        self.process_steps(id, steps);
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

    fn process_steps(&mut self, id: NavigationId, steps: impl IntoIterator<Item = ScriptStep>) {
        for step in steps {
            match step {
                ScriptStep::Redirect { address } => {
                    self.queue
                        .push_back(NavigationEvent::Redirected { id, address });
                }
                ScriptStep::Commit { address } => {
                    self.queue.push_back(NavigationEvent::Committed {
                        id,
                        address: address.clone(),
                    });
                    self.current = Some(Page::new(address, ""));
                    self.current_nav = Some(id);
                }
                ScriptStep::Title { title } => {
                    self.queue.push_back(NavigationEvent::TitleChanged {
                        id,
                        title: title.clone(),
                    });
                    if self.current_nav == Some(id) {
                        if let Some(page) = &self.current {
                            self.current = Some(Page::new(page.address().to_owned(), title));
                        }
                    }
                }
                ScriptStep::Succeed => {
                    self.queue.push_back(NavigationEvent::Succeeded { id });
                }
                ScriptStep::Fail(error) => {
                    self.queue.push_back(NavigationEvent::Failed { id, error });
                }
                ScriptStep::NavigateAway => {
                    self.queue.push_back(NavigationEvent::NavigatedAway { id });
                }
            }
        }
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
        let address = request.address().to_owned();
        self.loads.push(address.clone());
        if let Some(ctx) = &self.context {
            ctx.loads.borrow_mut().push(address.clone());
        }
        let id = self.mint();

        self.queue.push_back(NavigationEvent::Started {
            id,
            address: address.clone(),
        });

        let response = self.responses.get(&address).cloned().or_else(|| {
            self.context
                .as_ref()
                .and_then(|c| c.responses.borrow().get(&address).cloned())
        });

        match response {
            Some(Response::Page { title }) => {
                self.process_steps(
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
                self.process_steps(id, [ScriptStep::Fail(error)]);
            }
            Some(Response::Sequence(steps)) => {
                self.process_steps(id, steps);
            }
            None => {
                self.process_steps(id, [ScriptStep::Fail(LoadError::Unresolvable { address })]);
            }
        }

        id
    }

    fn poll_event(&mut self) -> Option<NavigationEvent> {
        self.queue.pop_front()
    }

    fn current(&self) -> Option<&Page> {
        self.current.as_ref()
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
}
