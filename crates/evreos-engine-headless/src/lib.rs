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

use std::collections::{HashMap, VecDeque};

use evreos_engine::{Engine, LoadError, NavigationEvent, NavigationId, Page, Request};

/// What the headless engine will do when asked for a given address.
#[derive(Debug, Clone)]
pub enum Response {
    /// Serve a page with this title.
    Page { title: String },
    /// Fail with this cause.
    Fail(LoadError),
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
#[derive(Debug, Default)]
pub struct HeadlessEngine {
    responses: HashMap<String, Response>,
    queue: VecDeque<NavigationEvent>,
    next_id: Option<NavigationId>,
    current: Option<Page>,
    current_nav: Option<NavigationId>,
    loads: Vec<String>,
}

impl HeadlessEngine {
    pub fn new() -> Self {
        Self::default()
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
}

impl Engine for HeadlessEngine {
    fn name(&self) -> &'static str {
        "headless"
    }

    fn start_navigation(&mut self, request: &Request) -> NavigationId {
        let address = request.address().to_owned();
        self.loads.push(address.clone());
        let id = self.mint();

        self.queue.push_back(NavigationEvent::Started {
            id,
            address: address.clone(),
        });

        match self.responses.get(&address).cloned() {
            Some(Response::Page { title }) => {
                // Commit is when the current page changes; the title arrives on
                // its own event afterwards, and the page carries it from the
                // moment that event exists, drained or not.
                self.queue.push_back(NavigationEvent::Committed {
                    id,
                    address: address.clone(),
                });
                self.current = Some(Page::new(address, ""));
                self.current_nav = Some(id);

                self.queue.push_back(NavigationEvent::TitleChanged {
                    id,
                    title: title.clone(),
                });
                if self.current_nav == Some(id) {
                    if let Some(page) = &self.current {
                        self.current = Some(Page::new(page.address().to_owned(), title));
                    }
                }

                self.queue.push_back(NavigationEvent::Succeeded { id });
            }
            Some(Response::Fail(error)) => {
                // FR-015: a failed load does not become a successful empty
                // page. The previously loaded page stays current, because the
                // failure did not replace it.
                self.queue.push_back(NavigationEvent::Failed { id, error });
            }
            None => {
                self.queue.push_back(NavigationEvent::Failed {
                    id,
                    error: LoadError::Unresolvable { address },
                });
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
