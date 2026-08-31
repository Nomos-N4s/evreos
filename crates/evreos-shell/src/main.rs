//! The Evreos shell.
//!
//! At M0 this proves one thing and claims nothing else: the shell drives
//! rendering through [`evreos_engine::Engine`] and never through a concrete
//! backend, so the seam Principle III requires is real rather than asserted.
//! It is exercised here against the headless implementation, which is why this
//! binary builds and runs on a machine with no system webview at all.

#![forbid(unsafe_code)]

use evreos_engine::{Engine, LoadError, Request};
use evreos_engine_headless::HeadlessEngine;

/// Drive an engine through one navigation and report what happened.
///
/// This function is the whole point of the crate at M0: it is generic over
/// [`Engine`], so it cannot reach for anything a webview happens to expose.
/// When the system-webview backend lands it substitutes here with no change to
/// this code, and if it cannot, the seam was wrong and this is where that
/// shows.
fn navigate<E: Engine>(engine: &mut E, address: &str) -> String {
    match engine.load(&Request::new(address)) {
        Ok(page) => format!("{} — {}", page.title(), page.address()),
        // FR-015: name the cause and offer a next step. The next step is the
        // shell's to choose; naming the cause is the engine's contract.
        Err(error) => format!("could not load: {error}"),
    }
}

fn main() {
    let mut engine = HeadlessEngine::new()
        .with_page("https://example.invalid/", "Example")
        .with_failure(
            "https://expired.invalid/",
            LoadError::Certificate {
                address: "https://expired.invalid/".into(),
                detail: "the certificate expired".into(),
            },
        );

    println!("engine: {}", engine.name());
    for address in [
        "https://example.invalid/",
        "https://expired.invalid/",
        "https://nowhere.invalid/",
    ] {
        println!("{}", navigate(&mut engine, address));
    }
}
