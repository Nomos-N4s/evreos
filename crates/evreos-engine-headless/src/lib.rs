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

use std::collections::HashMap;

use evreos_engine::{Engine, LoadError, Page, Request};

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
#[derive(Debug, Default)]
pub struct HeadlessEngine {
    responses: HashMap<String, Response>,
    current: Option<Page>,
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
}

impl Engine for HeadlessEngine {
    fn name(&self) -> &'static str {
        "headless"
    }

    fn load(&mut self, request: &Request) -> Result<Page, LoadError> {
        let address = request.address().to_owned();
        self.loads.push(address.clone());

        match self.responses.get(&address) {
            Some(Response::Page { title }) => {
                let page = Page::new(address, title.clone());
                self.current = Some(page.clone());
                Ok(page)
            }
            Some(Response::Fail(error)) => {
                // FR-015: a failed load does not become a successful empty
                // page. The previously loaded page stays current, because the
                // failure did not replace it.
                Err(error.clone())
            }
            None => Err(LoadError::Unresolvable { address }),
        }
    }

    fn current(&self) -> Option<&Page> {
        self.current.as_ref()
    }
}
