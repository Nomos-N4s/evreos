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
//! 6. `LoadError::Intercepted` is produced from a shell-supplied/scripted classification rather than synthesised from a platform status.
//! 7. Instances minted from one host share a context and instances from two hosts do not.
//! 8. Surfaces are independently addressable and navigation on one does not affect another.
//! 9. Suspend and resume lose no state the shell can observe.
//! 10. A non-persistent store leaves nothing behind when its surface closes.
//! 11. The per-surface blocked count is observable to the shell, isolated across surfaces, reset on re-navigation, and honors site exemptions and policy replacement.
//! 12. A navigation observation carries an epoch that increments on every change of address, same-document navigation included, defining an FR-018a navigation and bounding an occasion.

use super::{
    CompiledPolicy, DataStoreSelector, Engine, EngineHost, LoadError, NavigationEpoch,
    NavigationEvent, Request, SurfaceState,
};

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
/// - `https://blocked-content.test/` -> Page with subresources matching test blocking policies
pub fn conformance_suite<E: Engine>(make: impl Fn() -> E) {
    test_four_causes_distinguishable(&make);
    test_failed_load_never_becomes_current_page(&make);
    test_address_reported_is_address_that_loaded(&make);
    test_event_ordering_and_navigation_id_correlation(&make);
    test_load_that_never_resolves(&make);
    test_intercepted_from_shell_classification(&make);
    test_surfaces_are_independently_addressable(&make);
    test_suspend_and_resume_preserve_surface_state(&make);
    test_non_persistent_store_leaves_nothing_on_close(&make);
    test_surface_blocked_count_observable(&make);
    test_navigation_epoch_increments_on_every_address_change(&make);
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

/// Invariant 6: `LoadError::Intercepted` is reported from shell-supplied/scripted classification
/// rather than from a mapped platform status.
///
/// Under decisions/0005, platform error codes contain no value denoting interception.
/// No backend may synthesise `Intercepted` from a platform status code; an implementation
/// reporting `Intercepted` does so from a shell-supplied classification (or scripted test classification),
/// leaving the headless engine as the sole producer today.
pub fn test_intercepted_from_shell_classification<E: Engine>(make: &impl Fn() -> E) {
    let mut engine = make();
    let url = "https://intercepted.test/";
    let request = Request::new(url);
    let nav_id = engine.start_navigation(&request);

    let mut events = Vec::new();
    while let Some(event) = engine.poll_event() {
        events.push(event);
    }

    let failed_event = events
        .into_iter()
        .find(|e| matches!(e, NavigationEvent::Failed { .. }));
    assert!(
        failed_event.is_some(),
        "Expected Failed event for intercepted request {url}"
    );

    if let Some(NavigationEvent::Failed { id, error }) = failed_event {
        assert_eq!(id, nav_id, "NavigationId mismatch for {url}");
        assert!(
            matches!(error, LoadError::Intercepted { .. }),
            "Expected LoadError::Intercepted for {url}, got {error:?}"
        );
        assert_eq!(error.address(), url, "Error address mismatch for {url}");
    }
}

/// Invariant 8: Surfaces are independently addressable; navigation on one does not affect another.
///
/// Under FR-001, FR-002, and FR-016, rendering surfaces must be independently addressable.
/// Navigating one surface updates only that surface's current page and document title,
/// leaving other surfaces unaffected.
pub fn test_surfaces_are_independently_addressable<E: Engine>(make: &impl Fn() -> E) {
    let mut engine = make();

    let s1 = engine.create_surface(DataStoreSelector::Persistent);
    let s2 = engine.create_surface(DataStoreSelector::Persistent);
    assert_ne!(s1, s2, "Surfaces must have distinct identifiers");

    engine.activate_surface(s1);
    assert_eq!(engine.active_surface(), Some(s1));
    assert_eq!(engine.surface_state(s1), Some(SurfaceState::Active));

    let req1 = Request::new("https://success.test/");
    let _nav1 = engine.start_surface_navigation(s1, &req1);
    while let Some(_event) = engine.poll_event() {}

    assert_eq!(
        engine.surface_current(s1).map(|p| p.address()),
        Some("https://success.test/")
    );
    assert_eq!(
        engine.surface_current(s1).map(|p| p.title()),
        Some("Success Page")
    );
    assert_eq!(
        engine.current().map(|p| p.address()),
        Some("https://success.test/")
    );
    assert_eq!(
        engine.surface_current(s2),
        None,
        "Surface 2 must not be affected by navigation on surface 1"
    );

    engine.activate_surface(s2);
    assert_eq!(engine.active_surface(), Some(s2));
    assert_eq!(engine.surface_state(s2), Some(SurfaceState::Active));
    assert_eq!(engine.surface_state(s1), Some(SurfaceState::Inactive));

    let req2 = Request::new("https://redirect-source.test/");
    let _nav2 = engine.start_surface_navigation(s2, &req2);
    while let Some(_event) = engine.poll_event() {}

    assert_eq!(
        engine.surface_current(s2).map(|p| p.address()),
        Some("https://redirect-target.test/")
    );
    assert_eq!(
        engine.surface_current(s2).map(|p| p.title()),
        Some("Redirected Page")
    );
    assert_eq!(
        engine.current().map(|p| p.address()),
        Some("https://redirect-target.test/")
    );

    // Verify surface 1's current page was completely unaffected.
    assert_eq!(
        engine.surface_current(s1).map(|p| p.address()),
        Some("https://success.test/"),
        "Surface 1 page address was corrupted by navigation on surface 2"
    );
    assert_eq!(
        engine.surface_current(s1).map(|p| p.title()),
        Some("Success Page"),
        "Surface 1 title was corrupted by navigation on surface 2"
    );

    // Switch back to surface 1; active surface changes without re-navigation.
    engine.activate_surface(s1);
    assert_eq!(engine.active_surface(), Some(s1));
    assert_eq!(
        engine.current().map(|p| p.address()),
        Some("https://success.test/")
    );
    assert_eq!(engine.current().map(|p| p.title()), Some("Success Page"));
}

/// Invariant 9: Suspend and resume lose no state the shell can observe.
///
/// Under FR-002, background or inactive surfaces can be suspended to conserve memory.
/// Suspending and resuming a surface preserves all shell-observable state (such as the
/// current page address and title) without losing state or forcing a re-navigation.
pub fn test_suspend_and_resume_preserve_surface_state<E: Engine>(make: &impl Fn() -> E) {
    let mut engine = make();

    let surface = engine.create_surface(DataStoreSelector::Persistent);
    engine.activate_surface(surface);

    let req = Request::new("https://success.test/");
    let _nav = engine.start_surface_navigation(surface, &req);
    while let Some(_event) = engine.poll_event() {}

    let page_before = engine
        .surface_current(surface)
        .expect("Page should be present after success")
        .clone();
    assert_eq!(page_before.address(), "https://success.test/");
    assert_eq!(page_before.title(), "Success Page");

    // Suspend the surface.
    engine.suspend_surface(surface);
    assert_eq!(
        engine.surface_state(surface),
        Some(SurfaceState::Suspended),
        "Surface state should be Suspended after suspend_surface"
    );

    // Shell-observable state must not be lost while suspended.
    let page_suspended = engine
        .surface_current(surface)
        .expect("Surface current page must be preserved while suspended");
    assert_eq!(
        page_suspended.address(),
        page_before.address(),
        "Address lost during suspend"
    );
    assert_eq!(
        page_suspended.title(),
        page_before.title(),
        "Title lost during suspend"
    );

    // Resume the surface.
    engine.resume_surface(surface);
    assert_eq!(
        engine.surface_state(surface),
        Some(SurfaceState::Active),
        "Surface state should be Active after resume_surface"
    );

    // Shell-observable state must still be preserved after resume.
    let page_resumed = engine
        .surface_current(surface)
        .expect("Surface current page must be preserved after resume");
    assert_eq!(
        page_resumed.address(),
        page_before.address(),
        "Address lost after resume"
    );
    assert_eq!(
        page_resumed.title(),
        page_before.title(),
        "Title lost after resume"
    );
}

/// Invariant 10: A non-persistent store leaves nothing behind when its surface closes.
///
/// Under FR-007, a private surface's data store is isolated and non-persistent.
/// When the surface is closed, its data store is destroyed completely, leaving no
/// browsing traces behind, whereas a persistent store retains data across closure.
pub fn test_non_persistent_store_leaves_nothing_on_close<E: Engine>(make: &impl Fn() -> E) {
    let mut engine = make();

    let s_np = engine.create_surface(DataStoreSelector::NonPersistent);
    let s_p = engine.create_surface(DataStoreSelector::Persistent);

    assert_eq!(
        engine.surface_data_store(s_np),
        Some(DataStoreSelector::NonPersistent)
    );
    assert_eq!(
        engine.surface_data_store(s_p),
        Some(DataStoreSelector::Persistent)
    );

    // Navigate both surfaces so they accumulate data.
    engine.activate_surface(s_np);
    let _ = engine.start_surface_navigation(s_np, &Request::new("https://success.test/"));
    while let Some(_event) = engine.poll_event() {}

    engine.activate_surface(s_p);
    let _ = engine.start_surface_navigation(s_p, &Request::new("https://success.test/"));
    while let Some(_event) = engine.poll_event() {}

    assert!(
        engine.surface_has_retained_data(s_np),
        "Non-persistent surface must have session data while active"
    );
    assert!(
        engine.surface_has_retained_data(s_p),
        "Persistent surface must have session data while active"
    );

    // Close the non-persistent surface.
    engine.close_surface(s_np);

    assert_eq!(
        engine.surface_state(s_np),
        Some(SurfaceState::Closed),
        "Surface state must be Closed after close_surface"
    );
    assert_eq!(
        engine.surface_current(s_np),
        None,
        "Closed surface must have no current page"
    );
    assert!(
        !engine.surface_has_retained_data(s_np),
        "Non-persistent store MUST leave nothing behind when closed (FR-007)"
    );

    // Close the persistent surface and verify persistent data remains.
    engine.close_surface(s_p);
    assert!(
        engine.surface_has_retained_data(s_p),
        "Persistent store must retain persistent data even after surface closes"
    );
}

/// Invariant 11: The per-surface blocked count is observable to the shell.
///
/// Under FR-008 and ADR-0001, tracker and advert blocking is active from first
/// launch. This test proves that:
/// - Blocked counts are observable per surface to the shell.
/// - Surfaces maintain independent blocked counts (surface isolation).
/// - Navigating to a clean page resets the blocked count for that surface.
/// - Installing a replacement policy updates the blocking behaviour.
/// - Exempting a site allows subresources through with zero blocked count.
/// - Revoking an exemption re-enables blocking for subsequent navigations.
pub fn test_surface_blocked_count_observable<E: Engine>(make: &impl Fn() -> E) {
    let mut engine = make();

    let s1 = engine.create_surface(DataStoreSelector::Persistent);
    let s2 = engine.create_surface(DataStoreSelector::Persistent);

    // Initial blocked counts must be zero.
    assert_eq!(engine.surface_blocked_count(s1), 0);
    assert_eq!(engine.surface_blocked_count(s2), 0);
    assert_eq!(engine.blocked_count(s1), 0);

    // Install a compiled policy matching tracker and ad subresources.
    let policy = CompiledPolicy::from_rules("test-block-policy", ["tracker.js", "ad.png"]);
    engine.install_policy(policy);

    // Navigate s1 to a page with subresources.
    let req_blocked = Request::new("https://blocked-content.test/");
    let _ = engine.start_surface_navigation(s1, &req_blocked);
    while let Some(_event) = engine.poll_event() {}

    // s1 must observe the blocked count (2 items: tracker.js and ad.png).
    let count_s1 = engine.surface_blocked_count(s1);
    assert_eq!(
        count_s1, 2,
        "Surface s1 blocked count must be observable to the shell"
    );
    assert_eq!(
        engine.blocked_count(s1),
        2,
        "blocked_count synonym must match surface_blocked_count"
    );

    // s2 was not navigated and must still have 0 blocked count (surface isolation).
    assert_eq!(
        engine.surface_blocked_count(s2),
        0,
        "Surface s2 blocked count must remain 0"
    );

    // Navigate s2 to a clean page.
    let req_clean = Request::new("https://success.test/");
    let _ = engine.start_surface_navigation(s2, &req_clean);
    while let Some(_event) = engine.poll_event() {}

    assert_eq!(
        engine.surface_blocked_count(s2),
        0,
        "Clean page on s2 must have 0 blocked count"
    );
    assert_eq!(
        engine.surface_blocked_count(s1),
        2,
        "s1 blocked count must remain isolated and unaffected by s2 navigation"
    );

    // Exempt the site for s1.
    engine.exempt_site("blocked-content.test");
    assert!(
        engine.is_site_exempt("blocked-content.test"),
        "Site must be reported as exempt after exempt_site"
    );

    // Re-navigating s1 to the exempted site must yield 0 blocked items.
    let _ = engine.start_surface_navigation(s1, &req_blocked);
    while let Some(_event) = engine.poll_event() {}

    assert_eq!(
        engine.surface_blocked_count(s1),
        0,
        "Exempted site must have 0 blocked count"
    );

    // Revoke the site exemption.
    engine.remove_site_exemption("blocked-content.test");
    assert!(
        !engine.is_site_exempt("blocked-content.test"),
        "Site must no longer be exempt after remove_site_exemption"
    );

    // Re-navigating s1 must block items again.
    let _ = engine.start_surface_navigation(s1, &req_blocked);
    while let Some(_event) = engine.poll_event() {}

    assert_eq!(
        engine.surface_blocked_count(s1),
        2,
        "Blocked count must restore after exemption is revoked"
    );

    // Replace policy with a narrower policy matching only tracker.js.
    let narrow_policy = CompiledPolicy::from_rules("tracker-only-policy", ["tracker.js"]);
    engine.install_policy(narrow_policy);

    let _ = engine.start_surface_navigation(s1, &req_blocked);
    while let Some(_event) = engine.poll_event() {}

    assert_eq!(
        engine.surface_blocked_count(s1),
        1,
        "Replaced policy must be active and report new blocked count"
    );

    // Navigating s1 to a clean page resets its blocked count to 0.
    let _ = engine.start_surface_navigation(s1, &req_clean);
    while let Some(_event) = engine.poll_event() {}

    assert_eq!(
        engine.surface_blocked_count(s1),
        0,
        "Navigating to clean page must reset surface blocked count to 0"
    );
}

/// Invariant 12: A navigation observation carries an epoch that increments on every change of
/// address, including same-document navigations, defining an FR-018a navigation and bounding
/// an occasion.
pub fn test_navigation_epoch_increments_on_every_address_change<E: Engine>(make: &impl Fn() -> E) {
    let mut engine = make();
    let surface = engine.create_surface(DataStoreSelector::Persistent);
    let initial_epoch = engine.surface_navigation_epoch(surface);
    assert_eq!(initial_epoch, NavigationEpoch::FIRST);

    // Initial navigation to success page
    let req = Request::new("https://success.test/");
    let nav_id1 = engine.start_surface_navigation(surface, &req);
    let mut obs1 = Vec::new();
    while let Some(obs) = engine.poll_observation() {
        obs1.push(obs);
    }
    let epoch_after_nav = engine.surface_navigation_epoch(surface);
    assert!(
        epoch_after_nav.as_u64() > initial_epoch.as_u64(),
        "Epoch must increment when regular navigation commits"
    );

    let committed_obs = obs1
        .iter()
        .find(|obs| matches!(obs.event(), NavigationEvent::Committed { id, .. } if *id == nav_id1));
    assert!(committed_obs.is_some(), "Must emit Committed observation");
    assert_eq!(
        committed_obs.unwrap().epoch(),
        epoch_after_nav,
        "Committed observation must carry epoch active at commit"
    );

    // Same-document navigation (FR-018a)
    let nav_id2 = engine.navigate_same_document(surface, "https://success.test/#section2");
    let epoch_after_same_doc = engine.surface_navigation_epoch(surface);
    assert!(
        epoch_after_same_doc.as_u64() > epoch_after_nav.as_u64(),
        "Epoch must increment on same-document navigation"
    );

    let mut obs2 = Vec::new();
    while let Some(obs) = engine.poll_observation() {
        obs2.push(obs);
    }
    let same_doc_obs = obs2
        .iter()
        .find(|obs| matches!(obs.event(), NavigationEvent::SameDocumentNavigated { id, address } if *id == nav_id2 && address == "https://success.test/#section2"));
    assert!(
        same_doc_obs.is_some(),
        "Must emit SameDocumentNavigated observation"
    );
    assert_eq!(
        same_doc_obs.unwrap().epoch(),
        epoch_after_same_doc,
        "SameDocumentNavigated observation must carry incremented epoch"
    );
}

/// Invariant 7: Instances minted from one host share a platform context and instances from two hosts do not.
///
/// Under ADR-0001 accepted costs and SC-004 memory constraints, environment/context sharing
/// must be explicit at the seam. An [`EngineHost`] owns the shared platform context, and all
/// [`Engine`] instances minted from that host must share the host's [`ContextId`]
/// (`shares_context_with` is true). Instances minted from two distinct hosts must have
/// distinct context IDs and not share context.
pub fn test_instances_minted_from_one_host_share_context<H: EngineHost>(
    make_host: &impl Fn() -> H,
) {
    let mut host1 = make_host();
    let engine1_a = host1.create_engine();
    let engine1_b = host1.create_engine();

    let mut host2 = make_host();
    let engine2 = host2.create_engine();

    assert_eq!(
        engine1_a.context_id(),
        engine1_b.context_id(),
        "Instances minted from the same host must share a context ID"
    );
    assert_eq!(
        engine1_a.context_id(),
        host1.context_id(),
        "Instance context ID must match the host that minted it"
    );
    assert_ne!(
        engine1_a.context_id(),
        engine2.context_id(),
        "Instances minted from two different hosts must not share a context ID"
    );
    assert_ne!(
        host1.context_id(),
        host2.context_id(),
        "Two distinct hosts must have distinct context IDs"
    );
    assert!(
        engine1_a.shares_context_with(&engine1_b),
        "Instances minted from the same host must return true for shares_context_with"
    );
    assert!(
        !engine1_a.shares_context_with(&engine2),
        "Instances minted from different hosts must return false for shares_context_with"
    );
}

/// Run the host conformance test battery asserting that instances minted from one host
/// share a context and instances from two hosts do not.
pub fn conformance_host_suite<H: EngineHost>(make_host: impl Fn() -> H) {
    test_instances_minted_from_one_host_share_context(&make_host);
}

/// Run the full conformance test battery for both engine navigation invariants and host
/// context sharing using an [`EngineHost`] factory.
pub fn conformance_suite_for_host<H: EngineHost>(make_host: impl Fn() -> H) {
    conformance_suite(|| make_host().create_engine());
    test_instances_minted_from_one_host_share_context(&make_host);
}
