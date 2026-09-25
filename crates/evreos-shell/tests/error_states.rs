//! FR-015 error states and presentation integration tests for evreos-shell.
//!
//! Under FR-015 and SC-009:
//! - When navigation fails — an unresolvable address, an untrusted or expired
//!   certificate, an intercepting network, or a request for authentication —
//!   the browser MUST distinguish that failure from a successful load, and MUST
//!   present an error state naming the cause and offering a next step.
//! - Treating a failed load as a successful empty page is a defect.
//! - A failed navigation MUST NEVER replace the page the member was on.
//! - A loading indicator that never resolves is a defect; loads that never resolve
//!   MUST surface the named error state within the shell's stated bound (30s),
//!   driven by an injected clock rather than by waiting on it.
//! - Presentations are resolved from catalogue keys against `evreos_i18n` across
//!   supported languages (`de`, `el`, `en`), never from `LoadError`'s `Display` strings.

use std::time::{Duration, Instant};

use evreos_engine::{Engine, LoadError, NavigationEvent, NavigationId, Request};
use evreos_engine_headless::HeadlessEngine;
use evreos_i18n::Language;
use evreos_shell::app::AppWindowId;
use evreos_shell::error::ShellError;
use evreos_shell::error_presentation::{
    ErrorPresentation, render_error, render_load_error, render_timeout, timeout_as_shell_error,
};
use evreos_shell::tabs::{
    DEFAULT_NAVIGATION_TIMEOUT, MockClock, NavigationState, NavigationTracker, Tab, TabId,
    TabLifecycle,
};

/// Helper to drive one navigation on the engine to quiescence and collect events.
fn drive_navigation<E: Engine>(
    engine: &mut E,
    address: &str,
) -> (NavigationId, Vec<NavigationEvent>) {
    let id = engine.start_navigation(&Request::new(address));
    let mut events = Vec::new();
    while let Some(event) = engine.poll_event() {
        events.push(event);
    }
    (id, events)
}

/// Find a failure event matching `id`.
fn extract_failure(events: &[NavigationEvent], id: NavigationId) -> Option<LoadError> {
    events.iter().find_map(|event| match event {
        NavigationEvent::Failed {
            id: event_id,
            error,
        } if *event_id == id => Some(error.clone()),
        _ => None,
    })
}

/// Helper returning the four FR-015 failure causes for testing.
fn fr015_causes(host: &str) -> [(&'static str, LoadError); 4] {
    [
        (
            "unresolvable",
            LoadError::Unresolvable {
                address: format!("https://{host}/unresolvable"),
            },
        ),
        (
            "certificate",
            LoadError::Certificate {
                address: format!("https://{host}/cert-error"),
                detail: "certificate has expired".into(),
            },
        ),
        (
            "intercepted",
            LoadError::Intercepted {
                address: format!("https://{host}/captive"),
            },
        ),
        (
            "authentication_required",
            LoadError::AuthenticationRequired {
                address: format!("https://{host}/protected"),
            },
        ),
    ]
}

#[test]
fn all_four_fr015_causes_in_all_languages_assert_distinct_named_cause_and_next_step() {
    let host = "test.portal.invalid";
    let causes = fr015_causes(host);
    let languages = [Language::De, Language::El, Language::En];

    for language in languages {
        let mut seen_causes = Vec::new();
        let mut seen_next_steps = Vec::new();

        for (_label, error) in &causes {
            let address = error.address();
            let mut engine = HeadlessEngine::new().with_failure(address, error.clone());

            let (id, events) = drive_navigation(&mut engine, address);
            let failure = extract_failure(&events, id).expect("engine must emit Failed event");
            assert_eq!(&failure, error);

            // Render presentation from the engine failure via catalogue keys
            let presentation = ErrorPresentation::from_load_error(&failure, language)
                .unwrap_or_else(|e| panic!("failed to render {error:?} in {language:?}: {e}"));

            let cause_text = presentation.cause();
            let next_step_text = presentation.next_step();

            // 1. Both cause and next step must be non-empty
            assert!(
                !cause_text.trim().is_empty(),
                "cause in {language:?} must not be empty for {error:?}"
            );
            assert!(
                !next_step_text.trim().is_empty(),
                "next step in {language:?} must not be empty for {error:?}"
            );

            // 2. Cause text must name the problem (address or host)
            let raw_display = format!("{error}");
            assert_ne!(
                cause_text, raw_display,
                "FR-015 presentation must resolve from catalogue keys, NEVER from LoadError's Display string"
            );

            match error {
                LoadError::Unresolvable { .. } => {
                    assert!(
                        cause_text.contains(address),
                        "unresolvable cause in {language:?} must name address: {cause_text}"
                    );
                }
                LoadError::Certificate { .. }
                | LoadError::Intercepted { .. }
                | LoadError::AuthenticationRequired { .. } => {
                    assert!(
                        cause_text.contains(host),
                        "cause for {error:?} in {language:?} must name host: {cause_text}"
                    );
                }
            }

            // 3. HTML rendering includes semantic structure, correct lang, and ARIA attributes
            let html = presentation.to_html();
            assert!(html.contains(&format!("lang=\"{}\"", language.subtag())));
            assert!(html.contains("role=\"alert\""));
            assert!(html.contains("aria-live=\"assertive\""));
            assert!(html.contains("<h1 class=\"error-cause\">"));
            assert!(html.contains("<p class=\"error-next-step next-step\">"));

            // 4. Privacy invariant: log projection carries NO address or host
            let proj = presentation.log_projection();
            let proj_debug = format!("{proj:?}");
            let proj_display = format!("{proj}");
            assert!(
                !proj_debug.contains(address) && !proj_debug.contains(host),
                "log projection must not leak sensitive tokens: {proj_debug}"
            );
            assert!(
                !proj_display.contains(address) && !proj_display.contains(host),
                "log projection must not leak sensitive tokens: {proj_display}"
            );

            // 5. Also verify ShellError direct rendering matches
            let shell_err = ShellError::from(error);
            let direct_pres = render_error(&shell_err, language).expect("render_error");
            assert_eq!(presentation, direct_pres);
            let load_pres = render_load_error(error, language).expect("render_load_error");
            assert_eq!(presentation, load_pres);

            seen_causes.push(cause_text.to_owned());
            seen_next_steps.push(next_step_text.to_owned());
        }

        // Assert 4 distinct named causes per language
        let mut unique_causes = seen_causes.clone();
        unique_causes.sort();
        unique_causes.dedup();
        assert_eq!(
            unique_causes.len(),
            4,
            "two causes produced identical text in {language:?}: {seen_causes:?}"
        );

        // Assert 4 distinct next steps per language
        let mut unique_next_steps = seen_next_steps.clone();
        unique_next_steps.sort();
        unique_next_steps.dedup();
        assert_eq!(
            unique_next_steps.len(),
            4,
            "two next steps produced identical text in {language:?}: {seen_next_steps:?}"
        );
    }
}

#[test]
fn zero_failures_presented_as_successful_blank_pages() {
    let host = "blank-check.invalid";
    let causes = fr015_causes(host);

    for (_label, error) in causes {
        let address = error.address();
        let mut engine = HeadlessEngine::new().with_failure(address, error.clone());

        let (id, events) = drive_navigation(&mut engine, address);

        // Verify Failed event occurred
        assert!(
            extract_failure(&events, id).is_some(),
            "failure event must be present"
        );

        // Verify NO success or commit event occurred
        for event in &events {
            match event {
                NavigationEvent::Committed { id: e_id, .. } => {
                    assert_ne!(
                        *e_id, id,
                        "failed load for {address} must NOT commit to the surface"
                    );
                }
                NavigationEvent::Succeeded { id: e_id } => {
                    assert_ne!(
                        *e_id, id,
                        "failed load for {address} must NOT be reported as Succeeded"
                    );
                }
                _ => {}
            }
        }

        // Engine current page MUST NOT exist (not a successful blank page)
        assert!(
            engine.current().is_none(),
            "failed load for {address} MUST NOT produce a current page in the engine"
        );

        // Shell tab lifecycle verification (FR-015 mutual exclusion)
        let mut tab = Tab::new(
            TabId::FIRST,
            AppWindowId::FIRST,
            0,
            address,
            TabLifecycle::Loading,
            Some(Instant::now()),
        );
        tab.mark_failed(error.clone());

        assert!(
            !tab.is_rendered(),
            "FR-015: a failed load MUST NOT present as a rendered live page"
        );
        assert!(
            !tab.lifecycle().is_live(),
            "tab lifecycle MUST NOT be Live on failure"
        );
        assert!(
            tab.lifecycle().is_failed(),
            "tab lifecycle MUST transition to Failed"
        );
        assert_eq!(tab.lifecycle().failed_error(), Some(&error));
        assert_eq!(tab.loading_started_at(), None);

        // Render presentation in English, German, and Greek: none is blank or empty
        for lang in [Language::En, Language::De, Language::El] {
            let pres = ErrorPresentation::from_load_error(&error, lang).expect("render");
            assert!(
                !pres.cause().trim().is_empty(),
                "cause MUST NOT be empty for {address}"
            );
            assert!(
                !pres.next_step().trim().is_empty(),
                "next step MUST NOT be empty for {address}"
            );
            assert!(
                !pres.presentation().trim().is_empty(),
                "presentation MUST NOT be blank for {address}"
            );
            assert!(
                !pres.to_html().trim().is_empty(),
                "HTML presentation MUST NOT be blank for {address}"
            );
        }
    }
}

#[test]
fn failure_never_replaces_the_page_the_member_was_on() {
    let good_address = "https://good-page.invalid/member-portal";
    let good_title = "Member Portal Home";
    let host = "bad-page.invalid";
    let causes = fr015_causes(host);

    for (_label, error) in causes {
        let bad_address = error.address();

        // Engine starts with a good page already loaded and scripts failure for bad_address
        let mut engine = HeadlessEngine::new()
            .with_page(good_address, good_title)
            .with_failure(bad_address, error.clone());

        // First navigate to the good page
        let (good_id, good_events) = drive_navigation(&mut engine, good_address);
        assert!(
            good_events
                .iter()
                .any(|e| matches!(e, NavigationEvent::Succeeded { id } if *id == good_id)),
            "good page must load successfully"
        );
        assert_eq!(engine.current().map(|p| p.address()), Some(good_address));
        assert_eq!(engine.current().map(|p| p.title()), Some(good_title));

        // Now navigate to the failing address
        let (bad_id, bad_events) = drive_navigation(&mut engine, bad_address);
        assert!(
            extract_failure(&bad_events, bad_id).is_some(),
            "bad page must fail"
        );

        // FR-015 Invariant: The engine MUST STILL be on the good page!
        assert_eq!(
            engine.current().map(|p| p.address()),
            Some(good_address),
            "a failed navigation MUST NOT replace the current page address"
        );
        assert_eq!(
            engine.current().map(|p| p.title()),
            Some(good_title),
            "a failed navigation MUST NOT replace the current page title"
        );
    }
}

#[test]
fn load_that_never_resolves_surfaces_named_error_state_within_stated_bound_driven_by_injected_clock()
 {
    let hanging_address = "https://stalled.portal.invalid/";
    let mut engine = HeadlessEngine::new().with_hanging_load(hanging_address);

    let start = Instant::now();
    let clock = MockClock::new(start);
    let mut tracker = NavigationTracker::with_timeout_bound(clock, DEFAULT_NAVIGATION_TIMEOUT);

    let mut tab = Tab::new(
        TabId::FIRST,
        AppWindowId::FIRST,
        0,
        hanging_address,
        TabLifecycle::Loading,
        Some(start),
    );

    let req = Request::new(hanging_address);
    let id = engine.start_navigation(&req);
    tracker.start_navigation(id, hanging_address.to_owned());
    tab.start_loading(id, start);

    // Initial event from engine is Started; engine then halts with no more events
    let event = engine.poll_event();
    assert!(
        matches!(event, Some(NavigationEvent::Started { id: e_id, ref address }) if e_id == id && address == hanging_address)
    );
    assert_eq!(
        engine.poll_event(),
        None,
        "hanging load produces no further engine events"
    );

    // 1. Advance injected clock by less than the stated bound (e.g. 15s < 30s)
    tracker.clock_mut().advance(Duration::from_secs(15));
    let timed_out_early = tracker.check_timeouts();
    assert!(
        timed_out_early.is_empty(),
        "must not time out before stated 30s bound"
    );
    assert_eq!(
        tab.resolve_in_flight_timeout(&mut tracker),
        None,
        "tab must remain in loading state before 30s bound"
    );
    assert_eq!(
        tab.loading_started_at(),
        Some(start),
        "loading indicator must remain active before bound"
    );
    assert!(matches!(
        tracker.get_state(id),
        Some(NavigationState::Loading { .. })
    ));

    // 2. Advance injected clock past the stated bound (e.g. additional 16s => 31s >= 30s)
    tracker.clock_mut().advance(Duration::from_secs(16));

    // Tab resolve in-flight timeout surfaces named error state and clears loading indicator
    // (SC-009 zero unresolved loading indicators past 30s)
    let resolved = tab.resolve_in_flight_timeout(&mut tracker);
    assert!(
        resolved.is_some(),
        "load that never resolves MUST surface error state after 30s"
    );
    let error_message = resolved.unwrap();
    assert!(
        error_message.contains("timed out after 30s"),
        "error state must name 30s timeout bound: {error_message}"
    );
    assert!(
        error_message.contains("check your network connection or try reloading"),
        "error state must offer actionable next step: {error_message}"
    );
    assert_eq!(
        tab.loading_started_at(),
        None,
        "loading indicator MUST NOT persist indefinitely past 30s bound"
    );

    // Verify tracker state transitioned to Failed
    assert!(matches!(
        tracker.get_state(id),
        Some(NavigationState::Failed { error_message: msg }) if msg == &error_message
    ));

    // FR-015 presentations in de, el, en for timeout state
    let shell_err = timeout_as_shell_error(hanging_address);
    assert_eq!(
        shell_err,
        ShellError::Unresolvable {
            address: hanging_address.to_owned()
        }
    );

    for lang in [Language::De, Language::El, Language::En] {
        let pres = ErrorPresentation::render(&shell_err, lang).expect("render");
        assert!(!pres.cause().trim().is_empty());
        assert!(!pres.next_step().trim().is_empty());

        let timeout_pres = render_timeout(hanging_address, DEFAULT_NAVIGATION_TIMEOUT, lang);
        assert!(!timeout_pres.cause().trim().is_empty());
        assert!(!timeout_pres.next_step().trim().is_empty());
        assert_eq!(timeout_pres.timeout_bound(), DEFAULT_NAVIGATION_TIMEOUT);
        let timeout_html = timeout_pres.to_html();
        assert!(timeout_html.contains("role=\"alert\""));
        assert!(timeout_html.contains(&format!("lang=\"{}\"", lang.subtag())));
    }
}
