//! The FR-035 closure properties, proved over the embedded catalogues.
//!
//! Three things must hold of every catalogue this crate ships: every message
//! key is present in all three languages, so no member's language is the one
//! with a hole in it; every message resolves with no account, no network and
//! no Apivo state, because FR-016a's neutral menu entry is drawn from here
//! and must render at first run on a fresh profile; and no catalogue
//! filename, catalogue key or message key carries a region subtag, which is
//! FR-035's "keyed by the primary language subtag alone" — a rule `de-DE`
//! would satisfy in name and refuse in substance.
//!
//! Where these tests spell a forbidden fused value, they assemble it from
//! parts. The tree-wide check at `scripts/checks/check_language_place.py`
//! reads string literals in every Rust source file, this file included, and a
//! literal counter-example here would be indistinguishable from a breach.

use evreos_i18n::{Catalogue, CatalogueError, Language, Place, ResolveError, catalogue};

/// A value for every argument name a message could reasonably interpolate.
/// Built from the catalogue's own answer, not guessed: `arguments` names what
/// `resolve` will demand.
fn values_for(names: &[&str]) -> Vec<(String, String)> {
    names
        .iter()
        .map(|name| ((*name).to_owned(), format!("value-of-{name}")))
        .collect()
}

#[test]
fn every_message_key_is_present_in_all_three_catalogues() {
    // FR-035 requires the text in German, Greek and English — all three, not
    // whichever a change remembered. Comparing whole key sets in both
    // directions reports an extra key as loudly as a missing one.
    let keys: Vec<Vec<&str>> = Language::ALL
        .iter()
        .map(|&language| catalogue(language).keys().collect())
        .collect();
    for (language, its_keys) in Language::ALL.iter().zip(&keys) {
        assert_eq!(
            its_keys,
            &keys[0],
            "the {} catalogue does not hold the same keys as the {} catalogue",
            language.subtag(),
            Language::ALL[0].subtag(),
        );
    }
    assert!(
        !keys[0].is_empty(),
        "a catalogue with no keys proves nothing; the closure tests would pass vacuously"
    );
}

#[test]
fn every_message_resolves_with_no_account_no_network_and_no_apivo_state() {
    // Resolution is a pure function over text compiled into this binary: this
    // test process has signed into nothing, fetched nothing and holds no
    // Apivo state, and it is the whole environment resolution gets. The
    // arguments come from the catalogue's own declaration, so a message
    // gaining an argument cannot make this test lie about completeness.
    for language in Language::ALL {
        let messages = catalogue(language);
        for key in messages.keys().collect::<Vec<_>>() {
            let names = messages.arguments(key).expect("key came from keys()");
            let values = values_for(&names);
            let borrowed: Vec<(&str, &str)> = values
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str()))
                .collect();
            let resolved = messages.resolve(key, &borrowed).unwrap_or_else(|error| {
                panic!("{key} did not resolve in {}: {error}", language.subtag())
            });
            assert!(
                !resolved.trim().is_empty(),
                "{key} resolved to nothing in {}",
                language.subtag()
            );
        }
    }
}

#[test]
fn the_neutral_menu_entry_renders_at_first_run_in_every_language() {
    // FR-016a: a single neutral entry point to the home surface, a static
    // label drawn from these catalogues, present from first run. Static means
    // no arguments — a label that needed one would be waiting on state, and
    // first run has none to give it.
    for language in Language::ALL {
        let messages = catalogue(language);
        assert_eq!(
            messages
                .arguments("menu.home_surface")
                .expect("the menu entry exists"),
            Vec::<&str>::new(),
            "the menu entry label must be static in {}",
            language.subtag()
        );
        let label = messages
            .resolve("menu.home_surface", &[])
            .expect("the menu entry resolves from nothing");
        assert!(!label.trim().is_empty());
    }
}

#[test]
fn the_four_error_causes_each_carry_a_cause_and_a_next_step() {
    // FR-015: an error state names the cause and offers a next step, for each
    // of the four enumerated failures. The keys are the contract the shell's
    // error type (T031) resolves against, so their existence is pinned here.
    for cause in [
        "unresolvable",
        "certificate",
        "intercepted",
        "authentication",
    ] {
        for part in ["cause", "next_step"] {
            let key = format!("error.{cause}.{part}");
            for language in Language::ALL {
                assert!(
                    catalogue(language).keys().any(|held| held == key),
                    "{key} is missing from the {} catalogue",
                    language.subtag()
                );
            }
        }
    }
}

#[test]
fn no_catalogue_filename_carries_a_region_subtag() {
    // The files are read from the source tree rather than through the crate,
    // because the crate cannot see its own filenames after include_str! — and
    // the filename is exactly where `de-DE` crept into every localisation
    // scheme this rule descends from. One file per language, named by the
    // primary subtag alone, nothing else in the directory.
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("catalogues");
    let mut stems: Vec<String> = std::fs::read_dir(&directory)
        .expect("the catalogues directory is part of this crate")
        .map(|entry| entry.expect("readable directory entry").file_name())
        .map(|name| {
            let name = name
                .to_str()
                .expect("catalogue filenames are ASCII")
                .to_owned();
            let (stem, _extension) = name.split_once('.').unwrap_or((name.as_str(), ""));
            assert!(
                stem.len() >= 2
                    && stem.len() <= 3
                    && stem.bytes().all(|byte| byte.is_ascii_lowercase()),
                "{name} is not named by a primary language subtag alone"
            );
            stem.to_owned()
        })
        .collect();
    stems.sort();
    let mut subtags: Vec<&str> = Language::ALL
        .iter()
        .map(|language| language.subtag())
        .collect();
    subtags.sort();
    assert_eq!(
        stems, subtags,
        "one catalogue file per language, no more, no fewer"
    );
}

#[test]
fn no_catalogue_key_or_message_key_carries_a_region_subtag() {
    // The parser refuses uppercase, digits and hyphens in a key, so a region
    // subtag — which needs one of the three — is unrepresentable. This test
    // re-proves the property over the shipped keys rather than trusting the
    // parser's promise, because the parser is code this same change wrote.
    for language in Language::ALL {
        for key in catalogue(language).keys() {
            assert!(
                key.bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'.' || byte == b'_'),
                "{key} in {} strays outside lowercase ASCII, `.` and `_`, which is \
                 room enough for a region subtag",
                language.subtag()
            );
        }
    }
}

#[test]
fn the_parser_refuses_a_key_with_a_region_subtag() {
    // The counter-example is assembled, not written literally: this file is
    // Rust source the tree-wide check reads, and a fused literal here would
    // read as a breach. The assembled key is the shape T030's tests name.
    let fused = ["wallet.de", "AT.title"].join("-");
    let text = format!("{fused} = something");
    let refused = Catalogue::parse(Language::De, &text);
    assert!(
        matches!(refused, Err(CatalogueError::InvalidKey { line: 1, .. })),
        "a region subtag in a key must be refused at parse: {refused:?}"
    );
}

#[test]
fn the_parser_refuses_what_a_catalogue_must_not_hold() {
    let uppercase = Catalogue::parse(Language::En, "Menu.title = x");
    assert!(matches!(
        uppercase,
        Err(CatalogueError::InvalidKey { line: 1, .. })
    ));

    let duplicated = Catalogue::parse(Language::En, "menu.a = x\nmenu.a = y");
    assert!(matches!(
        duplicated,
        Err(CatalogueError::DuplicateKey { line: 2, .. })
    ));

    let blank = Catalogue::parse(Language::En, "menu.a =");
    assert!(matches!(
        blank,
        Err(CatalogueError::EmptyMessage { line: 1, .. })
    ));

    let separatorless = Catalogue::parse(Language::En, "menu.a");
    assert!(matches!(
        separatorless,
        Err(CatalogueError::MissingSeparator { line: 1 })
    ));

    let unclosed = Catalogue::parse(Language::En, "menu.a = hello {name");
    assert!(matches!(
        unclosed,
        Err(CatalogueError::InvalidPlaceholder { line: 1, .. })
    ));

    let stray = Catalogue::parse(Language::En, "menu.a = hello } there");
    assert!(matches!(
        stray,
        Err(CatalogueError::InvalidPlaceholder { line: 1, .. })
    ));

    let empty_name = Catalogue::parse(Language::En, "menu.a = hello {}");
    assert!(matches!(
        empty_name,
        Err(CatalogueError::InvalidPlaceholder { line: 1, .. })
    ));
}

#[test]
fn named_arguments_interpolate_by_name_never_by_position() {
    // FR-042: a brand name enters a message as an argument, so the message
    // text stays brand-free. Interpolation is by name — the caller's order
    // cannot matter, and a value the message does not name is ignored rather
    // than landing in the wrong hole.
    let messages = catalogue(Language::En);
    let resolved = messages
        .resolve(
            "error.unresolvable.cause",
            &[("unrelated", "ignored"), ("address", "printer.local")],
        )
        .expect("resolves with its named argument supplied");
    assert!(
        resolved.contains("printer.local"),
        "the supplied argument value must appear: {resolved}"
    );
    assert!(
        !resolved.contains("ignored"),
        "a value the message does not name must not leak in: {resolved}"
    );
    assert!(
        !resolved.contains("{address}"),
        "the placeholder must be consumed: {resolved}"
    );
}

#[test]
fn withholding_a_named_argument_is_an_error_not_a_hole() {
    // Rendering a template with a hole where a brand name belongs is FR-042's
    // rule half-kept, so a missing argument fails and names itself.
    let refused = catalogue(Language::De).resolve("error.certificate.cause", &[]);
    assert_eq!(
        refused,
        Err(ResolveError::MissingArgument {
            key: "error.certificate.cause".to_owned(),
            name: "host".to_owned(),
        })
    );
}

#[test]
fn an_unknown_key_is_an_error_in_every_language_alike() {
    // The completeness test above makes the key sets identical, so unknown
    // here means unknown everywhere — a member's language cannot be the
    // variable that decides whether text renders.
    for language in Language::ALL {
        let refused = catalogue(language).resolve("no.such.key", &[]);
        assert_eq!(
            refused,
            Err(ResolveError::UnknownKey {
                key: "no.such.key".to_owned(),
            })
        );
    }
}

#[test]
fn the_language_enum_is_the_three_primary_subtags_and_nothing_longer() {
    // The closed enum is the catalogue key type. Its serialised form is the
    // primary subtag alone, which is what keeps every stored preference and
    // every request parameter regionless by construction.
    let subtags: Vec<&str> = Language::ALL
        .iter()
        .map(|language| language.subtag())
        .collect();
    assert_eq!(subtags, ["de", "el", "en"]);
    for subtag in subtags {
        assert!(
            subtag.len() == 2 && subtag.bytes().all(|byte| byte.is_ascii_lowercase()),
            "{subtag} is not a bare primary language subtag"
        );
    }
}

#[test]
fn a_place_is_two_uppercase_letters_and_never_a_fused_tag() {
    // FR-035: language and place are two values. The place slot holds a place
    // code and refuses everything else — a lowercase value that could be a
    // language, and any assembled fusion of the two. The fused counter-example
    // is assembled for the reason the parser test assembles its key.
    assert_eq!(
        Place::new("AT").map(|place| place.code().to_owned()),
        Ok("AT".to_owned())
    );

    assert!(
        Place::new("at").is_err(),
        "lowercase is a language's spelling, not a place's"
    );
    assert!(
        Place::new("AUT").is_err(),
        "three letters is not the alpha-2 shape"
    );
    assert!(Place::new("").is_err());

    let fused = ["de", "AT"].join("-");
    assert!(
        Place::new(&fused).is_err(),
        "a fused language-place tag must not fit in the place slot"
    );
}
