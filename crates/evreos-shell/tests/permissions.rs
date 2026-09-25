//! Integration tests for site permission store and prompt flow under FR-006 and FR-037.
//!
//! # Invariants Covered
//!
//! - **Each Capability Covered (FR-006)**:
//!   Camera, Microphone, Location, Notification default to Ask for every site.
//! - **Revocation Taking Effect (FR-006)**:
//!   Decisions are revisitable and revocation takes effect immediately without reinstalling.
//! - **Platform Capability Honesty (FR-037)**:
//!   `UnavailableOnThisPlatform` is never presented as a denial and routes to hand-off.
//! - **Private Window Decisions Die with the Window (FR-007, FR-007a)**:
//!   Private window decisions live exclusively in memory and die on window closure.
//! - **SiteKey Subdomain Unification (Decision 0007)**:
//!   Permissions granted on one subdomain apply across the registrable domain.

#![forbid(unsafe_code)]

use std::fs;
use std::path::PathBuf;

use evreos_shell::{
    AppWindowId, Capability, PermissionDecision, PermissionPromptRequest, PermissionStore,
    PromptOutcome, PromptResponse, PromptResult, SiteKey, WindowScope,
};

fn temp_profile_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "evreos_test_perm_{test_name}_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp profile dir");
    dir
}

#[test]
fn each_capability_defaults_to_ask_and_operates_independently() {
    let mut store = PermissionStore::in_memory();
    let site = SiteKey::from_url("https://conferencing.invalid/room").expect("valid site");

    // All four capabilities default to Ask
    for cap in Capability::ALL {
        assert_eq!(
            store.query(&site, cap, WindowScope::Persistent),
            PermissionDecision::Ask,
            "Capability {cap} must default to Ask"
        );
    }

    // Grant Camera
    store
        .record_decision(
            site.clone(),
            Capability::Camera,
            PermissionDecision::Granted,
            WindowScope::Persistent,
        )
        .expect("grant camera");

    // Deny Microphone
    store
        .record_decision(
            site.clone(),
            Capability::Microphone,
            PermissionDecision::Denied,
            WindowScope::Persistent,
        )
        .expect("deny microphone");

    // Verify Camera is Granted, Microphone is Denied, Location and Notification stay Ask
    assert_eq!(
        store.query(&site, Capability::Camera, WindowScope::Persistent),
        PermissionDecision::Granted
    );
    assert_eq!(
        store.query(&site, Capability::Microphone, WindowScope::Persistent),
        PermissionDecision::Denied
    );
    assert_eq!(
        store.query(&site, Capability::Location, WindowScope::Persistent),
        PermissionDecision::Ask
    );
    assert_eq!(
        store.query(&site, Capability::Notification, WindowScope::Persistent),
        PermissionDecision::Ask
    );
}

#[test]
fn revocation_takes_effect_without_reinstalling() {
    let mut store = PermissionStore::in_memory();
    let site = SiteKey::from_host("telehealth.invalid").expect("valid site");

    // Grant Location
    store
        .record_decision(
            site.clone(),
            Capability::Location,
            PermissionDecision::Granted,
            WindowScope::Persistent,
        )
        .expect("record grant");
    assert_eq!(
        store.query(&site, Capability::Location, WindowScope::Persistent),
        PermissionDecision::Granted
    );

    // Revoke Location -> decision revisitable without reinstalling anything
    store
        .revoke(&site, Capability::Location, WindowScope::Persistent)
        .expect("revoke location");

    // Immediately returns Ask
    assert_eq!(
        store.query(&site, Capability::Location, WindowScope::Persistent),
        PermissionDecision::Ask
    );
}

#[test]
fn unavailable_state_never_presented_as_denial_and_routes_to_handoff() {
    let mut store = PermissionStore::in_memory();
    let site = SiteKey::from_url("https://navigation.invalid/map").expect("valid site");

    // Declare location unavailable on this platform (e.g. tier-2 engine missing delegate)
    store.set_capability_available(Capability::Location, false);

    // 1. Query returns UnavailableOnThisPlatform, NEVER Denied
    let decision = store.query(&site, Capability::Location, WindowScope::Persistent);
    assert_eq!(
        decision,
        PermissionDecision::UnavailableOnThisPlatform,
        "Unavailable capability must never be presented as Denied"
    );
    assert!(!decision.is_denied());
    assert!(decision.is_unavailable());

    // 2. Evaluation routes directly to FR-037 hand-off offer without presenting a consent prompt
    let outcome = store.evaluate_request(&site, Capability::Location, WindowScope::Persistent);
    assert_eq!(
        outcome,
        PromptOutcome::UnavailableOnThisPlatform {
            handoff_offered: true
        }
    );

    // Attempting to evaluate an available capability (Camera) still prompts
    let camera_outcome = store.evaluate_request(&site, Capability::Camera, WindowScope::Persistent);
    assert!(matches!(camera_outcome, PromptOutcome::NeedsPrompt(_)));
}

#[test]
fn private_window_decisions_die_with_the_window_and_leave_no_disk_trace() {
    let root = temp_profile_dir("private_isolation");
    let mut store = PermissionStore::open(&root);
    let site = SiteKey::from_url("https://sensitive.invalid/portal").expect("valid site");
    let win_id = AppWindowId::FIRST;
    let private_scope = WindowScope::PrivateWindow(win_id);

    // Grant Notification in private window
    store
        .record_decision(
            site.clone(),
            Capability::Notification,
            PermissionDecision::Granted,
            private_scope,
        )
        .expect("record private decision");

    // Active in this private window
    assert_eq!(
        store.query(&site, Capability::Notification, private_scope),
        PermissionDecision::Granted
    );

    // Normal window still sees default Ask (isolated)
    assert_eq!(
        store.query(&site, Capability::Notification, WindowScope::Persistent),
        PermissionDecision::Ask
    );

    // Verify disk has no permissions.toml or permissions.toml contains no sensitive entry
    let perm_file = root.join("permissions.toml");
    if perm_file.exists() {
        let content = fs::read_to_string(&perm_file).expect("read permissions.toml");
        assert!(
            !content.contains("sensitive.invalid"),
            "Private window permission must NEVER be written to disk"
        );
    }

    // Close private window -> purge
    store.purge_private_window(win_id);

    // Now private window also returns Ask
    assert_eq!(
        store.query(&site, Capability::Notification, private_scope),
        PermissionDecision::Ask
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn subdomains_share_permissions_per_founder_decision_0007() {
    let mut store = PermissionStore::in_memory();
    let bank_main = SiteKey::from_url("https://bank.invalid/").expect("bank main");
    let bank_login = SiteKey::from_url("https://login.bank.invalid/auth").expect("bank login");
    let bank_video = SiteKey::from_url("https://video.teller.bank.invalid/").expect("bank video");

    // Member grants camera on main site or video subdomain
    store
        .record_decision(
            bank_video.clone(),
            Capability::Camera,
            PermissionDecision::Granted,
            WindowScope::Persistent,
        )
        .expect("record grant");

    // Main site and login subdomain observe the grant because SiteKey unifies them
    assert_eq!(
        store.query(&bank_main, Capability::Camera, WindowScope::Persistent),
        PermissionDecision::Granted
    );
    assert_eq!(
        store.query(&bank_login, Capability::Camera, WindowScope::Persistent),
        PermissionDecision::Granted
    );
}

#[test]
fn persistent_permissions_survive_store_restart_across_reopen() {
    let root = temp_profile_dir("restart_persistence");

    // Session 1: Record permissions and drop store
    {
        let mut store = PermissionStore::open(&root);
        let site1 = SiteKey::from_host("first.invalid").expect("site 1");
        let site2 = SiteKey::from_host("second.invalid").expect("site 2");

        store
            .record_decision(
                site1,
                Capability::Camera,
                PermissionDecision::Granted,
                WindowScope::Persistent,
            )
            .expect("record site 1");
        store
            .record_decision(
                site2,
                Capability::Microphone,
                PermissionDecision::Denied,
                WindowScope::Persistent,
            )
            .expect("record site 2");
    }

    // Session 2: Open fresh store pointing to same directory
    {
        let store = PermissionStore::open(&root);
        let site1 = SiteKey::from_host("first.invalid").expect("site 1");
        let site2 = SiteKey::from_host("second.invalid").expect("site 2");

        assert_eq!(
            store.query(&site1, Capability::Camera, WindowScope::Persistent),
            PermissionDecision::Granted
        );
        assert_eq!(
            store.query(&site2, Capability::Microphone, WindowScope::Persistent),
            PermissionDecision::Denied
        );
        assert_eq!(
            store.query(&site1, Capability::Microphone, WindowScope::Persistent),
            PermissionDecision::Ask
        );
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn prompt_flow_lifecycle_and_responses() {
    let mut store = PermissionStore::in_memory();
    let site = SiteKey::from_url("https://meeting.invalid/").expect("site");

    // 1. Initial request evaluates to NeedsPrompt
    let outcome = store.evaluate_request(&site, Capability::Camera, WindowScope::Persistent);
    assert_eq!(
        outcome,
        PromptOutcome::NeedsPrompt(PermissionPromptRequest {
            site: site.clone(),
            capability: Capability::Camera,
            window_scope: WindowScope::Persistent,
        })
    );

    // 2. Member grants with remember: true
    let result = store
        .handle_prompt_response(
            site.clone(),
            Capability::Camera,
            PromptResponse::Grant { remember: true },
            WindowScope::Persistent,
        )
        .expect("handle grant");
    assert_eq!(result, PromptResult::AccessGranted { remembered: true });

    // 3. Subsequent request evaluates to AlreadyGranted
    let next_outcome = store.evaluate_request(&site, Capability::Camera, WindowScope::Persistent);
    assert_eq!(next_outcome, PromptOutcome::AlreadyGranted);

    // 4. Member revisits decision and denies
    let deny_result = store
        .handle_prompt_response(
            site.clone(),
            Capability::Camera,
            PromptResponse::Deny { remember: true },
            WindowScope::Persistent,
        )
        .expect("handle deny");
    assert_eq!(deny_result, PromptResult::AccessDenied { remembered: true });

    // 5. Subsequent request evaluates to AlreadyDenied
    let denied_outcome = store.evaluate_request(&site, Capability::Camera, WindowScope::Persistent);
    assert_eq!(denied_outcome, PromptOutcome::AlreadyDenied);
}
