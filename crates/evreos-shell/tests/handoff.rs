//! FR-015a, FR-037, and FR-007a hand-off integration tests for evreos-shell.
//!
//! Under FR-015a:
//! - Local detection MUST inspect only whether a password-type input is present.
//! - Local detection MUST NOT transmit or retain page content.
//! - The browser MUST offer to open the site in the hand-off browser when a
//!   password-type input is detected.
//!
//! Under FR-037:
//! - Where a capability proves unavailable (media hardware, location, notifications,
//!   or protected-media playback), the browser MUST say so and offer a hand-off.
//!
//! Under Key Entities:
//! - Evreos MUST NEVER nominate itself as the hand-off browser.
//!
//! Under FR-007a:
//! - Hand-off passes ONLY the address of the current site, on the member's action
//!   for that occasion, to a program on the same machine and NEVER to a server.

use evreos_shell::brand::brand;
use evreos_shell::handoff::{
    HandOffBrowser, HandOffError, HandOffOffer, HandOffReason, MockHandOffExecutor,
    detect_password_input,
};
use evreos_shell::permissions::Capability;

#[test]
fn detection_inspects_only_whether_password_type_input_is_present() {
    let positive_cases = [
        "<input type=\"password\">",
        "<input type='password'>",
        "<input type=password>",
        "<input type=\"PASSWORD\">",
        "<INPUT TYPE=\"password\">",
        "<INPUT TYPE=\"PASSWORD\">",
        "<input name=\"pass\" type=\"password\" id=\"p\" />",
        "<input class=\"form-control\" type = 'password' >",
        "<div><form><fieldset><input type=\"password\"></fieldset></form></div>",
        "<html><body><form><input\n  type=\"password\"\n  autocomplete=\"current-password\"></form></body></html>",
        "<input value=\"secret\" type=\"password\" />",
        "<input type=\"password\" disabled>",
    ];

    for snippet in positive_cases {
        assert!(
            detect_password_input(snippet),
            "expected password input detected in: {snippet}"
        );
    }

    let negative_cases = [
        "",
        "<html><body>No input elements here</body></html>",
        "<input type=\"text\">",
        "<input type=\"email\">",
        "<input type=\"search\">",
        "<input type=\"tel\">",
        "<input type=\"number\">",
        "<input type=\"checkbox\">",
        "<input type=\"radio\">",
        "<input type=\"hidden\" value=\"password\">",
        "<p>Please enter your master password below:</p><input type=\"text\">",
        "<button type=\"submit\">Change Password</button>",
        "<label for=\"password\">Password</label>",
        "<textarea name=\"password\"></textarea>",
        "<div class=\"password-container\">Secret Account</div>",
        "<span>password</span>",
    ];

    for snippet in negative_cases {
        assert!(
            !detect_password_input(snippet),
            "expected no password input detected in: {snippet}"
        );
    }
}

#[test]
fn detection_and_handoff_offer_transmits_and_retains_no_page_content() {
    let sensitive_token = "SUPER_CONFIDENTIAL_MEMBER_TOKEN_4242";
    let sensitive_content = "Confidential Banking Ledger Balance $999,999.00 USD";
    let sensitive_password = "MySuperSecretMasterPassword123!";
    let sensitive_field_name = "account_credential_field_secret";

    let sensitive_page_html = format!(
        r#"<!DOCTYPE html>
<html>
<head><title>Confidential Vault</title></head>
<body>
  <h1>{sensitive_content}</h1>
  <div data-token="{sensitive_token}">
    <form action="/login" method="POST">
      <label>Master Password</label>
      <input type="password" name="{sensitive_field_name}" value="{sensitive_password}">
      <button type="submit">Unlock</button>
    </form>
  </div>
</body>
</html>"#
    );

    // 1. Detection runs locally on the snippet and returns a boolean flag
    let detected = detect_password_input(&sensitive_page_html);
    assert!(detected, "password input must be detected");

    // 2. Hand-off offer is constructed with ONLY the site address
    let site_address = "https://vault.example.invalid/login";
    let target = HandOffBrowser::nominated("chromium").expect("valid browser");
    let offer =
        HandOffOffer::for_site_credential(site_address, target.clone()).expect("valid offer");

    // 3. Invariant: HandOffOffer retains NO page content
    let offer_debug = format!("{offer:?}");
    let offer_display = format!("{offer}");

    let sensitive_tokens = [
        sensitive_token,
        sensitive_content,
        sensitive_password,
        sensitive_field_name,
        "Confidential Vault",
        "Unlock",
        "method=\"POST\"",
    ];

    for token in sensitive_tokens {
        assert!(
            !offer_debug.contains(token),
            "HandOffOffer debug format retained sensitive page content: {token}"
        );
        assert!(
            !offer_display.contains(token),
            "HandOffOffer display format retained sensitive page content: {token}"
        );
    }

    // 4. Invariant: Hand-off dispatch transmits ONLY the address, never page content
    let mut executor = MockHandOffExecutor::new();
    offer.dispatch(&mut executor).expect("dispatch succeeds");

    assert_eq!(executor.dispatch_count(), 1);
    let (dispatched_target, dispatched_addr) = &executor.dispatches()[0];

    assert_eq!(dispatched_target, &target);
    assert_eq!(dispatched_addr, site_address);

    for token in sensitive_tokens {
        assert!(
            !dispatched_addr.contains(token),
            "dispatched payload contained sensitive page content: {token}"
        );
    }
}

#[test]
fn evreos_never_nominates_itself() {
    // Literal "self" in any case
    assert!(HandOffBrowser::nominated("self").is_err());
    assert!(HandOffBrowser::nominated("SELF").is_err());
    assert!(HandOffBrowser::nominated("  self  ").is_err());

    // Identity "evreos" in any case
    assert!(HandOffBrowser::nominated("evreos").is_err());
    assert!(HandOffBrowser::nominated("EVREOS").is_err());
    assert!(HandOffBrowser::nominated("Evreos").is_err());
    assert!(HandOffBrowser::nominated("evreos --private").is_err());

    // Executable paths and stems
    assert!(HandOffBrowser::nominated("evreos.exe").is_err());
    assert!(HandOffBrowser::nominated("EVREOS.EXE").is_err());
    assert!(HandOffBrowser::nominated("/usr/bin/evreos").is_err());
    assert!(HandOffBrowser::nominated("/usr/local/bin/evreos").is_err());
    assert!(HandOffBrowser::nominated("C:\\Program Files\\Evreos\\evreos.exe").is_err());
    assert!(HandOffBrowser::nominated("C:/Program Files/Evreos/evreos.exe").is_err());

    // Dynamic product name from brand configuration (if set)
    let product_name = brand().product_name.to_string();
    if !product_name.is_empty() && product_name != "unset" {
        assert!(HandOffBrowser::nominated(&product_name).is_err());
        assert!(HandOffBrowser::nominated(product_name.to_lowercase()).is_err());
        assert!(HandOffBrowser::nominated(format!("{product_name}.exe")).is_err());
    }

    // Empty or whitespace inputs
    assert!(HandOffBrowser::nominated("").is_err());
    assert!(HandOffBrowser::nominated("    ").is_err());

    // Third-party external browsers MUST be accepted
    assert!(HandOffBrowser::nominated("firefox").is_ok());
    assert!(HandOffBrowser::nominated("chrome").is_ok());
    assert!(HandOffBrowser::nominated("safari").is_ok());
    assert!(HandOffBrowser::nominated("edge").is_ok());
    assert!(HandOffBrowser::nominated("chromium").is_ok());
    assert!(HandOffBrowser::nominated("C:\\Program Files\\Mozilla Firefox\\firefox.exe").is_ok());
    assert!(HandOffBrowser::nominated("/usr/bin/google-chrome").is_ok());
}

#[test]
fn handoff_offers_raised_under_fr015a_and_fr037() {
    let target = HandOffBrowser::SystemDefault;

    // FR-015a: Site-credential autofill detection offer
    let autofill_offer =
        HandOffOffer::for_site_credential("https://bank.example.invalid/login", target.clone())
            .expect("valid offer");
    assert_eq!(
        autofill_offer.reason(),
        &HandOffReason::PasswordInputDetected
    );
    assert_eq!(
        autofill_offer.address(),
        "https://bank.example.invalid/login"
    );
    assert!(autofill_offer.reason().description().contains("autofill"));

    // FR-037: All four declared capabilities
    for cap in Capability::ALL {
        let cap_offer = HandOffOffer::for_unavailable_capability(
            cap,
            "https://app.example.invalid/session",
            target.clone(),
        )
        .expect("valid offer");

        assert_eq!(
            cap_offer.reason(),
            &HandOffReason::CapabilityUnavailable(cap)
        );
        assert_eq!(cap_offer.address(), "https://app.example.invalid/session");
        assert!(
            cap_offer.reason().description().contains(cap.as_str()),
            "reason description must name capability {}",
            cap.as_str()
        );
    }

    // FR-037: Protected media playback offer
    let media_offer =
        HandOffOffer::for_protected_media("https://stream.example.invalid/video", target)
            .expect("valid offer");
    assert_eq!(
        media_offer.reason(),
        &HandOffReason::ProtectedMediaUnavailable
    );
    assert_eq!(
        media_offer.address(),
        "https://stream.example.invalid/video"
    );
    assert!(
        media_offer
            .reason()
            .description()
            .contains("Protected media")
    );
}

#[test]
fn handoff_passes_only_address_to_program_on_same_machine_and_never_to_a_server() {
    let target = HandOffBrowser::nominated("firefox").expect("valid browser");
    let site_address = "https://member.service.example.invalid/portal";
    let offer =
        HandOffOffer::for_site_credential(site_address, target.clone()).expect("valid offer");

    let (program, args) = offer.prepare_dispatch();
    assert_eq!(program, Some("firefox"));
    assert_eq!(args, vec![site_address.to_string()]);

    let mut executor = MockHandOffExecutor::new();
    offer.dispatch(&mut executor).expect("dispatch success");

    assert_eq!(executor.dispatch_count(), 1);
    assert_eq!(executor.dispatches()[0], (target, site_address.to_string()));

    // Rejected if address is empty
    assert!(matches!(
        HandOffOffer::new(
            HandOffReason::PasswordInputDetected,
            "",
            HandOffBrowser::SystemDefault
        ),
        Err(HandOffError::InvalidAddress(_))
    ));
}
