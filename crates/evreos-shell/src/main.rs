//! The Evreos shell.
//!
//! At M0 this proves one thing and claims nothing else: the shell drives
//! rendering through [`evreos_engine::Engine`] and never through a concrete
//! backend, so the seam Principle III requires is real rather than asserted.
//! It is exercised here against the headless implementation, which is why this
//! binary builds and runs on a machine with no system webview at all.

#![forbid(unsafe_code)]

use evreos_engine::{Engine, LoadError, NavigationEvent, Request};
use evreos_engine_headless::HeadlessEngine;

/// Drive an engine through one navigation and report what happened.
///
/// This function is the whole point of the crate at M0: it is generic over
/// [`Engine`], so it cannot reach for anything a webview happens to expose.
/// When the system-webview backend lands it substitutes here with no change to
/// this code, and if it cannot, the seam was wrong and this is where that
/// shows.
fn navigate<E: Engine>(engine: &mut E, address: &str) -> String {
    let id = engine.start_navigation(&Request::new(address));

    // Drain the queue to quiescence and keep the terminal event for the
    // navigation this call started; events for other navigations are not this
    // demo's to report. A navigation with no terminal event is still loading,
    // and the bound on how long that may be shown is the shell's policy under
    // SC-009 — the consumer that applies it replaces this demo.
    let mut outcome = format!("{address} is still loading");
    while let Some(event) = engine.poll_event() {
        if event.id() != id {
            continue;
        }
        match event {
            NavigationEvent::Succeeded { .. } => {
                outcome = match engine.current() {
                    Some(page) => format!("{} — {}", page.title(), page.address()),
                    None => "succeeded with no current page".to_owned(),
                };
            }
            // FR-015: name the cause and offer a next step. The next step is
            // the shell's to choose; naming the cause is the engine's contract.
            NavigationEvent::Failed { error, .. } => {
                outcome = format!("could not load: {error}");
            }
            NavigationEvent::Started { .. }
            | NavigationEvent::Redirected { .. }
            | NavigationEvent::Committed { .. }
            | NavigationEvent::TitleChanged { .. }
            | NavigationEvent::NavigatedAway { .. } => {}
        }
    }
    outcome
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
