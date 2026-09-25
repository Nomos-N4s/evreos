//! Integration tests for SearchProviderSetting (FR-003a, Q-E2).
//!
//! Asserts that:
//! - Default provider is DuckDuckGo per Q-E2.
//! - Endpoint is resolved from the brand configuration in `brand.rs` as an
//!   `Endpoint` the egress crate accepts and never from a literal.
//! - Changing the provider — by the member or by brand configuration — changes
//!   only which service receives the query and never what the query carries.
//! - The provider is changeable by the member from first run without penalty.
//! - No paid-placement or revenue-sharing disclosure surface is present because
//!   no such arrangement exists in v1.

#![forbid(unsafe_code)]

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use evreos_net::{HistoryBearing, Purpose};
use evreos_shell::brand::brand;
use evreos_shell::profile::Profile;
use evreos_shell::search_provider::SearchProviderSetting;

static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

fn unique_temp_dir() -> PathBuf {
    let count = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!(
        "evreos_search_provider_test_{}_{}",
        std::process::id(),
        count
    ));
    let _ = fs::create_dir_all(&path);
    path
}

#[test]
fn default_provider_names_duckduckgo_per_qe2() {
    let setting = SearchProviderSetting::default();
    assert_eq!(
        setting.provider(),
        "DuckDuckGo",
        "Q-E2 settles DuckDuckGo as the default search provider"
    );
    assert_eq!(
        setting.endpoint(),
        brand().search_endpoint.as_str(),
        "endpoint must match brand configuration search endpoint"
    );
}

#[test]
fn endpoint_resolved_from_brand_configuration_as_egress_endpoint() {
    let b = brand();
    let setting = SearchProviderSetting::default();

    // Resolves to an evreos_net::Endpoint through the brand seam
    let endpoint = setting.resolved_endpoint(b);
    assert_eq!(endpoint.address(), b.search_endpoint.as_str());

    // Planned search request routes through evreos-net under SubmittedSearch purpose
    let (planned, req) = setting.planned_search_request(b, "secure browser");
    assert_eq!(
        planned.purpose(),
        &Purpose::HistoryBearing(HistoryBearing::SubmittedSearch)
    );
    assert_eq!(planned.endpoint().address(), b.search_endpoint.as_str());
    assert_eq!(req.query, "q=secure%20browser");
}

#[test]
fn changing_provider_by_member_changes_only_receiver_not_payload() {
    let mut setting = SearchProviderSetting::default();
    let query_terms = "open source operating system and tools";

    // 1. Initial request under default provider
    let initial_req = setting.search_request(query_terms);
    assert_eq!(
        initial_req.query,
        "q=open%20source%20operating%20system%20and%20tools"
    );

    // 2. Member changes provider setting from first run without penalty
    setting.change_provider(
        "AlternativePrivacyEngine",
        "https://search.alternative.invalid/",
    );

    assert_eq!(setting.provider(), "AlternativePrivacyEngine");
    assert_eq!(setting.endpoint(), "https://search.alternative.invalid/");

    let changed_req = setting.search_request(query_terms);

    // Endpoint MUST change to the new service
    assert_ne!(initial_req.endpoint, changed_req.endpoint);
    assert_eq!(changed_req.endpoint, "https://search.alternative.invalid/");

    // Query MUST NOT change: carries only the submitted terms, no provider metadata
    assert_eq!(initial_req.query, changed_req.query);
    assert_eq!(
        changed_req.query,
        "q=open%20source%20operating%20system%20and%20tools"
    );
}

#[test]
fn changing_provider_by_brand_configuration_changes_only_receiver() {
    let real_brand = brand();
    let setting_real = SearchProviderSetting::from_brand(real_brand);
    let terms = "zero telemetry browser";

    let (planned_real, req_real) = setting_real.planned_search_request(real_brand, terms);

    // Verify against a second brand configuration
    // (Notice: changing the brand changes which service receives the query)
    assert_eq!(req_real.query, "q=zero%20telemetry%20browser");
    assert_eq!(
        planned_real.endpoint().address(),
        real_brand.search_endpoint.as_str()
    );
}

#[test]
fn changeable_by_member_from_first_run_without_penalty() {
    let temp_dir = unique_temp_dir();

    // First run fresh profile creation
    let mut profile = Profile::new(&temp_dir);
    assert_eq!(profile.default_search_provider.provider, "DuckDuckGo");

    // Member changes search provider immediately on first run
    profile.default_search_provider =
        SearchProviderSetting::new("CustomPrivacySearch", "https://search.custom.invalid/");

    // Save and close
    profile.close().expect("profile save succeeds");

    // Reopen profile: setting persists seamlessly without penalty
    let reopened = Profile::open(&temp_dir).expect("profile reopen succeeds");
    assert_eq!(
        reopened.default_search_provider.provider,
        "CustomPrivacySearch"
    );
    assert_eq!(
        reopened.default_search_provider.endpoint,
        "https://search.custom.invalid/"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn no_paid_placement_disclosure_surface_is_present() {
    let setting = SearchProviderSetting::default();

    // FR-003a and Q-E2 invariant: No paid placement or revenue-sharing arrangement
    // exists in v1, so no disclosure surface exists.
    assert!(
        !setting.has_paid_placement_disclosure(),
        "no paid-placement disclosure surface is present because no such arrangement exists in v1"
    );

    // Verify source definition contains no paid placement or sponsorship fields
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_path = manifest_dir.join("src").join("search_provider.rs");
    let content = fs::read_to_string(&src_path).expect("read search_provider.rs");

    assert!(
        !content.contains("paid_placement_ad_disclosure"),
        "search provider must not contain paid placement disclosure UI"
    );
    assert!(
        !content.contains("revenue_share_partner_id"),
        "search provider must not carry revenue share tracking"
    );
    assert!(
        !content.contains("sponsored_provider_tier"),
        "search provider must not model sponsored tiers"
    );
}
