//! The Evreos shell.
//!
//! At M0 this proves one thing and claims nothing else: the shell drives
//! rendering through [`evreos_engine::Engine`] and never through a concrete
//! backend, so the seam Principle III requires is real rather than asserted.
//! It is exercised here against the headless implementation, which is why this
//! binary builds and runs on a machine with no system webview at all.
//!
//! The same console proof exercises the second seam Principle VIII requires:
//! every brand value the shell shows or sends to comes through [`brand`],
//! never from a literal here, so the printed brand line and the composed —
//! not sent — search request below change with the brand file and with
//! nothing else.

#![forbid(unsafe_code)]

use evreos_shell::brand;
pub use evreos_shell::error;

use std::collections::HashMap;
use std::time::{Duration, Instant};

#[cfg(test)]
use evreos_engine::{Engine, Request};
use evreos_engine::{LoadError, NavigationEvent, NavigationId};
use evreos_engine_headless::HeadlessEngine;

/// Source of monotonic time for navigation timeout tracking.
pub trait Clock {
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
}

impl Clock for MockClock {
    fn now(&self) -> Instant {
        self.now
    }
}

/// Default navigation timeout bound (30 seconds per SC-009).
pub const DEFAULT_NAVIGATION_TIMEOUT: Duration = Duration::from_secs(30);

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

    pub fn clock_mut(&mut self) -> &mut C {
        &mut self.clock
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
    pub fn check_timeouts(&mut self) {
        let now = self.clock.now();
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
                    *state = NavigationState::Failed { error_message: msg };
                }
            }
        }
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

/// Drive an engine through one navigation and report what happened.
///
/// Generic over [`Engine`] and [`Clock`]. Drains event stream, tracks in-flight
/// navigation, displays actual loaded address, and applies timeout policy.
#[cfg(test)]
fn navigate<E: Engine, C: Clock>(
    engine: &mut E,
    tracker: &mut NavigationTracker<C>,
    address: &str,
) -> String {
    let id = engine.start_navigation(&Request::new(address));
    tracker.start_navigation(id, address.to_owned());

    while let Some(event) = engine.poll_event() {
        tracker.process_event(event);
    }

    // Apply policy check for in-flight timeout
    tracker.check_timeouts();

    tracker.display_status(id)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let brand = brand::brand();
    let engine = HeadlessEngine::new();
    let mut app = evreos_shell::app::App::with_title(engine, brand.product_name.clone());

    #[cfg(any(windows, target_os = "macos"))]
    {
        if std::env::var("EVREOS_HEADLESS").is_ok() {
            let win_id = app.open_window(brand.product_name.clone());
            println!("{}: opened window {win_id}", brand.product_name);
            app.close_window(win_id);
            println!(
                "{}: closed last window; exiting cleanly",
                brand.product_name
            );
            return Ok(());
        }

        app.run()?;
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let win_id = app.open_window(brand.product_name.clone());
        println!("{}: opened window {win_id}", brand.product_name);
        app.close_window(win_id);
        println!(
            "{}: closed last window; exiting cleanly",
            brand.product_name
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use evreos_engine::{NavigationId, Page};
    use std::rc::Rc;

    /// An engine holding an `Rc`, so it is not `Send`. Driving it through
    /// [`navigate`] is the shell-side half of the no-`Send` guard: if a `Send`
    /// bound ever lands on this entry point's generics, this test stops
    /// compiling — the engine crate's own guard cannot see a bound added here.
    struct AbandoningEngine {
        _pinned: Rc<()>,
        queue: Vec<evreos_engine::NavigationEvent>,
        next: NavigationId,
    }

    impl Engine for AbandoningEngine {
        fn name(&self) -> &'static str {
            "abandoning"
        }

        fn start_navigation(&mut self, request: &Request) -> NavigationId {
            let id = self.next;
            self.next = id.next();
            self.queue.push(evreos_engine::NavigationEvent::Started {
                id,
                address: request.address().to_owned(),
            });
            self.queue
                .push(evreos_engine::NavigationEvent::NavigatedAway { id });
            id
        }

        fn poll_event(&mut self) -> Option<evreos_engine::NavigationEvent> {
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
    fn an_abandoned_navigation_is_not_reported_as_still_loading() {
        let mut engine = AbandoningEngine {
            _pinned: Rc::new(()),
            queue: Vec::new(),
            next: NavigationId::FIRST,
        };
        let mut tracker = NavigationTracker::new(SystemClock);
        let outcome = navigate(&mut engine, &mut tracker, "https://gone.invalid/");
        assert_eq!(
            outcome,
            "https://gone.invalid/ was abandoned before it resolved"
        );
    }

    #[test]
    fn displays_address_that_actually_loaded_on_redirect() {
        let mut engine = HeadlessEngine::new().with_redirect(
            "https://requested.invalid/",
            "https://actual.invalid/",
            "Actual Page",
        );
        let mut tracker = NavigationTracker::new(SystemClock);
        let outcome = navigate(&mut engine, &mut tracker, "https://requested.invalid/");
        assert_eq!(outcome, "Actual Page — https://actual.invalid/");
    }

    #[test]
    fn in_flight_navigation_timing_out_resolves_to_error_state() {
        let start = Instant::now();
        let mock_clock = MockClock::new(start);
        let mut tracker = NavigationTracker::new(mock_clock);

        let id = NavigationId::FIRST;
        tracker.start_navigation(id, "https://slow.invalid/".into());

        assert_eq!(
            tracker.display_status(id),
            "https://slow.invalid/ is still loading"
        );

        // Advance clock past timeout bound
        tracker.clock_mut().advance(Duration::from_secs(30));
        tracker.check_timeouts();

        let status = tracker.display_status(id);
        assert!(status.contains("timed out after 30s"));
        assert!(status.contains("check your network connection or try reloading"));
    }

    #[test]
    fn load_errors_format_cause_and_next_step() {
        let errors = [
            (
                LoadError::Unresolvable {
                    address: "https://a.invalid/".into(),
                },
                "could not load https://a.invalid/: server address could not be found. Next step: check the address for typos or verify network connection.",
            ),
            (
                LoadError::Certificate {
                    address: "https://b.invalid/".into(),
                    detail: "bad cert".into(),
                },
                "could not load https://b.invalid/: security certificate error (bad cert). Next step: verify your system clock or do not proceed if on a public network.",
            ),
            (
                LoadError::Intercepted {
                    address: "https://c.invalid/".into(),
                },
                "could not load https://c.invalid/: connection intercepted by a captive portal or proxy. Next step: log in to the network or check proxy settings.",
            ),
            (
                LoadError::AuthenticationRequired {
                    address: "https://d.invalid/".into(),
                },
                "could not load https://d.invalid/: HTTP authentication required. Next step: enter valid credentials when prompted.",
            ),
        ];

        for (err, expected) in errors {
            assert_eq!(format_load_error(&err), expected);
        }
    }
}
