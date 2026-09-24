use evreos_engine::LoadError;
use evreos_engine::conformance::conformance_suite;
use evreos_engine_headless::HeadlessEngine;

fn make_configured_headless_engine() -> HeadlessEngine {
    HeadlessEngine::new()
        .with_failure(
            "https://unresolvable.test/",
            LoadError::Unresolvable {
                address: "https://unresolvable.test/".into(),
            },
        )
        .with_failure(
            "https://cert-error.test/",
            LoadError::Certificate {
                address: "https://cert-error.test/".into(),
                detail: "invalid self-signed cert".into(),
            },
        )
        .with_failure(
            "https://intercepted.test/",
            LoadError::Intercepted {
                address: "https://intercepted.test/".into(),
            },
        )
        .with_failure(
            "https://auth-required.test/",
            LoadError::AuthenticationRequired {
                address: "https://auth-required.test/".into(),
            },
        )
        .with_page("https://success.test/", "Success Page")
        .with_redirect(
            "https://redirect-source.test/",
            "https://redirect-target.test/",
            "Redirected Page",
        )
        .with_hanging_load("https://hanging.test/")
}

#[test]
fn headless_engine_passes_conformance_battery() {
    conformance_suite(make_configured_headless_engine);
}
