//! The conformance battery for implementations of [`Engine`].
//!
//! This module exports [`conformance_suite`], which tests every invariant stated
//! by the rendering seam contract:
//!
//! 1. The four `LoadError` causes (`Unresolvable`, `Certificate`, `Intercepted`,
//!    `AuthenticationRequired`) are distinguishable and reported correctly.
//! 2. A failed load never replaces or becomes the current page.
//! 3. The address reported in `Committed` and `current()` is the address that actually loaded.
//! 4. Event ordering and `NavigationId` correlation are strictly maintained.
//! 5. A load that never resolves emits `Started` but no outcome event and leaves `current()` unchanged.

use super::{Engine, LoadError, NavigationEvent, Request};

/// Run the full conformance test battery against an engine instance factory.
///
/// `make` must return an `Engine` instance pre-configured to respond to standard conformance addresses:
/// - `https://unresolvable.test/` -> `LoadError::Unresolvable`
/// - `https://cert-error.test/` -> `LoadError::Certificate`
/// - `https://intercepted.test/` -> `LoadError::Intercepted`
/// - `https://auth-required.test/` -> `LoadError::AuthenticationRequired`
/// - `https://success.test/` -> Page titled "Success Page"
/// - `https://redirect-source.test/` -> Redirects to `https://redirect-target.test/` and succeeds
/// - `https://hanging.test/` -> Starts and never emits an outcome event
pub fn conformance_suite<E: Engine>(make: impl Fn() -> E) {
    test_four_causes_distinguishable(&make);
    test_failed_load_never_becomes_current_page(&make);
    test_address_reported_is_address_that_loaded(&make);
    test_event_ordering_and_navigation_id_correlation(&make);
    test_load_that_never_resolves(&make);
}

/// Invariant 1: The four causes of `LoadError` are distinguishable and match expected failure variants.
pub fn test_four_causes_distinguishable<E: Engine>(make: &impl Fn() -> E) {
    let cases = [
        (
            "https://unresolvable.test/",
            LoadError::Unresolvable {
                address: "https://unresolvable.test/".into(),
            },
        ),
        (
            "https://cert-error.test/",
            LoadError::Certificate {
                address: "https://cert-error.test/".into(),
                detail: "invalid self-signed cert".into(),
            },
        ),
        (
            "https://intercepted.test/",
            LoadError::Intercepted {
                address: "https://intercepted.test/".into(),
            },
        ),
        (
            "https://auth-required.test/",
            LoadError::AuthenticationRequired {
                address: "https://auth-required.test/".into(),
            },
        ),
    ];

    for (url, expected_err) in cases {
        let mut engine = make();
        let request = Request::new(url);
        let nav_id = engine.start_navigation(&request);

        let mut events = Vec::new();
        while let Some(event) = engine.poll_event() {
            events.push(event);
        }

        assert!(
            !events.is_empty(),
            "Engine failed to emit any events for {url}"
        );
        assert_eq!(
            events[0],
            NavigationEvent::Started {
                id: nav_id,
                address: url.into()
            },
            "First event for {url} must be Started"
        );

        let failed_event = events
            .into_iter()
            .find(|e| matches!(e, NavigationEvent::Failed { .. }));
        assert!(
            failed_event.is_some(),
            "Expected Failed event for {url}, got none"
        );

        if let Some(NavigationEvent::Failed { id, error }) = failed_event {
            assert_eq!(
                id, nav_id,
                "NavigationId mismatch on Failed event for {url}"
            );
            assert_eq!(
                std::mem::discriminant(&error),
                std::mem::discriminant(&expected_err),
                "Error discriminant mismatch for {url}: got {error:?}, expected {expected_err:?}"
            );
            assert_eq!(error.address(), url, "Error address mismatch for {url}");
        }
    }
}

/// Invariant 2: A failed load never replaces or becomes the `current()` page.
pub fn test_failed_load_never_becomes_current_page<E: Engine>(make: &impl Fn() -> E) {
    let mut engine = make();

    // 1. First navigate to a successful page.
    let req1 = Request::new("https://success.test/");
    let _id1 = engine.start_navigation(&req1);
    while let Some(_event) = engine.poll_event() {}

    let initial_page = engine
        .current()
        .expect("Engine should have a current page after success")
        .clone();
    assert_eq!(initial_page.address(), "https://success.test/");

    // 2. Attempt a failing load.
    let req2 = Request::new("https://unresolvable.test/");
    let _id2 = engine.start_navigation(&req2);
    while let Some(_event) = engine.poll_event() {}

    // 3. Verify `current()` is still the initial successful page.
    let page_after_failure = engine
        .current()
        .expect("Engine should maintain current page after failure");
    assert_eq!(
        page_after_failure.address(),
        initial_page.address(),
        "Failed load replaced current page address"
    );
    assert_eq!(
        page_after_failure.title(),
        initial_page.title(),
        "Failed load replaced current page title"
    );
}

/// Invariant 3: The address reported in `Committed` and `current()` is the address that loaded (after redirect).
pub fn test_address_reported_is_address_that_loaded<E: Engine>(make: &impl Fn() -> E) {
    let mut engine = make();
    let request = Request::new("https://redirect-source.test/");
    let nav_id = engine.start_navigation(&request);

    let mut events = Vec::new();
    while let Some(event) = engine.poll_event() {
        events.push(event);
    }

    let redirected_event = events
        .iter()
        .find(|e| matches!(e, NavigationEvent::Redirected { .. }));
    assert!(
        redirected_event.is_some(),
        "Expected Redirected event for redirect request"
    );
    if let Some(NavigationEvent::Redirected { id, address }) = redirected_event {
        assert_eq!(*id, nav_id);
        assert_eq!(address, "https://redirect-target.test/");
    }

    let committed_event = events
        .iter()
        .find(|e| matches!(e, NavigationEvent::Committed { .. }));
    assert!(
        committed_event.is_some(),
        "Expected Committed event after redirect"
    );
    if let Some(NavigationEvent::Committed { id, address }) = committed_event {
        assert_eq!(*id, nav_id);
        assert_eq!(address, "https://redirect-target.test/");
    }

    let current = engine
        .current()
        .expect("Current page must be set after commit");
    assert_eq!(
        current.address(),
        "https://redirect-target.test/",
        "Reported current address must be the target address that loaded"
    );
}

/// Invariant 4: Event ordering and `NavigationId` correlation are strictly enforced.
pub fn test_event_ordering_and_navigation_id_correlation<E: Engine>(make: &impl Fn() -> E) {
    let mut engine = make();
    let request = Request::new("https://success.test/");
    let nav_id = engine.start_navigation(&request);

    let mut events = Vec::new();
    while let Some(event) = engine.poll_event() {
        events.push(event);
    }

    assert!(
        events.len() >= 3,
        "Expected at least Started, Committed, Succeeded/Title"
    );

    // All events must carry `nav_id`.
    for event in &events {
        assert_eq!(
            event.id(),
            nav_id,
            "Event {event:?} carries mismatched NavigationId"
        );
    }

    // Verify ordering: Started -> Committed -> Succeeded.
    let started_idx = events
        .iter()
        .position(|e| matches!(e, NavigationEvent::Started { .. }))
        .expect("Missing Started event");
    let committed_idx = events
        .iter()
        .position(|e| matches!(e, NavigationEvent::Committed { .. }))
        .expect("Missing Committed event");
    let succeeded_idx = events
        .iter()
        .position(|e| matches!(e, NavigationEvent::Succeeded { .. }))
        .expect("Missing Succeeded event");

    assert!(
        started_idx < committed_idx,
        "Started must precede Committed"
    );
    assert!(
        committed_idx < succeeded_idx,
        "Committed must precede Succeeded"
    );
}

/// Invariant 5: A load that never resolves emits `Started` but no outcome event and does not alter `current()`.
pub fn test_load_that_never_resolves<E: Engine>(make: &impl Fn() -> E) {
    let mut engine = make();
    let request = Request::new("https://hanging.test/");
    let nav_id = engine.start_navigation(&request);

    let mut events = Vec::new();
    while let Some(event) = engine.poll_event() {
        events.push(event);
    }

    assert_eq!(
        events.len(),
        1,
        "Hanging load must emit exactly Started event and no outcome"
    );
    assert_eq!(
        events[0],
        NavigationEvent::Started {
            id: nav_id,
            address: "https://hanging.test/".into()
        }
    );

    assert!(
        engine.current().is_none(),
        "Hanging load must not set or change current page"
    );
}
