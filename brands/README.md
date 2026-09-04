# Brand configurations

This directory is the one place a brand exists. Principle VIII of
`.specify/memory/constitution.md` and FR-042 of `specs/001-evreos-v1/spec.md`
allow no brand name, colour, endpoint or support address to be hardcoded
outside a single brand configuration, and these files are that configuration:

| File | Role |
| --- | --- |
| `evreos.toml` | the real brand, selected by default |
| `fixture.toml` | a complete fictional brand, built by CI on every change |

`crates/evreos-shell/src/brand.rs` embeds exactly one of them, chosen by the
`fixture-brand` cargo feature on `evreos-shell` — the real brand with the
feature off, the fixture with it on, and the fixture when both are enabled so
`--all-features` builds mean one thing. `scripts/checks/check_brand.py` reads
both files and fails the build when any set value appears in workspace Rust
source outside this directory and that one module, which is what keeps "no
brand outside the seam" a checked fact rather than a review obligation.

## The format

TOML restricted to `key = "string"` lines, plus full-line `#` comments and
blank lines. No tables, no arrays, no escapes, no trailing comments. The
restriction is deliberate: the parser in `crates/evreos-shell/src/brand/schema.rs`
is shared with a build script that must not grow dependencies, and a page of
stdlib-only code can read this format exactly.

Every field is required. A brand file missing one fails the build rather than
defaulting — `crates/evreos-shell/build.rs` parses both files on every build of
`evreos-shell` and refuses a file the schema rejects, naming the line or the
field.

## The `unset` sentinel

`unset` — exact and lowercase — marks a field whose real value is undetermined
or owned outside this repository. It is never replaced by a plausible-looking
placeholder, which is how a wrong endpoint ships: a build against `unset`
either works (debug, tests) or refuses loudly (release), and nothing
in between.

The refusal is scoped to where a release can come from: a release-profile
build for a shipping platform — Windows (tier 1) or macOS (tier 2) — fails
while the selected brand carries any `unset` field, naming each one. The
Linux release-profile build passes deliberately: Linux is the deferred
platform, and `.github/workflows/build.yml` itself records that its
release-profile binary meets no release entry's condition — it is CI's proof
that the release path compiles, not an artefact anyone can ship. Debug and
test builds pass everywhere, which is what lets the tree build and test while
most of the real brand is honestly unset.

## What fills each field of `evreos.toml`

Which task or decision fills a field is bookkeeping, not a founder decision,
so it is recorded here rather than in the `docs/decisions/` register — that
register's charter records founder choices that other files must cite by a
stable name, and this table is neither. Task numbers refer to
`specs/001-evreos-v1/tasks.md`.

| Field | Status | Filled by |
| --- | --- | --- |
| `product_name` | unset | Founder decision on standalone versus endorsed branding, after the Q-E7 trademark clearance T210 runs; recorded at `docs/decisions/` when taken. |
| `money_service_name` | unset | The same Q-E7 clearance and founder decision as `product_name`. |
| `primary_colour` | unset | Founder decision on the palette; no task carries it; recorded at `docs/decisions/` when taken. |
| `accent_colour` | unset | Same palette decision as `primary_colour`. |
| `search_endpoint` | **set** | Q-E2: DuckDuckGo, held to FR-003a's boundary. |
| `money_endpoint` | unset | The Apivo service's base endpoint, owned outside this repository; recorded with the Apivo contract findings under `specs/001-evreos-v1/contracts/` when the service publishes it. The User Story 2 request builders resolve against it through `crates/evreos-net`, never from a literal. |
| `update_endpoint` | unset | The update channel whose terms T206 records at `specs/001-evreos-v1/contracts/update-channel-terms.md` and which T207 implements against. |
| `surface_endpoint` | unset | The FR-019 surface-delivery service T145 consumes through `crates/evreos-net`. |
| `diagnostics_endpoint` | unset | The FR-039b relay ingress of the operator T198 contracts and records at `docs/diagnostics/relay-operator.md`. |
| `support_address` | unset | Founder decision; no task carries it; recorded at `docs/decisions/` when taken. |

Filling a field is editing `evreos.toml` in the change that settles the value,
citing the record above. Adding a field is a schema change: the `Brand` struct
and `FIELDS` table in `crates/evreos-shell/src/brand/schema.rs` first, both
files here second, and the unit tests in `crates/evreos-shell/src/brand.rs`
hold the two in step.

## The fixture's values

The fixture's values are its own and mean nothing. Hosts sit under the
reserved `.invalid` top-level domain so they can never resolve; the names are
invented words. Every set field must differ from every set field of
`evreos.toml` — a shared value would let a hardcoded copy of it hide from
`scripts/checks/check_brand.py` behind the wrong file — and the
`the_two_brands_differ_in_every_set_field` unit test enforces that.
