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

use evreos_engine_headless::HeadlessEngine;

#[cfg(test)]
use std::time::{Duration, Instant};

#[cfg(test)]
use evreos_engine::{Engine, LoadError, Request};

pub use evreos_shell::tabs::{
    Clock, DEFAULT_NAVIGATION_TIMEOUT, MockClock, NavigationState, NavigationTracker, SystemClock,
    format_load_error,
};

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
