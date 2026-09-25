//! Integration tests for `SiteKey` over bank login subdomains and URL variations.
//!
//! # Invariants Verified
//!
//! - **Founder Decision 0007 (`decisions/0007`)**:
//!   The site key is the canonical registrable domain (eTLD+1) for domain names,
//!   falling back to canonical host for IP addresses and single-label hosts.
//! - **Bank Subdomain Invariant (Edge Cases)**:
//!   Subdomains share the same [`SiteKey`] (e.g. `login.bank.invalid` and
//!   `bank.invalid` produce identical site keys), preventing browser abandonment
//!   caused by broken login redirection flows.
//! - **Store Compatibility (FR-006, FR-008)**:
//!   [`SiteKey`] functions seamlessly as a lookup key in maps and sets, allowing
//!   exceptions and permissions granted on one subdomain to apply across all
//!   subdomains of that registrable domain.

#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};

use evreos_shell::{SiteKey, SiteKeyError};

#[test]
fn bank_login_subdomain_unification_prevents_abandonment() {
    // The scenario named in Edge Cases: a member visits a bank landing page,
    // proceeds to the login subdomain, and navigates across authentication hops.
    let landing = SiteKey::from_url("https://bank.invalid/").expect("landing url");
    let login = SiteKey::from_url("https://login.bank.invalid/auth/signin").expect("login url");
    let auth_api =
        SiteKey::from_url("https://auth.bank.invalid:8443/oauth/token").expect("auth api url");
    let portal =
        SiteKey::from_url("https://online.banking.bank.invalid/dashboard").expect("portal url");
    let www = SiteKey::from_url("https://www.bank.invalid/").expect("www url");

    // All these subdomains must produce the identical SiteKey: "bank.invalid"
    assert_eq!(landing.as_str(), "bank.invalid");
    assert_eq!(login.as_str(), "bank.invalid");
    assert_eq!(auth_api.as_str(), "bank.invalid");
    assert_eq!(portal.as_str(), "bank.invalid");
    assert_eq!(www.as_str(), "bank.invalid");

    assert_eq!(landing, login);
    assert_eq!(login, auth_api);
    assert_eq!(auth_api, portal);
    assert_eq!(portal, www);

    // Matching methods also confirm membership
    assert!(landing.matches_url("https://login.bank.invalid/step2"));
    assert!(landing.matches_host("auth.login.bank.invalid"));
    assert!(login.matches_url("https://bank.invalid/"));

    // A different bank must NOT match
    let other_bank = SiteKey::from_url("https://login.otherbank.invalid/").expect("other bank url");
    assert_eq!(other_bank.as_str(), "otherbank.invalid");
    assert_ne!(landing, other_bank);
    assert!(!landing.matches_url("https://otherbank.invalid/"));
}

#[test]
fn store_map_lookup_shares_exceptions_across_subdomains() {
    // Simulate a blocking exception store keyed by SiteKey
    let mut blocking_exceptions: HashMap<SiteKey, bool> = HashMap::new();

    // Member disables ad/tracker blocking while on the main site
    let site = SiteKey::from_url("https://www.bank.invalid/").expect("main site");
    blocking_exceptions.insert(site, false); // false = blocking disabled for this site

    // Navigation redirects to the bank's authentication subdomain
    let current_page =
        SiteKey::from_url("https://login.bank.invalid/auth").expect("login subdomain");

    // Lookup must succeed, finding blocking disabled for the bank
    let is_exception = blocking_exceptions.get(&current_page);
    assert_eq!(is_exception, Some(&false));

    // And vice-versa: setting an exception on the login subdomain protects subsequent hops
    let mut permission_store: HashSet<SiteKey> = HashSet::new();
    let video_call_key =
        SiteKey::from_url("https://video.telehealth.invalid/room/123").expect("video url");
    permission_store.insert(video_call_key);

    let portal_key = SiteKey::from_url("https://portal.telehealth.invalid/").expect("portal url");
    assert!(permission_store.contains(&portal_key));
}

#[test]
fn multi_part_public_suffixes_resolve_correct_registrable_domain() {
    // British .co.uk
    let uk_bank =
        SiteKey::from_url("https://login.secure.mybank.co.uk/sign-in").expect("uk bank url");
    let uk_root = SiteKey::from_url("https://mybank.co.uk/").expect("uk root url");
    assert_eq!(uk_bank.as_str(), "mybank.co.uk");
    assert_eq!(uk_root.as_str(), "mybank.co.uk");
    assert_eq!(uk_bank, uk_root);

    // Australian .com.au
    let au_bank = SiteKey::from_url("https://netbank.mybank.com.au/logon").expect("au bank url");
    assert_eq!(au_bank.as_str(), "mybank.com.au");

    // German .de (single-label ccTLD)
    let de_bank = SiteKey::from_url("https://banking.mybank.de/login").expect("de bank url");
    assert_eq!(de_bank.as_str(), "mybank.de");

    // Greek .gr (single-label ccTLD) and .com.gr (multi-part ccTLD)
    let gr_bank = SiteKey::from_url("https://ebanking.bank.gr/login").expect("gr bank url");
    assert_eq!(gr_bank.as_str(), "bank.gr");

    let gr_company =
        SiteKey::from_url("https://login.mycompany.com.gr/portal").expect("gr company url");
    assert_eq!(gr_company.as_str(), "mycompany.com.gr");
    let gr_company_root =
        SiteKey::from_url("https://mycompany.com.gr/").expect("gr company root url");
    assert_eq!(gr_company, gr_company_root);
}

#[test]
fn localhost_and_ip_literals_canonicalize_as_host() {
    let localhost = SiteKey::from_url("http://localhost:8080/app").expect("localhost url");
    assert_eq!(localhost.as_str(), "localhost");

    let ipv4_loopback = SiteKey::from_url("http://127.0.0.1:3000/index.html").expect("ipv4 url");
    assert_eq!(ipv4_loopback.as_str(), "127.0.0.1");

    let ipv4_lan = SiteKey::from_url("https://192.168.1.1:8443/settings").expect("lan url");
    assert_eq!(ipv4_lan.as_str(), "192.168.1.1");

    let ipv6_loopback = SiteKey::from_url("https://[::1]:9090/status").expect("ipv6 url");
    assert_eq!(ipv6_loopback.as_str(), "[::1]");

    let ipv6_addr = SiteKey::from_url("https://[2001:db8::1]/path").expect("ipv6 global url");
    assert_eq!(ipv6_addr.as_str(), "[2001:db8::1]");
}

#[test]
fn url_normalization_strips_ports_credentials_and_case() {
    let raw = "HTTPS://admin:secret@LOGIN.MYBANK.INVALID.:8443/path?query=val#section";
    let key = SiteKey::from_url(raw).expect("complex url");
    assert_eq!(key.as_str(), "mybank.invalid");

    // Parse string via FromStr
    let parsed: SiteKey = raw.parse().expect("from_str parse");
    assert_eq!(parsed, key);
}

#[test]
fn error_conditions_rejected_cleanly() {
    assert_eq!(SiteKey::from_url(""), Err(SiteKeyError::Empty));
    assert_eq!(SiteKey::from_url("   "), Err(SiteKeyError::Empty));
    assert_eq!(SiteKey::from_host(""), Err(SiteKeyError::Empty));
    assert_eq!(SiteKey::from_host("  "), Err(SiteKeyError::Empty));

    // Invalid host characters
    assert!(SiteKey::from_host("invalid host with spaces").is_err());
    assert!(SiteKey::from_host("invalid..dots").is_err());
    assert!(SiteKey::from_host(".leading.dot").is_err());
}
