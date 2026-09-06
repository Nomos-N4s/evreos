//! T015's spike host. On tier 1 (Windows) and tier 2 (macOS) this runs the
//! winit host in `host.rs` with the minimal AccessKit front in
//! `fronts/accesskit_min.rs`. Elsewhere the crate's platform dependencies do
//! not exist — they are declared only for those targets — so this compiles
//! to a statement of the tier gate and exits 0.
#![forbid(unsafe_code)]

#[cfg(any(windows, target_os = "macos"))]
mod fronts;
#[cfg(any(windows, target_os = "macos"))]
mod host;

#[cfg(any(windows, target_os = "macos"))]
fn main() -> std::process::ExitCode {
    match host::run(Box::new(fronts::accesskit_min::AccessKitMinFront::new())) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("[n6] the spike host failed: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
fn main() {
    println!("n6-chrome-accessibility: this spike host is tier-gated to Windows and macOS.");
    println!("Its platform dependencies are declared only for those targets, so on this");
    println!("platform the crate compiles to this statement and exits 0. T015 measures on");
    println!("tier 1 and tier 2; see tests/spikes/n6-chrome-accessibility/README.md.");
}
