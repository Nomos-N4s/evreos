//! The brand configuration seam Principle VIII requires.
//!
//! No brand name, colour, endpoint or support address is hardcoded outside one
//! brand configuration (FR-042). The configuration is a file under `brands/`,
//! selected at build time by a cargo feature: the real brand,
//! `brands/evreos.toml`, by default, and `brands/fixture.toml` under
//! `fixture-brand`, which CI builds on every change so the seam is proved
//! rather than asserted. When both are enabled — `--all-features` — the
//! fixture wins, here and in `build.rs` alike, so the two never disagree about
//! which file a build embeds.
//!
//! The schema, the parser and the build-time validations live in
//! [`schema`], one file compiled both into this module and into `build.rs`,
//! so the function the build script calls is the function the tests below
//! prove. `build.rs` is what turns a bad file into a failed build: a missing
//! field fails every build, and an `unset` sentinel fails a release-profile
//! build for a shipping platform — see `schema::release_gate_applies` for why
//! the line sits exactly there.

// Compiled into two crates — this one and the build script — and each consumer
// uses part of it: the build script never composes a search request, and the
// running shell never re-runs the release gate. Dead-code analysis over either
// crate alone would therefore flag the other's half, so it is allowed here and
// the pairing is kept honest by the tests below instead.
#[allow(dead_code)]
mod schema;

pub use schema::Brand;

/// The selected brand file, embedded whole. `build.rs` has already parsed and
/// validated these same bytes, so the lazy parse below cannot fail in a build
/// that succeeded.
#[cfg(not(feature = "fixture-brand"))]
const SELECTED: &str = include_str!("../../../brands/evreos.toml");
#[cfg(feature = "fixture-brand")]
const SELECTED: &str = include_str!("../../../brands/fixture.toml");

static BRAND: std::sync::LazyLock<Brand> = std::sync::LazyLock::new(|| {
    schema::parse(SELECTED).expect("build.rs validated the embedded brand file")
});

/// The brand this build was made for.
pub fn brand() -> &'static Brand {
    &BRAND
}

/// A submitted search, composed but not sent: where it would go, and the whole
/// of what it would carry.
///
/// The two halves are separate fields because FR-003a draws the line between
/// them: changing the provider — by the member, or by brand configuration
/// under FR-042 — changes which service receives the query and MUST NOT change
/// what the query carries.
pub struct SearchRequest {
    /// The service that receives the query: the brand's search endpoint,
    /// verbatim.
    pub endpoint: String,
    /// What the request carries: the terms the member submitted, encoded, and
    /// nothing else — no address, no history, no identifier Evreos assigns.
    pub query: String,
}

/// Compose the FR-003a submitted search for `terms` against `brand`.
///
/// The query is built from `terms` alone. Nothing of the brand reaches it,
/// which is what the `changing_the_provider_changes_only_the_receiver` test
/// proves by composing the same terms against two brands that differ in every
/// field.
pub fn search_request(brand: &Brand, terms: &str) -> SearchRequest {
    SearchRequest {
        endpoint: brand.search_endpoint.clone(),
        query: format!("q={}", percent_encode(terms)),
    }
}

/// Percent-encode `terms` for a query value: unreserved bytes pass, everything
/// else is `%XX` over the UTF-8 encoding. Hand-rolled because the release path
/// takes no dependency for twelve lines of stdlib.
fn percent_encode(terms: &str) -> String {
    let mut out = String::with_capacity(terms.len());
    for byte in terms.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::schema::{FIELDS, UNSET, is_set, parse, release_gate_applies, unset_fields};
    use super::{brand, search_request};

    /// Both committed brand files, embedded so the tests read the bytes a
    /// build embeds rather than whatever is on disk when the tests run.
    const EVREOS: &str = include_str!("../../../brands/evreos.toml");
    const FIXTURE: &str = include_str!("../../../brands/fixture.toml");

    /// A complete, well-formed source: every field set to a value derived from
    /// its own name, with `overrides` applied and `skip` omitted.
    fn source(overrides: &[(&str, &str)], skip: &[&str]) -> String {
        FIELDS
            .iter()
            .map(|(name, _)| *name)
            .filter(|name| !skip.contains(name))
            .map(|name| {
                let value = overrides
                    .iter()
                    .find(|(field, _)| field == &name)
                    .map_or_else(|| format!("value-{name}"), |(_, value)| value.to_string());
                format!("{name} = \"{value}\"\n")
            })
            .collect()
    }

    #[test]
    fn both_committed_brand_files_parse() {
        parse(EVREOS).expect("brands/evreos.toml parses");
        parse(FIXTURE).expect("brands/fixture.toml parses");
    }

    #[test]
    fn the_selected_brand_is_readable_at_runtime() {
        // Whichever feature selected it, the embedded brand parses and every
        // field is present -- absence would have failed the build already, and
        // this is the runtime spelling of the same fact.
        assert!(!brand().product_name.is_empty());
    }

    #[test]
    fn the_fixture_brand_has_every_field_set() {
        // T034's CI step builds the fixture in the release profile, so one
        // `unset` here is a red build on every change -- deliberately.
        let fixture = parse(FIXTURE).expect("brands/fixture.toml parses");
        assert_eq!(unset_fields(&fixture), Vec::<&str>::new());
    }

    #[test]
    fn the_real_brand_sets_the_settled_search_endpoint() {
        // Q-E2 settles DuckDuckGo as the default provider, held to FR-003a's
        // boundary. This is the one field of the real brand with a settled
        // value today; brands/README.md records what fills the others.
        let evreos = parse(EVREOS).expect("brands/evreos.toml parses");
        assert_eq!(evreos.search_endpoint, "https://duckduckgo.com/");
        assert!(is_set(&evreos.search_endpoint));
    }

    #[test]
    fn the_two_brands_differ_in_every_set_field() {
        let evreos = parse(EVREOS).expect("brands/evreos.toml parses");
        let fixture = parse(FIXTURE).expect("brands/fixture.toml parses");
        let mut compared = 0;
        for (name, get) in &FIELDS {
            let (real, fake) = (get(&evreos), get(&fixture));
            if is_set(real) && is_set(fake) {
                assert_ne!(real, fake, "field `{name}` is identical in both brands");
                compared += 1;
            }
        }
        // Not vacuous: at least the settled search endpoint is set in both.
        assert!(compared >= 1, "no field is set in both brands");
    }

    #[test]
    fn every_field_is_required_and_none_defaults() {
        for (name, _) in &FIELDS {
            let error = parse(&source(&[], &[name]))
                .expect_err("a brand file missing a field must not parse");
            assert!(
                error.contains(name),
                "the error for a missing `{name}` does not name it: {error}"
            );
        }
    }

    #[test]
    fn unknown_fields_are_refused() {
        let mut text = source(&[], &[]);
        text.push_str("serach_endpoint = \"https://typo.invalid/\"\n");
        let error = parse(&text).expect_err("an unknown field must not parse");
        assert!(error.contains("serach_endpoint"), "unnamed: {error}");
    }

    #[test]
    fn duplicate_fields_are_refused() {
        let mut text = source(&[], &[]);
        text.push_str("product_name = \"again\"\n");
        let error = parse(&text).expect_err("a duplicated field must not parse");
        assert!(error.contains("product_name"), "unnamed: {error}");
    }

    #[test]
    fn values_outside_the_restricted_schema_are_refused() {
        for bad in [
            "product_name = unquoted",
            "product_name = \"open",
            "product_name = \"a\" # trailing comment",
            "product_name = \"a \\\" b\"",
            "product_name = \"\"",
            "just some words",
            "[table]",
        ] {
            let text = format!("{}{bad}\n", source(&[], &["product_name"]));
            assert!(parse(&text).is_err(), "accepted: {bad}");
        }
    }

    #[test]
    fn comments_and_blank_lines_are_ignored() {
        let text = format!("# a comment\n\n{}# another\n", source(&[], &[]));
        parse(&text).expect("comments and blank lines are not content");
    }

    #[test]
    fn the_sentinel_is_exact() {
        assert!(!is_set(UNSET));
        // Any other spelling is an ordinary value the release gate passes, so
        // the sentinel's exactness is a stated rule rather than an accident.
        assert!(is_set("Unset"));
        assert!(is_set("UNSET"));
    }

    #[test]
    fn the_release_gate_names_every_unset_field() {
        for (name, _) in &FIELDS {
            let brand = parse(&source(&[(name, UNSET)], &[]))
                .expect("the sentinel is a valid parse-time value");
            assert_eq!(unset_fields(&brand), vec![*name]);
        }
    }

    #[test]
    fn the_release_gate_binds_shipping_targets_only() {
        // The release profile on a shipping platform is where a release
        // artefact can come from; the Linux release-profile binary is
        // build.yml's proof that the release path compiles, not an artefact,
        // and debug builds are how the tree works while fields are unset.
        assert!(release_gate_applies("release", "windows"));
        assert!(release_gate_applies("release", "macos"));
        assert!(!release_gate_applies("debug", "windows"));
        assert!(!release_gate_applies("debug", "macos"));
        assert!(!release_gate_applies("release", "linux"));
        assert!(!release_gate_applies("debug", "linux"));
    }

    #[test]
    fn the_search_request_carries_only_the_encoded_terms() {
        let brand = parse(&source(&[("search_endpoint", "https://a.invalid/")], &[]))
            .expect("a complete source parses");
        let request = search_request(&brand, "hello wörld");
        assert_eq!(request.endpoint, "https://a.invalid/");
        assert_eq!(request.query, "q=hello%20w%C3%B6rld");
    }

    #[test]
    fn changing_the_provider_changes_only_the_receiver() {
        // Two brands that differ in EVERY field, so the equality below proves
        // the query is a function of the terms alone: were any brand value
        // woven in, some field's difference would surface in it.
        let first = parse(&source(&[], &[])).expect("a complete source parses");
        let second: String = FIELDS
            .iter()
            .map(|(name, _)| format!("{name} = \"other-{name}\"\n"))
            .collect();
        let second = parse(&second).expect("a complete source parses");
        let (a, b) = (
            search_request(&first, "same terms"),
            search_request(&second, "same terms"),
        );
        assert_ne!(a.endpoint, b.endpoint);
        assert_eq!(a.query, b.query);
    }

    #[test]
    fn field_accessors_cover_every_field_distinctly() {
        // FIELDS is the one enumeration everything walks, so an accessor
        // repeated or misdirected would silently exempt a field from the
        // missing-field and release gates. Ten distinct values in, ten
        // distinct values out proves each accessor reads its own field; the
        // count is asserted so a Brand field added without a FIELDS row is a
        // red test rather than an unvalidated value.
        let brand = parse(&source(&[], &[])).expect("a complete source parses");
        let mut seen: Vec<&str> = FIELDS.iter().map(|(_, get)| get(&brand)).collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), 10);
    }
}
