//! SC-009 requires each of the four navigation failures FR-015 enumerates to be
//! exercised on every supported platform, producing an error state that names
//! the cause. These tests are that exercise for the parts that are the shell's
//! rather than the platform's: that each cause is distinguishable, that none of
//! them is reported as a successful load, and that a failure does not silently
//! replace the page the member was on.
//!
//! They run against the headless engine, so they run everywhere — including on
//! a machine with no system webview, which is the point of the second
//! implementation Principle III requires.

use evreos_engine::{Engine, LoadError, NavigationEvent, NavigationId, Page, Request};
use evreos_engine_headless::HeadlessEngine;

fn engine_failing_with(address: &str, error: LoadError) -> HeadlessEngine {
    HeadlessEngine::new().with_failure(address, error)
}

/// Start one navigation and drain the queue to quiescence, returning the id
/// and everything the engine had to say.
fn drive<E: Engine>(engine: &mut E, address: &str) -> (NavigationId, Vec<NavigationEvent>) {
    let id = engine.start_navigation(&Request::new(address));
    let mut events = Vec::new();
    while let Some(event) = engine.poll_event() {
        events.push(event);
    }
    (id, events)
}

fn failure_of(events: &[NavigationEvent], id: NavigationId) -> Option<LoadError> {
    events.iter().find_map(|event| match event {
        NavigationEvent::Failed {
            id: event_id,
            error,
        } if *event_id == id => Some(error.clone()),
        _ => None,
    })
}

/// The other half of every failure assertion. The old trait's `Err` excluded
/// success by its type; an event stream does not, so a failure that also
/// committed or succeeded must be excluded by name — for every cause, not just
/// the one a single test happens to exercise.
fn assert_no_success(events: &[NavigationEvent], id: NavigationId, address: &str) {
    assert!(
        !events.iter().any(|event| matches!(
            event,
            NavigationEvent::Committed { id: event_id, .. }
            | NavigationEvent::Succeeded { id: event_id } if *event_id == id
        )),
        "{address} failed and must neither commit nor succeed"
    );
}

#[test]
fn each_of_the_four_causes_is_distinguishable() {
    let cases = [
        (
            "https://unresolvable.invalid/",
            LoadError::Unresolvable {
                address: "https://unresolvable.invalid/".into(),
            },
        ),
        (
            "https://expired.invalid/",
            LoadError::Certificate {
                address: "https://expired.invalid/".into(),
                detail: "the certificate expired".into(),
            },
        ),
        (
            "https://captive.invalid/",
            LoadError::Intercepted {
                address: "https://captive.invalid/".into(),
            },
        ),
        (
            "https://protected.invalid/",
            LoadError::AuthenticationRequired {
                address: "https://protected.invalid/".into(),
            },
        ),
    ];

    let mut seen = Vec::new();
    for (address, error) in cases {
        let mut engine = engine_failing_with(address, error.clone());
        let (id, events) = drive(&mut engine, address);
        let failure = failure_of(&events, id);
        assert_eq!(
            failure.as_ref(),
            Some(&error),
            "{address} did not fail as scripted"
        );

        assert_no_success(&events, id, address);
        assert!(
            engine.current().is_none(),
            "{address} failed and must not become the current page"
        );

        let described = failure.expect("asserted present above").to_string();
        assert!(
            described.contains(address),
            "the error state must name the address it concerns: {described}"
        );
        seen.push(described);
    }

    // FR-015 exists because a browser that reports every failure identically
    // gives the member nothing to act on. Four causes, four distinct messages.
    let mut distinct = seen.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(
        distinct.len(),
        4,
        "two causes produced the same message: {seen:?}"
    );
}

#[test]
fn a_failed_load_is_never_a_successful_empty_page() {
    // Named verbatim as a defect by FR-015: "Treating a failed load as a
    // successful empty page is a defect."
    let mut engine = engine_failing_with(
        "https://unresolvable.invalid/",
        LoadError::Unresolvable {
            address: "https://unresolvable.invalid/".into(),
        },
    );

    let (id, events) = drive(&mut engine, "https://unresolvable.invalid/");
    assert!(failure_of(&events, id).is_some());
    assert!(
        !events.iter().any(|event| matches!(
            event,
            NavigationEvent::Committed { id: event_id, .. }
            | NavigationEvent::Succeeded { id: event_id } if *event_id == id
        )),
        "a failed navigation must neither commit nor succeed"
    );
    assert!(
        engine.current().is_none(),
        "a failed load must not become the current page"
    );
}

#[test]
fn a_failure_does_not_replace_the_page_the_member_was_on() {
    let mut engine = HeadlessEngine::new()
        .with_page("https://good.invalid/", "Good")
        .with_failure(
            "https://bad.invalid/",
            LoadError::Intercepted {
                address: "https://bad.invalid/".into(),
            },
        );

    let (good_id, good_events) = drive(&mut engine, "https://good.invalid/");
    assert!(
        good_events
            .iter()
            .any(|event| matches!(event, NavigationEvent::Succeeded { id } if *id == good_id)),
        "the scripted page did not load"
    );
    assert_eq!(engine.current().map(|p| p.title()), Some("Good"));

    let (bad_id, bad_events) = drive(&mut engine, "https://bad.invalid/");
    assert!(failure_of(&bad_events, bad_id).is_some());
    assert_no_success(&bad_events, bad_id, "https://bad.invalid/");
    assert_eq!(
        engine.current().map(|p| p.title()),
        Some("Good"),
        "a failed navigation left the member on a page they had not requested"
    );
}

#[test]
fn an_unscripted_address_fails_rather_than_silently_succeeding() {
    // The headless engine's default for an unknown address. A test that forgets
    // to script a page must get a visible failure, not an empty success — the
    // same defect FR-015 forbids, one level down in the test tooling.
    let mut engine = HeadlessEngine::new();
    let (id, events) = drive(&mut engine, "https://unscripted.invalid/");
    assert!(matches!(
        failure_of(&events, id),
        Some(LoadError::Unresolvable { .. })
    ));
    assert_no_success(&events, id, "https://unscripted.invalid/");
    assert!(engine.current().is_none());
}

#[test]
fn current_reflects_a_commit_before_it_is_drained() {
    // The contract's visibility clause: current() reflects emission, not
    // drain — the page changes when a navigation commits, not when the shell
    // reads about it. Nothing is polled here before the assertion, which is
    // the whole point.
    let mut engine = HeadlessEngine::new().with_page("https://site.invalid/", "Site");
    engine.start_navigation(&Request::new("https://site.invalid/"));
    let page = engine.current().expect("committed before any drain");
    assert_eq!(page.address(), "https://site.invalid/");
    assert_eq!(page.title(), "Site");
}

/// An engine holding an `Rc`, so it is not `Send`. Driving it through this
/// file's generic helper is the consumer-side half of the no-`Send` guard: a
/// `Send` bound added to `drive`'s generics stops this compiling, which the
/// engine crate's own guard cannot see.
struct PinnedEngine {
    _pinned: std::rc::Rc<()>,
    queue: Vec<NavigationEvent>,
    next: NavigationId,
}

impl Engine for PinnedEngine {
    fn name(&self) -> &'static str {
        "pinned"
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
fn the_generic_entry_points_carry_no_send_bound() {
    let mut engine = PinnedEngine {
        _pinned: std::rc::Rc::new(()),
        queue: Vec::new(),
        next: NavigationId::FIRST,
    };
    let (id, events) = drive(&mut engine, "https://pinned.invalid/");
    assert_eq!(events.first().map(NavigationEvent::id), Some(id));
}

#[test]
fn the_shell_sees_the_address_that_loaded_not_the_one_requested() {
    // An address bar that shows the request while displaying the response is
    // how a browser lies about where the member is.
    let mut engine = HeadlessEngine::new().with_page("https://site.invalid/", "Site");
    let (id, events) = drive(&mut engine, "https://site.invalid/");
    let committed = events.iter().find_map(|event| match event {
        NavigationEvent::Committed {
            id: event_id,
            address,
        } if *event_id == id => Some(address.clone()),
        _ => None,
    });
    assert_eq!(committed.as_deref(), Some("https://site.invalid/"));
    assert_eq!(
        engine.current().map(|p| p.address()),
        Some("https://site.invalid/")
    );
}

#[test]
fn every_load_the_shell_asks_for_is_observable() {
    // FR-007a bounds what may leave the machine. A test asserting on outbound
    // behaviour needs to see what was actually requested.
    let mut engine = HeadlessEngine::new().with_page("https://a.invalid/", "A");
    let _ = drive(&mut engine, "https://a.invalid/");
    let _ = drive(&mut engine, "https://b.invalid/");
    assert_eq!(engine.loads(), ["https://a.invalid/", "https://b.invalid/"]);
}
