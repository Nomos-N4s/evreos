//! Take the update check's wake from `budgets.toml`.
//!
//! SC-005 enumerates every scheduled wake in the budget file, and T060 arms
//! the update check under that entry and adds no second. This script reads
//! the entry's period and processor-time bound and compiles them into the
//! crate, so the numbers the check runs under are the file's and a file
//! without the entry fails the build. The reader is compiled in through the
//! `#[path]` line below, so it is the reader the crate's unit tests prove.

#![forbid(unsafe_code)]

#[path = "src/update/wake.rs"]
#[allow(dead_code)]
mod wake;

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").map_err(|e| e.to_string())?);
    let budgets = manifest.join("../../budgets.toml");
    println!("cargo::rerun-if-changed={}", budgets.display());
    println!("cargo::rerun-if-changed=src/update/wake.rs");
    let text =
        fs::read_to_string(&budgets).map_err(|error| format!("{}: {error}", budgets.display()))?;
    let wake = wake::read(&text, wake::UPDATE_CHECK)?;
    let out = PathBuf::from(env::var("OUT_DIR").map_err(|e| e.to_string())?);
    fs::write(
        out.join("update_wake.rs"),
        format!(
            "Wake {{ period_seconds: {}, processor_time_bound_ms: {} }}\n",
            wake.period_seconds, wake.processor_time_bound_ms
        ),
    )
    .map_err(|error| error.to_string())
}
