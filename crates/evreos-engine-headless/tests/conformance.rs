use evreos_engine::LoadError;
use evreos_engine::conformance::{conformance_host_suite, conformance_suite};
use evreos_engine_headless::{HeadlessEngine, HeadlessHost};

fn make_configured_headless_host() -> HeadlessHost {
    HeadlessHost::new()
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
        .with_page("https://blocked-content.test/", "Blocked Content Test Page")
        .with_subresources(
            "https://blocked-content.test/",
            [
                "https://blocked-content.test/tracker.js",
                "https://blocked-content.test/ad.png",
                "https://blocked-content.test/content.css",
            ],
        )
        .with_download("https://download.test/", "test-download.bin", Some(1024))
        .with_download(
            "https://reject-download.test/",
            "reject-download.bin",
            Some(512),
        )
        .with_download(
            "https://cancel-download.test/",
            "cancel-download.bin",
            Some(2048),
        )
}

fn make_configured_headless_engine() -> HeadlessEngine {
    make_configured_headless_host().create_engine()
}

#[test]
fn headless_engine_passes_conformance_battery() {
    conformance_suite(make_configured_headless_engine);
}

#[test]
fn headless_host_passes_conformance_battery() {
    conformance_host_suite(make_configured_headless_host);
}
