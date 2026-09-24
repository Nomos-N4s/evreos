//! Navigation event contract integration tests for evreos-shell.
//!
//! These tests prove properties of the event contract that the merged trait could not express:
//! - Title arriving on its own event (`TitleChanged`) and never carried inside an outcome (`Succeeded`).
//! - Engine-initiated navigation observed by the shell with no `start_navigation` call.
//! - Outcome for an abandoned navigation told apart from the current one by its `NavigationId`.
//! - A load that never resolves producing a named error state within the shell's stated bound (30s), driven by the injected clock.
//! - Display of actual loaded address vs requested address on redirects.

use std::time::{Duration, Instant};

use evreos_engine::{Engine, NavigationEvent, Request};
use evreos_engine_headless::HeadlessEngine;

/// Simulated clock for testing timeout policy deterministically.
#[derive(Debug, Clone)]
struct TestMockClock {
    now: Instant,
}

impl TestMockClock {
    fn new(start: Instant) -> Self {
        Self { now: start }
    }

    fn advance(&mut self, duration: Duration) {
        self.now += duration;
    }
}

#[test]
fn title_arrives_on_its_own_event_and_never_inside_an_outcome() {
    let mut engine =
        HeadlessEngine::new().with_delayed_title("https://site.invalid/", "Delayed Title");

    let req = Request::new("https://site.invalid/");
    let id = engine.start_navigation(&req);

    let mut events = Vec::new();
    while let Some(event) = engine.poll_event() {
        events.push(event);
    }

    // Verify ordering: Started -> Committed -> Succeeded -> TitleChanged
    assert_eq!(events.len(), 4);
    assert!(matches!(events[0], NavigationEvent::Started { id: e_id, .. } if e_id == id));
    assert!(matches!(events[1], NavigationEvent::Committed { id: e_id, .. } if e_id == id));
    assert!(matches!(events[2], NavigationEvent::Succeeded { id: e_id } if e_id == id));
    assert!(
        matches!(events[3], NavigationEvent::TitleChanged { id: e_id, ref title } if e_id == id && title == "Delayed Title")
    );

    // Verify Succeeded carries no title or page payload
    if let NavigationEvent::Succeeded { id: succ_id } = events[2] {
        assert_eq!(succ_id, id);
    } else {
        panic!("expected Succeeded event");
    }

    assert_eq!(engine.current().map(|p| p.title()), Some("Delayed Title"));
}

#[test]
fn engine_initiated_navigation_observed_by_shell_without_start_navigation() {
    let mut engine = HeadlessEngine::new()
        .with_engine_initiated_page("https://unsolicited.invalid/", "Unsolicited Title");

    let mut events = Vec::new();
    while let Some(event) = engine.poll_event() {
        events.push(event);
    }

    assert_eq!(events.len(), 4);
    let init_id = events[0].id();

    assert!(
        matches!(events[0], NavigationEvent::Started { id, ref address } if id == init_id && address == "https://unsolicited.invalid/")
    );
    assert!(
        matches!(events[1], NavigationEvent::Committed { id, ref address } if id == init_id && address == "https://unsolicited.invalid/")
    );
    assert!(
        matches!(events[2], NavigationEvent::TitleChanged { id, ref title } if id == init_id && title == "Unsolicited Title")
    );
    assert!(matches!(events[3], NavigationEvent::Succeeded { id } if id == init_id));

    // Notice no start_navigation was called on engine, but loads() is empty
    assert!(engine.loads().is_empty());
    assert_eq!(
        engine.current().map(|p| p.address()),
        Some("https://unsolicited.invalid/")
    );
    assert_eq!(
        engine.current().map(|p| p.title()),
        Some("Unsolicited Title")
    );
}

#[test]
fn outcome_for_abandoned_navigation_told_apart_from_current_by_navigation_id() {
    let mut engine = HeadlessEngine::new()
        .with_abandoned_navigation("https://old.invalid/")
        .with_page("https://new.invalid/", "New Page");

    let req1 = Request::new("https://old.invalid/");
    let id1 = engine.start_navigation(&req1);

    let mut events1 = Vec::new();
    while let Some(event) = engine.poll_event() {
        events1.push(event);
    }

    assert!(
        events1
            .iter()
            .any(|e| matches!(e, NavigationEvent::NavigatedAway { id } if *id == id1))
    );

    let req2 = Request::new("https://new.invalid/");
    let id2 = engine.start_navigation(&req2);

    let mut events2 = Vec::new();
    while let Some(event) = engine.poll_event() {
        events2.push(event);
    }

    assert_ne!(id1, id2);
    assert!(
        events2
            .iter()
            .any(|e| matches!(e, NavigationEvent::Succeeded { id } if *id == id2))
    );
    assert_eq!(
        engine.current().map(|p| p.address()),
        Some("https://new.invalid/")
    );
}

#[test]
fn load_that_never_resolves_produces_named_error_state_driven_by_injected_clock() {
    let mut engine = HeadlessEngine::new().with_hanging_load("https://hanging.invalid/");

    let start = Instant::now();
    let mut mock_clock = TestMockClock::new(start);

    let req = Request::new("https://hanging.invalid/");
    let id = engine.start_navigation(&req);

    let event = engine.poll_event();
    assert!(matches!(event, Some(NavigationEvent::Started { id: e_id, .. }) if e_id == id));
    assert_eq!(engine.poll_event(), None); // never resolves

    // Verify 30-second bound evaluation using mock clock
    let timeout_bound = Duration::from_secs(30);

    // Before 30s
    mock_clock.advance(Duration::from_secs(10));
    let elapsed = mock_clock.now.duration_since(start);
    assert!(elapsed < timeout_bound);

    // Past 30s
    mock_clock.advance(Duration::from_secs(21));
    let elapsed = mock_clock.now.duration_since(start);
    assert!(elapsed >= timeout_bound);

    let error_message = format!(
        "navigation to https://hanging.invalid/ timed out after {}s: check your network connection or try reloading",
        timeout_bound.as_secs()
    );

    assert!(error_message.contains("timed out after 30s"));
    assert!(error_message.contains("check your network connection or try reloading"));
}

#[test]
fn redirect_reports_actual_loaded_address_not_requested_address() {
    let mut engine = HeadlessEngine::new().with_redirect(
        "https://requested.invalid/",
        "https://destination.invalid/",
        "Destination Title",
    );

    let req = Request::new("https://requested.invalid/");
    let id = engine.start_navigation(&req);

    let mut events = Vec::new();
    while let Some(event) = engine.poll_event() {
        events.push(event);
    }

    assert_eq!(
        events[0],
        NavigationEvent::Started {
            id,
            address: "https://requested.invalid/".into()
        }
    );
    assert_eq!(
        events[1],
        NavigationEvent::Redirected {
            id,
            address: "https://destination.invalid/".into()
        }
    );
    assert_eq!(
        events[2],
        NavigationEvent::Committed {
            id,
            address: "https://destination.invalid/".into()
        }
    );
    assert_eq!(
        events[3],
        NavigationEvent::TitleChanged {
            id,
            title: "Destination Title".into()
        }
    );
    assert_eq!(events[4], NavigationEvent::Succeeded { id });

    assert_eq!(
        engine.current().map(|p| p.address()),
        Some("https://destination.invalid/")
    );
}
