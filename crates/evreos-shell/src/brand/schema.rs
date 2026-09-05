//! The brand schema: what a brand file must carry, and how one is read.
//!
//! Principle VIII allows no brand name, colour, endpoint or support address
//! outside one brand configuration, and FR-042 restates that rule for this
//! feature. This module is the schema half of that seam: the closed set of
//! fields a brand carries, the parser for the restricted file format, and the
//! two validations the build script applies. The selection half — which file is
//! embedded, and the value the running shell reads — lives in `brand.rs`.
//!
//! This file is deliberately compiled twice: once here, as a module of the
//! shell, and once by `build.rs` through a `#[path]` module. That is what makes
//! the build-time gate honest — the function the build script calls is the
//! function the unit tests in `brand.rs` prove, not a second copy of it.
//!
//! The file format is TOML restricted to `key = "string"` lines, with full-line
//! `#` comments and blank lines and nothing else: no tables, no arrays, no
//! escapes, no trailing comments. The restriction is what lets the parser be a
//! page of stdlib-only code shared with a build script that must not grow
//! dependencies, and `brands/README.md` states it where the files live.
//!
//! Two rules, split on purpose:
//!
//! - **A missing field fails every build.** Every field is required, so a brand
//!   file missing one fails the build rather than defaulting — a default is how
//!   a wrong endpoint ships without anyone choosing it.
//! - **An `unset` field fails a release build for a shipping platform.** The
//!   sentinel exists because most of the real brand's values are undetermined
//!   or owned outside this repository, and a plausible-looking placeholder is
//!   worse than an honest hole. Debug and test builds carry the sentinel
//!   freely; a build that could become a release artefact may not.

/// Every value Principle VIII moves behind the seam.
///
/// The fields are the closed set a brand consists of: the names, the palette,
/// every endpoint the shell may send to, and the support address. Adding a
/// field is a schema change made here and in [`FIELDS`] together, which the
/// `field_accessors_cover_every_field_distinctly` test in `brand.rs` keeps
/// honest.
#[derive(Debug)]
pub struct Brand {
    /// The browser's own name, as shown to the member.
    pub product_name: String,
    /// The money service's name. FR-042 and the FR-035 catalogues carry brand
    /// names as arguments, never baked into message text, and this is where
    /// that argument comes from.
    pub money_service_name: String,
    /// The primary interface colour, as an opaque string the theme consumes.
    pub primary_colour: String,
    /// The accent colour, likewise opaque to this schema.
    pub accent_colour: String,
    /// Where a submitted search goes — FR-003a's default provider, settled by
    /// Q-E2. Changing this value changes which service receives the query and
    /// nothing else; `search_request` in `brand.rs` is what holds that line.
    pub search_endpoint: String,
    /// The money service's base endpoint. The Apivo operations resolve against
    /// this through the egress chokepoint, never from a literal.
    pub money_endpoint: String,
    /// The FR-014 update channel's host.
    pub update_endpoint: String,
    /// The FR-019 surface-delivery host.
    pub surface_endpoint: String,
    /// The FR-039b diagnostics relay ingress.
    pub diagnostics_endpoint: String,
    /// Where a member writes for help.
    pub support_address: String,
}

/// The sentinel a field carries while its real value is undetermined.
///
/// Exact, case-sensitive, and never a value: `parse` accepts it so that debug
/// and test builds work, [`unset_fields`] reports it, and the build script
/// refuses it where [`release_gate_applies`]. Any other spelling — `Unset`,
/// `UNSET` — is an ordinary value, which is the trap the exactness avoids
/// stepping around silently: a misspelled sentinel is a set field the release
/// gate will not catch, so the spelling is stated here and checked nowhere
/// looser.
pub const UNSET: &str = "unset";

/// One row of [`FIELDS`]: a field's name, and the accessor that reads it from
/// a [`Brand`].
pub type Field = (&'static str, fn(&Brand) -> &str);

/// Every field of [`Brand`], by name, with its accessor.
///
/// This table is what makes "every field" mean the same thing everywhere: the
/// parser requires each name, [`unset_fields`] walks it, and the brand check in
/// `scripts/checks/check_brand.py` learns its forbidden literals from the same
/// files this table gives shape to.
pub const FIELDS: [Field; 10] = [
    ("product_name", |brand| brand.product_name.as_str()),
    ("money_service_name", |brand| {
        brand.money_service_name.as_str()
    }),
    ("primary_colour", |brand| brand.primary_colour.as_str()),
    ("accent_colour", |brand| brand.accent_colour.as_str()),
    ("search_endpoint", |brand| brand.search_endpoint.as_str()),
    ("money_endpoint", |brand| brand.money_endpoint.as_str()),
    ("update_endpoint", |brand| brand.update_endpoint.as_str()),
    ("surface_endpoint", |brand| brand.surface_endpoint.as_str()),
    ("diagnostics_endpoint", |brand| {
        brand.diagnostics_endpoint.as_str()
    }),
    ("support_address", |brand| brand.support_address.as_str()),
];

/// Whether `value` is a real value rather than the [`UNSET`] sentinel.
pub fn is_set(value: &str) -> bool {
    value != UNSET
}

/// Parse one brand file, refusing anything outside the restricted schema.
///
/// Errors are plain strings naming the line or the field, because the consumer
/// that matters is a build script whose whole output is the message a developer
/// reads when the build stops; a structured error type would be unwrapped into
/// exactly this text and nothing else.
pub fn parse(source: &str) -> Result<Brand, String> {
    // An editor that writes a byte-order mark is not a way past the schema,
    // and the mark is not whitespace, so the first line would otherwise carry
    // an invisible prefix and fail as a malformed key.
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let mut values: Vec<(&str, String)> = Vec::new();
    for (number, line) in source.lines().enumerate() {
        let number = number + 1;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key_part, value_part)) = line.split_once('=') else {
            return Err(format!(
                "line {number}: not a `key = \"value\"` line; the schema is TOML \
                 restricted to those, full-line comments and blank lines"
            ));
        };
        let key = key_part.trim();
        if key.is_empty() || !key.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
            return Err(format!(
                "line {number}: key {key:?} is not a lowercase snake_case name"
            ));
        }
        let Some(name) = FIELDS
            .iter()
            .map(|(name, _)| *name)
            .find(|name| *name == key)
        else {
            return Err(format!(
                "line {number}: unknown field `{key}`; the schema is closed, so a \
                 new field is added to the schema first and to the files second"
            ));
        };
        if values.iter().any(|(seen, _)| *seen == name) {
            return Err(format!("line {number}: field `{key}` is set twice"));
        }
        let value_part = value_part.trim();
        let Some(value) = value_part
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))
        else {
            return Err(format!(
                "line {number}: the value of `{key}` is not one double-quoted \
                 string with nothing after the closing quote"
            ));
        };
        if value.contains('"') || value.contains('\\') {
            return Err(format!(
                "line {number}: the value of `{key}` carries a quote or a \
                 backslash; escapes are outside the restricted schema"
            ));
        }
        if value.is_empty() {
            return Err(format!(
                "line {number}: the value of `{key}` is empty; a field carries a \
                 real value or the `{UNSET}` sentinel, never an empty string"
            ));
        }
        values.push((name, value.to_string()));
    }

    let missing: Vec<&str> = FIELDS
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| !values.iter().any(|(seen, _)| seen == name))
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "missing field(s): {}; every field is required, so a brand file \
             missing one fails the build rather than defaulting",
            missing.join(", ")
        ));
    }

    let mut take = |name: &str| -> String {
        let index = values
            .iter()
            .position(|(seen, _)| *seen == name)
            .expect("presence of every field was checked above");
        values.swap_remove(index).1
    };
    Ok(Brand {
        product_name: take("product_name"),
        money_service_name: take("money_service_name"),
        primary_colour: take("primary_colour"),
        accent_colour: take("accent_colour"),
        search_endpoint: take("search_endpoint"),
        money_endpoint: take("money_endpoint"),
        update_endpoint: take("update_endpoint"),
        surface_endpoint: take("surface_endpoint"),
        diagnostics_endpoint: take("diagnostics_endpoint"),
        support_address: take("support_address"),
    })
}

/// The fields of `brand` still carrying the [`UNSET`] sentinel, in schema
/// order, so a refusal can name every hole at once rather than one per build.
pub fn unset_fields(brand: &Brand) -> Vec<&'static str> {
    FIELDS
        .iter()
        .filter(|(_, get)| !is_set(get(brand)))
        .map(|(name, _)| *name)
        .collect()
}

/// Whether a build under `profile` for `target_os` must refuse `unset` fields.
///
/// The rule the sentinel enforces is that no release is attempted against an
/// undetermined value. A release artefact of this project only exists for the
/// shipping platforms — Windows is tier 1 and macOS is tier 2, and Linux is the
/// deferred platform whose release-profile binary `.github/workflows/build.yml`
/// itself records as meeting no release entry's condition; it is a proof that
/// the release path compiles, not an artefact anyone can ship. So the gate
/// binds exactly where a release could come from: the release profile on a
/// shipping platform. Debug and test builds pass everywhere, which is what
/// lets the tree build and test while most of the real brand is honestly
/// `unset`.
pub fn release_gate_applies(profile: &str, target_os: &str) -> bool {
    profile == "release" && matches!(target_os, "windows" | "macos")
}
