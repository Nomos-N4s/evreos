# N10: the catalogue format's byte cost

**Question.** N10 left the FR-035 catalogue file format open until its byte
cost existed as a figure: a plain keyed table against `fluent-bundle` with its
plural and formatting dependencies. Principle II's terms — a feature that
cannot justify its cost is not added — require the figure before the
adoption, so this measurement was taken before either option was adopted and
the tree at the time of measurement named no format-specific extension.

**Answer.** The plain keyed table is adopted. On the tier-1 release build of
`evreos-shell` the plain table adds **16,896 bytes (0.016 MB)** and
`fluent-bundle` adds **121,856 bytes (0.116 MB)** — more than the whole
114,176-byte shell it was added to, and 7.2 times the plain table's cost —
while no shipped message uses the plural categories or formatting machinery
the difference buys: the nine keys are the FR-015 error states and FR-016a's
menu entry, all of them singular prose with named string arguments. Fluent
was not adopted before the figure existed, and is not adopted after it; a
later message that genuinely needs plural rules re-opens N10 with this file
as its baseline, and the adoption re-states the cost under FR-043 then.

## What was measured, and what deliberately was not

The figure is the **byte delta each option adds to the release build of
`evreos-shell`** — the binary itself, on the tier-1 host, under the
workspace's release profile (`opt-level = "z"`, `lto = "fat"`,
`codegen-units = 1`, `panic = "abort"`, `strip = "symbols"`).

It is stated so as to be comparable with the SC-001 entries it will be held
against. `scripts/check-budgets.py`'s one size measurement,
`measure_download_size()`, reads an artefact's byte size and states it as
`st_size / (1024 * 1024)` MB to three decimals; the megabyte figures here are
that same arithmetic over the same binary the installer will package. The
gate itself measures only "the installer artefact CI publishes", and neither
platform's installer exists, so a run of the gate on the measuring host
reports — as it must — that nothing was measured:

```
$ python scripts/check-budgets.py --allow-unpinned-runners --allow-unmeasured
  ...
    - SC-001 download size (windows)  (no measurement was produced; deferred by --allow-unmeasured)
    - SC-001 installed footprint (windows)  (no measurement was produced; deferred by --allow-unmeasured)
  measured: nothing on this win32 host; no .msi artefact under target/packaging/windows; the windows installer is not built yet
```

**Both SC-001 download-size entries therefore stay unmeasured-with-reason
until each platform's installer exists.** What this file records is the
measured byte delta of the two builds, and not an installed-footprint
measurement: SC-001's installed footprint is a disk delta after first run
completes, which needs an installed artefact no build here produced.

## How each figure was produced

Toolchain and host: `rustc 1.97.1` / `cargo 1.97.1`, Windows 11, tier-1
platform, 2026-09-05. Three builds of the same commit's tree (the commit that
created `crates/evreos-i18n`), each measured with:

```
cargo build --release -p evreos-shell
python -c "import os; print(os.stat('target/release/evreos-shell.exe').st_size)"
```

| build | evreos-shell.exe | gate's MB form | delta |
| --- | ---: | ---: | ---: |
| baseline: shell without `evreos-i18n` | 114,176 B | 0.109 MB | — |
| plain keyed table | 131,072 B | 0.125 MB | **+16,896 B (0.016 MB)** |
| `fluent-bundle` 0.15.3 | 236,032 B | 0.225 MB | **+121,856 B (0.116 MB)** |

The shell does not yet consume the catalogues — its error type lands at
T031 — so for both option builds a **temporary dependency was wired and
exercised**, exactly as follows, and reverted before anything was committed;
the shipping tree carries only the adopted option, and `fluent-bundle`
appears nowhere in the committed `Cargo.lock`.

**Both option builds** added to `crates/evreos-shell/Cargo.toml`:

```toml
evreos-i18n = { path = "../evreos-i18n" }
```

and to the top of `main()` in `crates/evreos-shell/src/main.rs`, so the
catalogues and the resolver are linked, reached and not eliminated as dead
code — the smoke run printed the resolved German, Greek and English text in
all three builds that link it:

```rust
for language in evreos_i18n::Language::ALL {
    let messages = evreos_i18n::catalogue(language);
    println!(
        "{} / {}",
        messages
            .resolve("menu.home_surface", &[])
            .expect("static label"),
        messages
            .resolve(
                "error.unresolvable.cause",
                &[("address", "example.invalid")]
            )
            .expect("interpolated"),
    );
}
```

**The plain-table build** is the committed crate as it stands: no
dependencies, the parser and resolver in `crates/evreos-i18n/src/lib.rs`,
catalogues embedded with `include_str!`.

**The fluent-bundle build** temporarily replaced
`crates/evreos-i18n/src/lib.rs` with a variant exposing the same two calls —
`catalogue(Language)` and `Catalogue::resolve(key, &[(name, value)])` — so
the shell-side wiring above compiled byte-identically, and temporarily added
to `crates/evreos-i18n/Cargo.toml`:

```toml
[dependencies]
fluent-bundle = "0.15"
unic-langid = "0.9"
```

The variant embedded the same nine messages per language as FTL string
constants — FTL identifiers admit no `.`, so `error.unresolvable.cause`
became `error-unresolvable-cause` and lookup mapped `.` and `_` to `-`; the
hyphenated spelling exists only inside that measurement build — and resolved
through Fluent's own machinery, one bundle per language:

```rust
let resource = FluentResource::try_new(ftl.to_owned()).unwrap_or_else(|_| panic!("FTL parses"));
let locale: LanguageIdentifier = language.subtag().parse().expect("subtag parses");
let mut bundle = FluentBundle::new_concurrent(vec![locale]);
bundle.set_use_isolating(false);
bundle.add_resource(resource).unwrap_or_else(|_| panic!("resource adds"));
// per resolution:
let message = bundle.get_message(&fluent_key)?;
let mut args = FluentArgs::new();
for (name, value) in arguments { args.set(*name, *value); }
let mut errors = Vec::new();
let resolved = bundle.format_pattern(message.value()?, Some(&args), &mut errors);
```

What that build linked, as `cargo tree -p evreos-i18n --edges normal`
resolved it (proc-macros compile at build time and add no shipped bytes):

```
fluent-bundle v0.15.3
├── fluent-langneg v0.13.1 → unic-langid v0.9.6 → unic-langid-impl v0.9.6 → tinystr v0.8.4
├── fluent-syntax v0.11.1 → thiserror v1.0.69
├── intl-memoizer v0.5.3 → type-map v0.5.1 → rustc-hash v2.1.3
├── intl_pluralrules v7.0.2
├── rustc-hash v1.1.0
├── self_cell v0.10.3 → self_cell v1.3.0
└── smallvec v1.16.0
```

## The stated cost under FR-043

The adopted option's cost against `budgets.toml`: **+16,896 bytes (0.016
MB)** on the tier-1 release binary, against SC-001's stated 20 MB download
size and 60 MB installed footprint — 0.08% of the download figure. The cost
is nine messages in three languages plus a parser and resolver of no
dependencies; it grows with the catalogues, whose bytes SC-001 counts like
any others.

**No baseline is written in this change, and that is not an omission.** The
budget file at the base of this branch (`git show b22fa9e:budgets.toml`)
carries both SC-001 download-size entries with `baseline = 0.0` — a baseline
no measurement has written yet, inert by that file's own schema — and their
condition names "the installer artefact CI publishes", which does not exist
on either platform. A baseline is written by the commit that first measures
the ENTRY, and the entry's measurement is the installer artefact, not the
bare release binary measured here: writing 0.125 MB into an entry whose
condition names an artefact seven orders of assembly away would be inventing
a measurement, so the entries stay at `baseline = 0.0`,
unmeasured-with-reason, until each platform's installer exists.

## Limits, stated so nothing is assumed

- One build per option, not a distribution: binary size under a fixed
  toolchain and profile is deterministic enough for a 7.2x separation to
  settle the question; nothing finer is claimed.
- Measured on tier 1 only. Tier 2's linker and its object format will move
  both absolute sizes; nothing suggests it moves the ordering, and the
  ordering is what was decided on.
- The fluent build resolved the same nine messages but not plural or number
  formatting, because no shipped message uses them — that is the ground of
  the decision, not a blind spot in it. A message that needs them cannot be
  expressed in the adopted format at all, which is what forces N10 open
  again rather than letting the need be met quietly.
