//! Validate the brand configuration before anything is compiled against it.
//!
//! Principle VIII's seam holds the brand in files under `brands/`, and
//! `src/brand.rs` embeds the selected one. Embedding alone would defer every
//! defect to runtime; this script is what makes the rules build-time facts:
//!
//! - **Both brand files must parse and carry every field.** A missing field
//!   fails the build rather than defaulting, whichever brand is selected, so
//!   an edit that breaks the unselected file is caught by the same build that
//!   carries it rather than by the next fixture build.
//! - **The selected brand may carry no `unset` sentinel where a release could
//!   come from.** The gate binds on the release profile for a shipping
//!   platform — see `release_gate_applies` in `src/brand/schema.rs` for why
//!   the line sits exactly there — and its refusal names every unset field.
//!
//! The schema module is compiled into this script through the `#[path]` line
//! below, so the functions run here are the functions the unit tests in
//! `src/brand.rs` prove — one parser, one gate, no second copy.

#![forbid(unsafe_code)]

#[path = "src/brand/schema.rs"]
mod schema;

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
    // Naming any rerun path replaces cargo's default of rerunning on every
    // change, so every file this script reads is named. A feature change
    // reruns it regardless, through the changed build-script environment.
    println!("cargo:rerun-if-changed=../../brands/evreos.toml");
    println!("cargo:rerun-if-changed=../../brands/fixture.toml");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| "CARGO_MANIFEST_DIR is not set; this script only runs under cargo")?;
    let brands = PathBuf::from(manifest_dir)
        .join("..")
        .join("..")
        .join("brands");

    // The same selection `src/brand.rs` makes with cfg: the real brand by
    // default, the fixture under `fixture-brand`, the fixture when both are
    // enabled. Build scripts see features as environment variables, so this
    // is that rule's spelling here.
    let selected = if env::var_os("CARGO_FEATURE_FIXTURE_BRAND").is_some() {
        "fixture.toml"
    } else {
        "evreos.toml"
    };

    for name in ["evreos.toml", "fixture.toml"] {
        let path = brands.join(name);
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("brands/{name}: cannot be read: {error}"))?;
        let brand = schema::parse(&text).map_err(|error| format!("brands/{name}: {error}"))?;
        if name != selected {
            continue;
        }

        let profile = env::var("PROFILE").unwrap_or_default();
        let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
        if schema::release_gate_applies(&profile, &target_os) {
            let unset = schema::unset_fields(&brand);
            if !unset.is_empty() {
                return Err(format!(
                    "brands/{name}: unset field(s): {}. A release-profile build for \
                     {target_os} cannot carry the `unset` sentinel -- a release \
                     against an undetermined value is exactly what the sentinel \
                     exists to stop, and a plausible-looking placeholder is how a \
                     wrong endpoint ships. brands/README.md records what fills each \
                     field; until then, build without --release or build the \
                     fixture brand (--features fixture-brand).",
                    unset.join(", ")
                ));
            }
        }
    }
    Ok(())
}
