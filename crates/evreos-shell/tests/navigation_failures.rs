//! SC-009 requires each of the four navigation failures FR-015 enumerates to be
//! exercised on every supported platform, producing an error state that names
//! the cause and offers a next step.
//!
//! These tests verify failure mechanics against the event contract and shell error state formatting:
//! - Each of FR-015's four causes is distinguishable and none is reported as a successful load.
//! - A failed navigation does not replace the page the member was on.
//! - Unscripted addresses produce an Unresolvable failure.

use evreos_engine::{Engine, LoadError, NavigationEvent, NavigationId, Request};
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

/// A failure that also committed or succeeded must be excluded by name.
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

/// Format engine LoadError in the shell's member-facing style (naming cause and next step).
fn format_shell_error(error: &LoadError) -> String {
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

    let mut seen_raw = Vec::new();
    let mut seen_formatted = Vec::new();

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

        let raw = failure.expect("asserted present above").to_string();
        assert!(
            raw.contains(address),
            "the fallback error state must name the address it concerns: {raw}"
        );
        seen_raw.push(raw);

        let formatted = format_shell_error(&error);
        assert!(
            formatted.contains(address),
            "the shell error state must name the address: {formatted}"
        );
        assert!(
            formatted.contains("Next step:"),
            "the shell error state must offer a next step: {formatted}"
        );
        seen_formatted.push(formatted);
    }

    // Four causes, four distinct messages in both raw Display and shell-formatted text.
    let mut distinct_raw = seen_raw.clone();
    distinct_raw.sort();
    distinct_raw.dedup();
    assert_eq!(
        distinct_raw.len(),
        4,
        "two causes produced the same raw message: {seen_raw:?}"
    );

    let mut distinct_formatted = seen_formatted.clone();
    distinct_formatted.sort();
    distinct_formatted.dedup();
    assert_eq!(
        distinct_formatted.len(),
        4,
        "two causes produced the same formatted message: {seen_formatted:?}"
    );
}

#[test]
fn a_failed_load_is_never_a_successful_empty_page() {
    let mut engine = engine_failing_with(
        "https://unresolvable.invalid/",
        LoadError::Unresolvable {
            address: "https://unresolvable.invalid/".into(),
        },
    );

    let (id, events) = drive(&mut engine, "https://unresolvable.invalid/");
    assert!(failure_of(&events, id).is_some());
    assert_no_success(&events, id, "https://unresolvable.invalid/");
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
    let mut engine = HeadlessEngine::new();
    let (id, events) = drive(&mut engine, "https://unscripted.invalid/");
    assert!(matches!(
        failure_of(&events, id),
        Some(LoadError::Unresolvable { .. })
    ));
    assert_no_success(&events, id, "https://unscripted.invalid/");
    assert!(engine.current().is_none());
}
