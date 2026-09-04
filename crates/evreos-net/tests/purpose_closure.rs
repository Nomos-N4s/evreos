//! Closure proofs for the `Purpose` enumeration and the `Endpoint` boundary.
//!
//! What each proof carries, and how, stated up front because half of the
//! guarantee is compile-time and a reader should know which half:
//!
//! - **Reachability and closure** are proved in plain tests: every variant of
//!   both sets is constructed, and the matches below carry no wildcard arm,
//!   so adding a variant to either enum fails this file's compilation until
//!   the addition is accounted for — which for the history-bearing set means
//!   the specification amendment FR-007a requires, enforced independently by
//!   `scripts/checks/check_purpose_enum.py`.
//! - **Disjointness** is structural: a `Purpose` holds a `HistoryBearing` or
//!   a `NonHistory` and nothing can be both, which `classify` below proves by
//!   exhaustive match over exactly two arms.
//! - **Money field shapes** are proved at runtime for the halves a runtime
//!   test can reach: the validated charsets refuse an address, a search term
//!   and page content. The other half — that no money variant declares a free
//!   `String` field at all — is the type's public surface in
//!   `crates/evreos-net/src/purpose.rs`, reviewable there, and is not a
//!   runtime assertion.
//! - **`Endpoint` cannot be built from a literal** is a compile-time
//!   argument, deliberately without a compile-fail harness (no trybuild
//!   dependency): `Endpoint` exposes exactly one constructor,
//!   `Endpoint::resolve(BrandResolved)`, no public field, and no
//!   `From`/`TryFrom`/`FromStr` impl over any string-like type — absences
//!   carried by the crate's reviewable API surface, stated in the doc comment
//!   of `BrandResolved`, which also names the half that rests on review: who
//!   calls `BrandResolved::declared_in_brand_configuration`.
//! - **No purposeless, endpointless request path exists**: the crate's one
//!   entry point is `request(Purpose, Endpoint)`, exercised below; that it is
//!   the only one is again the public surface, reviewable in
//!   `crates/evreos-net/src/lib.rs`.

use evreos_net::{
    BrandResolved, ClaimCode, ClickOutReference, Endpoint, HistoryBearing, MinorUnits, NonHistory,
    Purpose, ValueError, request,
};

/// Every variant of both sets, constructed — the reachability half of the
/// closure proof, and the list the other tests draw from.
fn every_purpose() -> Vec<Purpose> {
    let mut purposes: Vec<Purpose> = [
        HistoryBearing::PageLoad,
        HistoryBearing::CertificateStatus,
        HistoryBearing::SubmittedSearch,
        HistoryBearing::HandOff,
    ]
    .into_iter()
    .map(Purpose::HistoryBearing)
    .collect();
    purposes.extend(
        [
            NonHistory::UpdateCheck,
            NonHistory::BlockingListRefresh,
            NonHistory::SurfaceDelivery,
            NonHistory::DiagnosticReport,
            NonHistory::SignIn,
            NonHistory::WalletRead,
            NonHistory::ClaimCodeRedemption {
                code: ClaimCode::new("SUMMER-2026").expect("a conforming code"),
            },
            NonHistory::WithdrawalRequest {
                amount: MinorUnits::new(2_500),
            },
            NonHistory::MerchantCatalogueRead,
            NonHistory::ClickOut {
                reference: ClickOutReference::new("svc_issued-REF-0042")
                    .expect("a conforming reference"),
            },
        ]
        .into_iter()
        .map(Purpose::NonHistory),
    );
    purposes
}

/// Which set a purpose is in. Exhaustive over exactly two arms with no
/// wildcard: the compiler is what proves a purpose cannot be in both sets and
/// cannot be in neither, which is the disjointness the task requires.
fn classify(purpose: &Purpose) -> bool {
    match purpose {
        Purpose::HistoryBearing(_) => true,
        Purpose::NonHistory(_) => false,
    }
}

#[test]
fn every_variant_is_reachable_and_the_sets_are_complete() {
    // Fourteen constructed values: FR-007a's four, the four infrastructure
    // purposes, the six money purposes. The matches below have no wildcard,
    // so a fifteenth variant fails compilation here until it is named.
    let purposes = every_purpose();
    assert_eq!(purposes.len(), 14);
    for purpose in &purposes {
        match purpose {
            Purpose::HistoryBearing(inner) => match inner {
                HistoryBearing::PageLoad
                | HistoryBearing::CertificateStatus
                | HistoryBearing::SubmittedSearch
                | HistoryBearing::HandOff => {}
            },
            Purpose::NonHistory(inner) => match inner {
                NonHistory::UpdateCheck
                | NonHistory::BlockingListRefresh
                | NonHistory::SurfaceDelivery
                | NonHistory::DiagnosticReport
                | NonHistory::SignIn
                | NonHistory::WalletRead
                | NonHistory::ClaimCodeRedemption { .. }
                | NonHistory::WithdrawalRequest { .. }
                | NonHistory::MerchantCatalogueRead
                | NonHistory::ClickOut { .. } => {}
            },
        }
    }
}

#[test]
fn the_two_sets_are_disjoint() {
    // `classify` compiles only because every purpose is in exactly one set;
    // this exercises it over every variant so the structural proof is also a
    // running one, and pins the split at four to ten.
    let purposes = every_purpose();
    let history_bearing = purposes.iter().filter(|p| classify(p)).count();
    assert_eq!(history_bearing, 4);
    assert_eq!(purposes.len() - history_bearing, 10);
}

#[test]
fn no_money_field_can_hold_an_address_a_search_term_or_page_content() {
    // The runtime half of the field-shape proof: the two text-shaped money
    // fields go through validated charsets with no `/`, `:`, `.`, whitespace
    // or markup characters, so the three shapes FR-007a names cannot fit.
    // The remaining money field is an integer, which cannot hold text at all,
    // and that no money variant declares a free `String` is the enum's public
    // surface in src/purpose.rs, reviewed rather than asserted here.
    let an_address = "https://example.org/visited/page";
    let a_bare_address = "example.org";
    let a_search_term = "what the member typed";
    let page_content = "<p>what the page said</p>";
    for history in [an_address, a_bare_address, a_search_term, page_content] {
        assert!(
            matches!(ClaimCode::new(history), Err(ValueError::Charset { .. })),
            "a claim code held {history:?}"
        );
        assert!(
            matches!(
                ClickOutReference::new(history),
                Err(ValueError::Charset { .. })
            ),
            "a click-out reference held {history:?}"
        );
    }
    // The bounds hold too: empty and oversized values are refused, so the
    // charset cannot be padded around.
    assert!(matches!(
        ClaimCode::new(""),
        Err(ValueError::Length { length: 0 })
    ));
    assert!(matches!(
        ClaimCode::new(&"A".repeat(65)),
        Err(ValueError::Length { length: 65 })
    ));
    assert!(matches!(
        ClickOutReference::new(&"A".repeat(129)),
        Err(ValueError::Length { length: 129 })
    ));
    // And conforming values pass, or the guarantee would be vacuous.
    assert_eq!(
        ClaimCode::new("SUMMER-2026")
            .expect("a conforming code")
            .as_str(),
        "SUMMER-2026"
    );
    assert_eq!(
        ClickOutReference::new("ref_0042")
            .expect("a conforming reference")
            .as_str(),
        "ref_0042"
    );
    assert_eq!(MinorUnits::new(2_500).count(), 2_500);
}

#[test]
fn an_endpoint_is_built_only_through_the_brand_resolved_route() {
    // The one route that exists, exercised so the minting path provably
    // works. That it is the ONLY route — no `new(&str)`, no string
    // `From`/`TryFrom`, no public field — is the compile-time half, carried
    // by the API surface this file's header describes; a second constructor
    // would be a visible diff to src/lib.rs.
    let resolved =
        BrandResolved::declared_in_brand_configuration(String::from("brand://update-host"));
    let endpoint = Endpoint::resolve(resolved);
    assert_eq!(endpoint.address(), "brand://update-host");
}

#[test]
fn the_request_path_takes_a_purpose_and_an_endpoint() {
    // The one entry point, exercised with a purpose from each set. Both
    // arguments are by-value and non-optional, so a call site with no
    // `Purpose` or no `Endpoint` has nothing to pass and does not compile;
    // that no other request-shaped path exists is the crate's public
    // surface, reviewable in src/lib.rs.
    let endpoint = |address: &str| {
        Endpoint::resolve(BrandResolved::declared_in_brand_configuration(
            String::from(address),
        ))
    };
    let update = request(
        Purpose::NonHistory(NonHistory::UpdateCheck),
        endpoint("brand://update-host"),
    );
    assert_eq!(
        update.purpose(),
        &Purpose::NonHistory(NonHistory::UpdateCheck)
    );
    assert_eq!(update.endpoint().address(), "brand://update-host");

    let search = request(
        Purpose::HistoryBearing(HistoryBearing::SubmittedSearch),
        endpoint("brand://search-provider"),
    );
    assert_eq!(
        search.purpose(),
        &Purpose::HistoryBearing(HistoryBearing::SubmittedSearch)
    );
    assert_eq!(search.endpoint().address(), "brand://search-provider");
}
