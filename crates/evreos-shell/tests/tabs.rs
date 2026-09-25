//! Tab and window model integration tests for evreos-shell.
//!
//! Under FR-001, FR-002, FR-015, FR-018a, and SC-009:
//! - Asserts that a failed load and a rendered page are mutually exclusive.
//! - Asserts that reorder is stable across suspend and resume.
//! - Asserts that a tab still loading past the bound resolves through the shell's
//!   existing in-flight policy and its injectable clock source rather than through
//!   any LoadError variant or a second timeout implementation.
//! - Asserts that same-document navigation increments navigation epoch.
//! - Asserts that session restoration defers loading until first activation.

use std::time::{Duration, Instant};

use evreos_engine::{Engine, LoadError, NavigationEpoch, NavigationId, Request};
use evreos_engine_headless::HeadlessEngine;
use evreos_shell::app::AppWindowId;
use evreos_shell::store::{BookmarkStore, HistoryStore, WindowKind};
use evreos_shell::suggest::{SuggestionIndex, SuggestionSource};
use evreos_shell::tabs::{
    MockClock, NavigationState, NavigationTracker, Tab, TabId, TabLifecycle, WindowTabs,
};

#[test]
fn failed_load_and_rendered_page_are_mutually_exclusive() {
    let clock = MockClock::new(Instant::now());
    let mut window = WindowTabs::new(AppWindowId::FIRST, WindowKind::Normal);

    // 1. Initial tab starts Loading
    let tab_id = window.open_tab("https://example.invalid/", &clock);
    let tab = window.get_tab(tab_id).expect("tab must exist");
    assert!(tab.lifecycle().is_loading());
    assert!(!tab.is_rendered(), "loading tab is not yet rendered");

    // 2. Tab succeeds -> Live (rendered)
    let tab_mut = window.get_tab_mut(tab_id).expect("tab mut must exist");
    tab_mut.mark_succeeded("https://example.invalid/");
    assert!(tab_mut.lifecycle().is_live());
    assert!(tab_mut.is_rendered(), "live tab is actively rendered");
    assert!(!tab_mut.lifecycle().is_failed());

    // 3. Tab fails with LoadError -> Failed (mutually exclusive with rendered page)
    let error = LoadError::Unresolvable {
        address: "https://fail.invalid/".to_owned(),
    };
    tab_mut.mark_failed(error.clone());

    assert!(tab_mut.lifecycle().is_failed());
    assert_eq!(tab_mut.lifecycle().failed_error(), Some(&error));
    assert!(
        !tab_mut.is_rendered(),
        "FR-015: a failed load MUST NOT present as a rendered live page"
    );
}

#[test]
fn reorder_is_stable_across_suspend_and_resume() {
    let clock = MockClock::new(Instant::now());
    let mut window = WindowTabs::new(AppWindowId::FIRST, WindowKind::Normal);

    // Open four tabs
    let t0 = window.open_tab("https://tab0.invalid/", &clock);
    let t1 = window.open_tab("https://tab1.invalid/", &clock);
    let t2 = window.open_tab("https://tab2.invalid/", &clock);
    let t3 = window.open_tab("https://tab3.invalid/", &clock);

    // Mark all four as Live
    for id in [t0, t1, t2, t3] {
        let tab = window.get_tab_mut(id).expect("tab exists");
        let addr = tab.displayed_address().to_owned();
        tab.mark_succeeded(&addr);
    }

    // Verify initial positions: [t0, t1, t2, t3]
    assert_eq!(window.tabs()[0].id(), t0);
    assert_eq!(window.tabs()[1].id(), t1);
    assert_eq!(window.tabs()[2].id(), t2);
    assert_eq!(window.tabs()[3].id(), t3);

    // Reorder: Move t0 to position 2 -> expected order: [t1, t2, t0, t3]
    window
        .reorder_tab(t0, 2)
        .expect("reorder t0 to 2 should succeed");

    assert_eq!(window.tabs()[0].id(), t1);
    assert_eq!(window.tabs()[0].position(), 0);
    assert_eq!(window.tabs()[1].id(), t2);
    assert_eq!(window.tabs()[1].position(), 1);
    assert_eq!(window.tabs()[2].id(), t0);
    assert_eq!(window.tabs()[2].position(), 2);
    assert_eq!(window.tabs()[3].id(), t3);
    assert_eq!(window.tabs()[3].position(), 3);

    // Suspend t0 and t2
    window.suspend_tab(t0).expect("suspend t0");
    window.suspend_tab(t2).expect("suspend t2");
    assert!(window.get_tab(t0).expect("t0").lifecycle().is_suspended());
    assert!(window.get_tab(t2).expect("t2").lifecycle().is_suspended());

    // Ordering is unchanged during suspension
    assert_eq!(window.tabs()[0].id(), t1);
    assert_eq!(window.tabs()[1].id(), t2);
    assert_eq!(window.tabs()[2].id(), t0);
    assert_eq!(window.tabs()[3].id(), t3);

    // Resume t0, suspend t1, resume t2
    window.resume_tab(t0).expect("resume t0");
    window.suspend_tab(t1).expect("suspend t1");
    window.resume_tab(t2).expect("resume t2");

    // Stable ordering invariant: order MUST remain [t1, t2, t0, t3]
    assert_eq!(window.tabs()[0].id(), t1);
    assert_eq!(window.tabs()[0].position(), 0);
    assert_eq!(window.tabs()[1].id(), t2);
    assert_eq!(window.tabs()[1].position(), 1);
    assert_eq!(window.tabs()[2].id(), t0);
    assert_eq!(window.tabs()[2].position(), 2);
    assert_eq!(window.tabs()[3].id(), t3);
    assert_eq!(window.tabs()[3].position(), 3);
}

#[test]
fn tab_loading_past_bound_resolves_through_existing_inflight_policy_and_injected_clock() {
    let start = Instant::now();
    let mut clock = MockClock::new(start);
    let mut tracker = NavigationTracker::new(clock.clone());
    let mut window = WindowTabs::new(AppWindowId::FIRST, WindowKind::Normal);

    // Open tab and initiate in-flight navigation
    let tab_id = window.open_tab("https://stalled.invalid/", &clock);
    let nav_id = NavigationId::FIRST;
    tracker.start_navigation(nav_id, "https://stalled.invalid/".to_owned());

    let tab = window.get_tab_mut(tab_id).expect("tab exists");
    tab.start_loading(nav_id, start);
    assert!(tab.lifecycle().is_loading());
    assert!(tab.loading_started_at().is_some());

    // Advance clock by 15 seconds (under 30s bound)
    clock.advance(Duration::from_secs(15));
    tracker.clock_mut().advance(Duration::from_secs(15));

    let timed_out = window.check_in_flight_timeouts(&mut tracker);
    assert!(
        timed_out.is_empty(),
        "under 30 seconds, navigation must remain loading"
    );
    assert!(
        window
            .get_tab(tab_id)
            .expect("tab exists")
            .loading_started_at()
            .is_some()
    );

    // Advance clock past 30-second bound (e.g. 16 more seconds -> 31s total)
    clock.advance(Duration::from_secs(16));
    tracker.clock_mut().advance(Duration::from_secs(16));

    // Resolve through the shell's existing in-flight policy
    let timed_out = window.check_in_flight_timeouts(&mut tracker);
    assert_eq!(timed_out.len(), 1);
    assert_eq!(timed_out[0].0, tab_id);
    assert!(timed_out[0].1.contains("timed out after 30s"));
    assert!(
        timed_out[0]
            .1
            .contains("check your network connection or try reloading")
    );

    // Verify loading indicator is cleared per SC-009
    let resolved_tab = window.get_tab(tab_id).expect("tab exists");
    assert_eq!(
        resolved_tab.loading_started_at(),
        None,
        "loading indicator must resolve past 30s bound"
    );

    // Verify tracker state is Failed with the timeout message (no LoadError variant)
    assert!(matches!(
        tracker.get_state(nav_id),
        Some(NavigationState::Failed { error_message }) if error_message.contains("timed out after 30s")
    ));
}

#[test]
fn same_document_navigation_increments_navigation_epoch() {
    let clock = MockClock::new(Instant::now());
    let mut window = WindowTabs::new(AppWindowId::FIRST, WindowKind::Normal);

    let tab_id = window.open_tab("https://article.invalid/post", &clock);
    let tab = window.get_tab_mut(tab_id).expect("tab exists");

    assert_eq!(tab.navigation_epoch(), NavigationEpoch::FIRST);
    assert_eq!(tab.displayed_address(), "https://article.invalid/post");

    // Same-document navigation (anchor hash update)
    tab.same_document_navigate("https://article.invalid/post#section-1", None);
    assert_eq!(tab.navigation_epoch(), NavigationEpoch::new(2));
    assert_eq!(
        tab.displayed_address(),
        "https://article.invalid/post#section-1"
    );

    // Second same-document navigation
    tab.same_document_navigate("https://article.invalid/post#section-2", None);
    assert_eq!(tab.navigation_epoch(), NavigationEpoch::new(3));
    assert_eq!(
        tab.displayed_address(),
        "https://article.invalid/post#section-2"
    );

    // Transitioning to Suspended and Live does NOT increment epoch
    tab.mark_succeeded("https://article.invalid/post#section-2");
    assert_eq!(tab.navigation_epoch(), NavigationEpoch::new(3));

    tab.suspend().expect("suspend");
    assert_eq!(
        tab.navigation_epoch(),
        NavigationEpoch::new(3),
        "suspend must not change epoch"
    );

    tab.resume().expect("resume");
    assert_eq!(
        tab.navigation_epoch(),
        NavigationEpoch::new(3),
        "resume must not change epoch"
    );
}

#[test]
fn session_restoration_defers_page_load_until_activation() {
    let clock = MockClock::new(Instant::now());
    let mut window = WindowTabs::new(AppWindowId::FIRST, WindowKind::Normal);

    // Restore two tabs: first active, second inactive
    let active_id = window.restore_tab("https://active.invalid/", "Active Title", 0, true);
    let inactive_id = window.restore_tab("https://inactive.invalid/", "Inactive Title", 1, false);

    assert_eq!(window.len(), 2);
    assert_eq!(window.active_tab_id(), Some(active_id));

    let inactive_tab = window.get_tab(inactive_id).expect("inactive tab");
    assert!(inactive_tab.lifecycle().is_restored_not_loaded());
    assert_eq!(
        inactive_tab.loading_started_at(),
        None,
        "restored inactive tab must defer loading"
    );

    // Activating the inactive tab initiates its load
    window
        .activate_tab(inactive_id, &clock)
        .expect("activate inactive tab");
    assert_eq!(window.active_tab_id(), Some(inactive_id));

    let activated_tab = window.get_tab(inactive_id).expect("activated tab");
    assert!(activated_tab.lifecycle().is_loading());
    assert!(activated_tab.loading_started_at().is_some());
}

#[test]
fn engine_navigation_observation_epoch_is_correlated() {
    let mut engine = HeadlessEngine::new().with_page("https://site.invalid/", "Site Title");

    let mut window = WindowTabs::new(AppWindowId::FIRST, WindowKind::Normal);
    let clock = MockClock::new(Instant::now());
    let tab_id = window.open_tab("https://site.invalid/", &clock);

    let req = Request::new("https://site.invalid/");
    let _id = engine.start_navigation(&req);

    // Poll observation stream and feed into window tab model
    while let Some(obs) = engine.poll_observation() {
        window
            .handle_navigation_observation(tab_id, &obs)
            .expect("observation must apply to tab");
    }

    let tab = window.get_tab(tab_id).expect("tab exists");
    assert_eq!(tab.displayed_address(), "https://site.invalid/");
    assert_eq!(tab.lifecycle(), &TabLifecycle::Live);
    assert_eq!(tab.navigation_epoch(), NavigationEpoch::new(2));

    // Engine emits observation with next epoch for same-document navigation
    let next_epoch = NavigationEpoch::new(3);
    let obs = evreos_engine::NavigationObservation::new(
        next_epoch,
        evreos_engine::NavigationEvent::SameDocumentNavigated {
            id: NavigationId::FIRST,
            address: "https://site.invalid/#frag1".to_owned(),
        },
    );
    window
        .handle_navigation_observation(tab_id, &obs)
        .expect("handle same document observation");

    let updated_tab = window.get_tab(tab_id).expect("tab exists");
    assert_eq!(
        updated_tab.displayed_address(),
        "https://site.invalid/#frag1"
    );
    assert_eq!(updated_tab.navigation_epoch(), next_epoch);
}

#[test]
fn tab_integrates_with_suggestion_index_as_open_tab() {
    let mut tab = Tab::new(
        TabId::FIRST,
        AppWindowId::FIRST,
        0,
        "https://searchable.invalid/docs",
        TabLifecycle::Live,
        None,
    );
    tab.set_title("Documentation Portal");

    let open_tab = tab.to_open_tab();
    assert_eq!(open_tab.address, "https://searchable.invalid/docs");
    assert_eq!(open_tab.title, "Documentation Portal");

    // Verify compatibility with SuggestionIndex
    let count =
        std::sync::atomic::AtomicU64::new(1).fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let temp_dir =
        std::env::temp_dir().join(format!("evreos_tab_test_{}_{count}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);

    let history = HistoryStore::open(&temp_dir.join("history"));
    let bookmarks = BookmarkStore::open(&temp_dir.join("bookmarks"));
    let open_tabs = vec![open_tab];
    let suggestions = SuggestionIndex::query(&history, &bookmarks, &open_tabs, "Document");
    assert!(!suggestions.is_empty());
    assert_eq!(suggestions[0].source(), SuggestionSource::OpenTab);

    let _ = std::fs::remove_dir_all(&temp_dir);
}
