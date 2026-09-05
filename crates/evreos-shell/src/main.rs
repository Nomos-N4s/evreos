//! The Evreos shell.
//!
//! At M0 this proves one thing and claims nothing else: the shell drives
//! rendering through [`evreos_engine::Engine`] and never through a concrete
//! backend, so the seam Principle III requires is real rather than asserted.
//! It is exercised here against the headless implementation, which is why this
//! binary builds and runs on a machine with no system webview at all.
//!
//! The same console proof exercises the second seam Principle VIII requires:
//! every brand value the shell shows or sends to comes through [`brand`],
//! never from a literal here, so the printed brand line and the composed —
//! not sent — search request below change with the brand file and with
//! nothing else.

#![forbid(unsafe_code)]

mod brand;

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

    let brand = brand::brand();
    println!("brand: {}", brand.product_name);
    // Composed and printed, never sent: FR-003a's submitted search is the one
    // point at which typed terms leave the machine, and this proof has no
    // member and no submission. What it shows is the seam -- the receiver
    // comes from the brand file, the query from the terms alone.
    let search = brand::search_request(brand, "example terms");
    println!(
        "a submitted search would go to {} carrying only {}",
        search.endpoint, search.query
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
