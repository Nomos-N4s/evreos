# Implementation Plan: Evreos v1

**Branch**: `docs/evreos-v1-plan` | **Date**: 2026-08-31 | **Spec**:
[specs/001-evreos-v1/spec.md](./spec.md)

**Input**: Feature specification from `/specs/001-evreos-v1/spec.md`

Spec Kit resolves this feature by directory — `specs/001-evreos-v1`, recorded in
`.specify/feature.json` — never by branch, so the branch above carries a purpose
prefix rather than the feature number.

**Note**: This template is filled in by the `/speckit-plan` command; its
definition describes the execution workflow.

## Summary

**The primary requirement is User Story 1**: signed out, with every Apivo
surface ignored, Evreos is a genuinely good private browser on a machine that
struggles with mainstream browsers — tabs and session restore, a combined entry
field, bookmarks, history, downloads, tracker and advert blocking active on
first launch with a per-site control, honest error states, and import from an
existing profile — inside the budgets Principle II makes law. That story is the
only one viable on its own, and Principle IV makes it the default experience
rather than a degraded mode. The super-app stories build on it: money surfaces
that render ledger state and never compute it (Story 2), and signed app surfaces
updated server-side without a browser release (Story 3).

**The technical approach the research settled.** Web content is hosted in the
operating system's own webviews through the `wry` crate, as ADR-0001 decided,
and reached only through the `Engine` interface the shell defines as its
consumer, with a headless second implementation kept working from M0 (Principle
III, FR-044). Phase 0 read the merged trait against both shipping backends and
found it the wrong shape; **the trait is therefore reshaped before the first
platform backend is written**, and the ordering below is the plan's central
commitment. Tier 1 needs no fork — Evreos registers its own
`add_NavigationCompleted`, `add_ServerCertificateErrorDetected` and
`add_BasicAuthenticationRequested` beside `wry`'s, over the `ICoreWebView2`,
`ICoreWebView2Controller` and `ICoreWebView2Environment` handles `wry` exposes.
Tier 2 needs no fork either: `WebViewExtMacOS::webview()` returns `wry`'s own
webview type, which is declared `#[unsafe(super(WKWebView))]` and so *is* a
`WKWebView`, meaning `setNavigationDelegate:` can be called on it;
`navigationDelegate` is a single-slot property, so installing ours displaces
`wry`'s, and the six behaviours that costs are all ones a browser shell wants to
own. Blocking is a policy surface on the seam, not a per-request veto, because
tier 1 decides per request in the host process while tier 2 at the macOS 13
floor decides inside WebKit from a precompiled rule list. Every outbound request
the shell makes goes through one crate carrying a closed `Purpose` enum; engine
traffic is not lintable, so SC-014's capture is its only instrument. App
surfaces are signed over a fixed-layout preimage, verified before rendering or
caching, served to the engine from shell memory, and make no network requests of
their own. Money surfaces ship in the shell and are held to Principle V in the
type system.

**Ordering, and it inverts the obvious order.**

- **T1 — reshape the `Engine` trait**, moving `evreos-engine-headless` in the
same commits, as FR-044 requires.
- **T2 — the conformance battery** as a feature-gated module inside
`evreos-engine`, which the headless implementation passes. Still no `unsafe`
anywhere.
- **T3 — `evreos-engine-webview`, Windows only**, with the `unsafe` carve-out
landing on the same commit as the first FFI line; it passes the battery and
SC-009's four causes on the tier-1 runner.
- **T4 — the macOS delegate replacement**, then the same battery on the tier-2
runner.
- **T5 — Q-E12's whole tier-2 "reach WebKit past `wry`" route** — compiled rule
lists, find-in-page, page zoom, the per-site control — rides in the crate T4
created and is sized once.

Two workstreams land early with no platform risk and are exercisable entirely
against the headless engine: the app verification core (preimage format
document, verifier, registry, catalogue, capability intersection, version floor,
cache, and the FR-019b artefact gate), and the blocking corpus and conversion
work, which produces the rule-count gate and the conversion-failure taxonomy
gate on ordinary CI. Three things run in parallel from Phase 1 because they are
longer-lead than any code: runner procurement, the FR-039b relay contract and
the DPIA, and two cheap bring-up measurements (N1's engine idle floor, and the
cold-start engine-initialisation floor SC-002's four provisional entries already
wait on) whose bad answers are specification amendments rather than code
changes.

## Technical Context

**Language/Version**: Stable Rust, edition 2024, with the floor checked in —
`rust-version = "1.85"` under `[workspace.package]` in `Cargo.toml`, so a
toolchain that cannot build the release path fails at resolve time rather than
at link time. No nightly features on the release path (Principle III, FR-044).
The workspace sets `[workspace.lints.rust] unsafe_code = "forbid"` and each of
the three crates repeats `#![forbid(unsafe_code)]`. Python 3.11 or later for the
budget gate (`tomllib`).

**Primary Dependencies**: None today — `Cargo.lock` resolves to exactly three
packages, all local path dependencies, and the workspace pulls no third-party
crate at all. Planned, and named only where the research established the choice:

- **`wry`** — the engine route ADR-0001 records, hosting WebView2 on Windows and
WKWebView on macOS. Every platform crate a backend needs is already inside `wry`
0.56.1's own graph at the versions it pins — `windows`, `windows-core`,
`webview2-com`, `webview2-com-sys` on Windows; `objc2`, `objc2-foundation`,
`objc2-web-kit`, `objc2-app-kit`, `block2` on macOS — so the backend adds no
third-party tree beyond what the engine decision already committed to.
- **The windowing crate is deliberately not named here.** ADR-0001 makes it a
  free
variable and an output of spike S4, and records that `wry` no longer depends on
`tao` — at 0.56.1 `tao` is a dev-dependency only, with `tao-macros` a real
dependency on Android alone — so "wry/tao" is not a package deal. On tiers 1 and
2 `wry`'s `build`/`build_as_child` are generic over `HasWindowHandle` alone,
which both `tao` 0.36 and `winit` 0.30 satisfy; only the deferred Linux platform
constrains further.
- **`adblock`** — the native matching engine on tier 1 and the
content-blocking conversion for tier 2's compiled rule lists.
- **`ed25519-dalek`**, used with strict verification, for FR-017 and FR-019a
signatures over a fixed-layout, length-prefixed, domain-separated preimage.
- **An OHTTP-over-HPKE client** for FR-039b, with the key configuration compiled
into the release rather than fetched.
- **A localisation format for FR-035** — Fluent is the candidate, one bundle per
primary language subtag; a plain keyed table is the alternative. This one is
*indicative*, not established: the byte cost against SC-001 is unmeasured (N10)
and FR-043 requires the pull request to state it.

**Storage**: Local files only, in five residence classes the data model fixes.
Profile-local and never transmitted in any form, derived or not: history,
bookmarks, downloads, session, site permissions, per-site blocking exceptions,
the suggestion index. The account credential lives in the operating system's
secure credential store and nowhere else (FR-023) — no Evreos profile file,
database, preference store, cache or log may hold it or anything from which it
can be reconstructed, and where that store is unavailable the member stays
signed out. The diagnostic enrolment state lives outside the browsing profile,
in the per-user application-data directory, so that clearing browsing data
cannot cause a re-enrolment — which the FR-039 pre-consent disclosure must
therefore state. Compiled blocking artefacts (the `WKContentRuleListStore`
directory on tier 2, the serialised `adblock` engine on tier 1) are product data
materialised at first run and land inside SC-001's installed-footprint
measurement. Money state is remote-owned: any wallet value held on the device is
typed as stale and carries the time it was received (FR-026a). FR-012's import
implies reading Chrome, Firefox and Edge profile stores; that dependency's byte
cost is unmeasured (N10). No server-side store of anything this client holds.

**Testing**: `cargo test --all`, `cargo fmt --all --check` and `cargo clippy
--all-targets --all-features -- -D warnings`, all three run in CI before
anything else. Today's tests are
`crates/evreos-shell/tests/navigation_failures.rs`, which exercises FR-015's
four causes against the headless engine, and `scripts/test_check_budgets.py`.
Planned: the conformance battery both `Engine` implementations run (a plan
decision, not a requirement — nothing today forces the two implementations to
*mean* the same thing); `trybuild` compile-fail tests asserting that `a + b` and
`iter.sum()` do not typecheck on the money types; the FR-007a network-capture
test committed to this repository and run in CI; the FR-016a two-profile diff
test over the neutral menu entry; the FR-019b release-artefact scan and
post-install offline test. Two test kinds are **not portable CI jobs**: SC-014's
capture must run against the real engine on the tier's real platform, with a
harness CA trusted only in the test image and DNS captured as well as TLS
payloads, and every hardware-dependent measurement runs on that tier's pinned
benchmark runner.

**Target Platform**: **Tier 1 — Windows 11 and later**; release criteria apply
in full, and the evergreen system runtime means this floor does not set the
rendering engine version. **Tier 2 — macOS 13 and later**; because the engine is
the operating system's own, this floor sets the engine version, the web features
available and the security-patch source. **Linux — deferred**, not part of v1
cross-platform scope, with its own budget and its own go/no-go. Windows ships
first (Q-E1). No mobile platform in v1.

**Project Type**: Desktop application — one Rust workspace, a set of library
crates and a single shell binary hosting the operating system's webviews. Not a
service, and nothing in this repository runs on a server; the receiving service
and relay FR-039b describes, and the Apivo API, are outside it.

**Performance Goals**, with each entry's status as the Success Criteria preamble
records it. Thirteen of the eighteen entries are ratified and tighten-only; five
are provisional.

| Criterion | Figure | Status |
| --- | --- | --- |
| SC-001 download size | 20 MB per platform | ratified (both entries) |
| SC-001 installed footprint | 60 MB per platform, disk delta after first run completes | ratified (both entries) |
| SC-002 warm start | 800 ms | **provisional**, both platforms, pending the cold-start spike |
| SC-002 cold start | 2 s | **provisional**, both platforms, pending the cold-start spike |
| SC-004 ten-tab memory | 150 MB at every 5-second sample, over a soak of at least 8 hours | ratified on tier 1; **provisional** on tier 2 |
| SC-005 60-minute window | below 0.5% of one core, which at that scale is 18 s of processor time | ratified |
| SC-005 wake-free 1-second sample | below 0.5% of one core, which at that scale is 5 ms | ratified |
| SC-006 tab switch | 16 ms at the 99th percentile of at least 1000 trials, and no trial over 16 ms at all | ratified |
| SC-006 address-field keystroke | the same | ratified |

SC-003 states a required experience rather than a figure, carries no budget
entry and no budget gate. The business and trust criteria are measured after
release and gate no build: SC-010 at 25% of people offered Evreos at the pilot
counter (not measurable until FR-029 redemption is enabled, Q-E11a); SC-011's
signed-in retention provisional at 40%, judged only on cohorts of at least 200
first sign-ins, reported separately from the signed-out figure and never
blended; SC-012 at one cashback activation per active member per calendar month.

**Constraints**: Every budget above is a CI gate from M0, not a target. Browsing
history may leave the machine only through the four transmissions FR-007a
enumerates, and that list is exhaustive — adding an entry is a specification
amendment made in the pull request that would add the transmission. Nothing is
injected into a web page without a member action taken in the browser's own
chrome, addressed to the specific thing it authorises, on the page load it
authorises; no advertising is placed in a page under any arrangement, with or
without a member action. No balance is computed, no affiliate deeplink built, no
money logic held in the client. Telemetry and crash reporting are opt-in,
identifier-free, aggregate and EU-hosted, and the diagnostic signal may not be
offered at all until a relay operator is named and contracted. WCAG 2.1 AA, full
keyboard operation and 200% scaling are release criteria. Interface text is
keyed by BCP-47 primary language subtag alone, with place never fused into it.
No brand name, colour, endpoint or support address outside one brand
configuration. No bundled engine, and no engine Evreos itself fetches, unpacks
or installs.

**Scale/Scope**: 61 functional requirements and 15 success criteria, against ten
constitutional principles and three Permanent Prohibitions. Eighteen budget
entries — nine per platform — of which `budgets.toml` carries four. Two shipping
platforms, three interface languages. Three crates exist on `main`; the design
adds fourteen shipped crates and two dev-only ones. Four specification spikes
(Q-E10, Q-E11, Q-E11b, Q-E12), two further spikes named where their figures are,
one ADR spike (S4), and twelve new measurements this plan opens (N1–N12). A solo
founder is the whole engineering capacity, which is why every "measure it later"
in this plan is written as a scheduled measurement with a named runner rather
than as an intention.

### Open items — NEEDS CLARIFICATION

Each of these is genuinely unsettled. None is answered here to make the plan
look complete, and each names what would settle it.

- **NEEDS CLARIFICATION: the definition of SC-002's "an interactive window
appears."** SC-002 states the figure and never defines the endpoint, and no
definition was located in the specification, the constitution or ADR-0001. An
undefined endpoint cannot be reproduced by a third party, so SC-013 fails on
SC-002 however carefully the milliseconds are measured. *Settled by*: a founder
reading recorded in the specification. One candidate worth costing is binding it
to SC-006's own instrument — the first presented frame at which an injected
address-field keystroke is accepted and produces a visible response — so the two
criteria cannot drift apart.
- **NEEDS CLARIFICATION: which reading of SC-014's "every URL-bearing payload"
governs.** A conforming build emits the FR-014 update check and the
blocking-list refresh in SC-014's scripted session; both bear a URL and neither
carries anything FR-007a governs, so the literal reading fails a build for doing
something FR-007a permits. *Settled by*: a founder decision landing as a
specification amendment — either restating SC-014 in terms of history-bearing
payloads, or adding the committed closed list of permitted non-history
destinations the capture's classifier reads. Suppressing the update check during
the capture is not available: it would make the capture a measurement of a build
nobody ships.
- **NEEDS CLARIFICATION: whether an outbound connectivity probe is
  permissible**,
if N3 finds that no combination of platform signals distinguishes an
intercepting network. This is a Principle VI and FR-007a question and is treated
at length below. *Settled by*: N3's measurement first, then a founder decision.
- **NEEDS CLARIFICATION: four points of the Apivo API contract.** Does the
  service
report, per state, a total it computed itself and a payable amount, rather than
only individual entries? Is the FR-027 pending-reason set a closed enumeration
with stable codes? Can a wallet hold more than one currency? Does the service
issue a withdrawal token before submission? The first decides whether the wallet
is buildable as specified at all: FR-026 forbids the client aggregating any
amount even when the arithmetic would be correct, so every total the wallet
shows must be a field in the response. *Settled by*: reading the wallet
endpoint's contract or capturing a real response, or a founder decision to add
the endpoint.
- **NEEDS CLARIFICATION: the FR-039b relay operator and contract.** No operator
  is
named, and no EU-native OHTTP relay operator was located — which is a failed
search, not a negative finding. FR-039b is explicit that where no operator is
named or no contract is in force, the signal MUST NOT be offered and no report
may be transmitted. *Settled by*: a signed contract with a named operator in a
stated jurisdiction, running EU-only ingress, bound to the three no-retention
obligations, with an effective date that also appears in the pre-consent
disclosure. Procurement, not engineering.
- **NEEDS CLARIFICATION: the site key.** FR-006 prompts "per site" and FR-008
exempts "for that site alone"; neither fixes whether the key is the origin, the
registrable domain or the host, and the three give different behaviour on a
bank's login subdomain, which the Edge Cases name as an abandonment trigger.
*Settled by*: a founder decision recorded with the per-site control's design.
- **NEEDS CLARIFICATION: which ten pages compose the SC-004 gating corpus.**
SC-004 states the count and the boundary; nothing names the pages, and SC-013's
reproducibility needs them pinned and content-addressed. *Settled by*: a founder
decision naming them, against the rule proposed in research — the cohort's daily
surfaces rather than a synthetic benchmark, archived on a recorded date,
including at least one first-party app surface.
- **NEEDS CLARIFICATION: whether the merchant catalogue is a delivered signed
surface or shell-native**, costed under FR-043 either way.
- **NEEDS CLARIFICATION: whether FR-039c's frame-contents rule is a ceiling or a
floor** — whether "MUST carry only the module name, the symbol name, and the
source file and line" requires line tables to ship, which is a measurable
download-size cost against a 20 MB budget. *Settled by*: taking the reading with
the byte cost in hand (N11).
- **NEEDS CLARIFICATION: where the root signing key lives, who holds it, and the
recorded procedure for signing a delegation.** Not derivable from the
specification, and the two-level key design is unimplementable without it.
*Settled by*: a founder decision recorded as an ADR.
- **NEEDS CLARIFICATION: spike S4 — the windowing crate and what renders the
chrome.** ADR-0001 makes both its output. *Settled by*: a measured candidate
comparison on the tier-1 runner against SC-006 and SC-004, plus the
screen-reader, dead-key and 200% passes. It is on the critical path for SC-006's
instrumentation, which has no shell-side marker to timestamp until it is taken.
- **NEEDS CLARIFICATION: whether to adopt EN 301 549 with the WCAG2ICT
  mapping**,
and the 24×24 minimum pointer target as a project rule. FR-034 states WCAG 2.1
AA and names no mapping for non-web software; WCAG 2.1's target-size criterion
is Level AAA at 44×44, so 24×24 is a founder decision for this cohort rather
than a conformance obligation.

### Two findings the plan must carry, because they change built code

#### The merged `Engine` trait is the wrong shape, and its revision precedes the first platform backend

The trait on `main` is `load(&mut self, &Request) -> Result<Page, LoadError>`
with `current() -> Option<&Page>`. Phase 0 read it against both shipping
backends at pinned versions and it does not survive that reading.

*Synchronous `load` has exactly one implementation route on either backend, and
that route breaks SC-006 by construction.* `wry::WebView::load_url` returns as
soon as navigation has begun; `webview2-com` states the UI-thread model in its
own doc comment; `wry`'s macOS delegate classes are `MainThreadOnly`. There is
no thread to move the engine to, so a synchronous `load` must block the UI
thread in a nested message pump — and `webview2-com`'s own `wait_with_pump`
documents itself as scoped to waiting *before* starting the main message loop.
Used in steady state it dispatches input and paint callbacks re-entrantly for
the length of a page load while the shell's state machine sits suspended
mid-call. SC-006 admits no trial over 16 ms at all, so this breaches it by
construction rather than by bad luck.

*`Result<Page, LoadError>` cannot express four things, each load-bearing on a
merged requirement.* Engine-initiated navigation — links, script, form posts,
redirects — produces no `load` call, so `current()` never updates where the
shell observes it, and the seam's own doc comment ("showing the request while
displaying the response is how an address bar lies") describes a failure the
trait as written makes unavoidable. The title arrives on its own event: `wry`
registers `add_DocumentTitleChanged` and `add_NavigationCompleted` as separate
handlers with no ordering between them, so one returned `Page { address, title
}` conflates two events and will routinely carry a stale or empty title. There
is no in-flight state, so SC-009's second clause — zero loading indicators
unresolved within 30 s — is not testable against this trait at all, and the
headless engine cannot even script a load that never resolves. And there is no
request-to-outcome correlation, so an outcome for a navigation the member
abandoned cannot be told from the current one.

The replacement is an asynchronous start returning a navigation id, plus a
closed event enum — Started, Committed, Redirected, Succeeded, Failed,
TitleChanged, NavigatedAway — with `LoadError` unchanged inside `Failed`.
Landing beside it, in the same workstream because the seam should be grown once
rather than five times: a host/factory type above `Engine` owning the shared
platform context, since the mechanism is per-view-by-default on **both** tiers
and ten tabs each minting their own context loses SC-004 before any product code
exists; an addressable rendering-surface handle with create, activate, suspend,
close and a data-store selector, because FR-001, FR-002, FR-007 and FR-016 each
need independently addressable contexts and the merged trait is single-surface;
a blocking policy surface rather than a per-request veto, because a veto is
implementable on tier 1 and not on tier 2 and so would make the seam
Windows-shaped; and no `Send` bound anywhere on the engine path, which is
unimplementable on either tier and cheaper to forbid now than to unwind later.

**Why this is the seam working rather than the seam failing.** ADR-0001's own
revisit triggers record that the trait's backend-swap intention "is untested
until a second real backend exists beside the headless one." Phase 0 is that
test arriving early, and its answer is that the interface needs these changes
before the backend is written. Principle III's rationale is that "a seam with
only one implementation is an assumption; a second implementation makes it a
fact" — and the assumption this seam converted into a fact is that a
synchronous, single-page, single-surface interface is not implementable over the
two engines the product ships. That conversion happened at the cheapest possible
moment: every change costs one commit moved through a headless implementation
that renders nothing, where the same change after a backend exists costs unsafe
COM and objc2 code with a second implementation to keep in step. Skipping T1
because the seam already exists is the highest-cost risk in this plan. FR-044
requires the second implementation kept working from M0, so each seam change and
its headless counterpart are one commit, and each states its cost under FR-043.

#### `LoadError::Intercepted` is not detectable on either platform, and the remedy is a founder decision

FR-015 names four navigation failures and SC-009 requires each exercised on
every supported platform. Three of the four are distinguishable from each
platform's own API on both shipping tiers. On Windows, exhaustively:
`COREWEBVIEW2_WEB_ERROR_STATUS` has nineteen values, of which
`HOST_NAME_NOT_RESOLVED` gives `Unresolvable`, the five certificate statuses
give `Certificate` and populate its `detail`, and the two credential statuses
give `AuthenticationRequired`, corroborated by
`add_BasicAuthenticationRequested`. On macOS, `NSURLErrorCannotFindHost` and
`DNSLookupFailed`; the secure-connection and client-certificate range; and
`UserAuthenticationRequired` with `webView:didReceiveAuthenticationChallenge:`.
**Nothing in either enumeration denotes interception, and none of the remaining
Windows statuses does either.** The reason is structural rather than an
omission: a captive portal *answers*, so the navigation succeeds. There is no
error to map.

Detecting it is therefore a shell-level inference, and any probe-based inference
is an outbound request. Principle VI and FR-007a govern that request: FR-007a's
list of transmissions that may carry an address the member navigated to is
closed and exhaustive, no entry accounts for a connectivity probe, and the
requirement states that adding one "is an amendment to this specification, made
in the pull request that would add the transmission; it is never an
implementation decision."

The plan therefore routes this as follows, and a backend implementer settles
none of it.

1. **N3 measures first.** Drive each tier's backend through a real captive
   portal
on that tier's reference runner and record the full signal tuple — on Windows
`(IsSuccess, WebErrorStatus, HttpStatusCode, final URI)`, on macOS `(which
delegate callback fired, NSError domain and code, final URL, whether
`didReceiveServerRedirectForProvisionalNavigation` fired)`. If some combination
distinguishes interception without any outbound request, the classification is a
shell-level rule and nothing further is owed.
2. **If nothing distinguishes it, it is a founder decision**, taken under
   Principle
VI and FR-007a, on whether an outbound probe is permissible at all. A "yes" is a
specification amendment adding a fifth entry to FR-007a's closed list, made in
the pull request that would add the probe and checked against the Permanent
Prohibition there. A "no" leaves FR-015's fourth cause producible only by the
headless engine, which keeps SC-009's fourth case testable today, and the
remaining choice — whether `Intercepted` stays in the closed enum or FR-015 is
amended — is likewise a specification change and not a refactor.
3. **In every case, no backend synthesises `Intercepted` from a platform error
code.** That is a contract clause on the seam. Guessing differently on each
platform would produce exactly the indistinguishable-cause state FR-015 exists
to forbid, and deleting the variant is not available to an implementer either,
because FR-015 names four causes and SC-009 requires four exercised.

One related finding carries forward with it, confirmed at a pinned version:
`wry` raises `PageLoadEvent::Finished` from `add_NavigationCompleted` with a
closure that discards the arguments carrying `IsSuccess` and `WebErrorStatus`,
so a Windows backend built on `wry`'s page-load handler alone reports all
nineteen statuses as success — the exact defect FR-015 names. On macOS it is
sharper: `Started` comes from `didCommitNavigation`, which a failed provisional
navigation never reaches, and neither `didFail` method is implemented, so a
failed load under `wry`'s delegate produces no event at all.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

**Gate result.** No Permanent Prohibition is breached by this design, and none
is approached by anything it builds. Principle IV and Principle X have no
violation in the plan; Principle X carries one unresolved item that gates spike
S4 rather than the plan. Principles I and II carry defects that already exist on
`main` and that this plan schedules rather than closes today, and Principle V
and Principle VI each carry a dependency outside this repository without which
the corresponding surface cannot ship. Every one of those is stated below and
recorded in *Complexity Tracking*. The gate is passed **with those entries
recorded**, not clean.

**Already enforced in code on `main`**, and planned on rather than re-proposed:
the `Engine` seam with its headless second implementation (Principle III,
`feat/engine-seam`, merged); `budgets.toml` with the three CI gates
`scripts/check-budgets.py` implements (Principle II, `feat/budget-gate`,
merged); the commit-hygiene checker and its commit-msg hook (Principle I).

### I. Sole Authorship and Signed Commits (NON-NEGOTIABLE)

**Satisfied by**: every change reaching `main` through a pull request linked to
a GitHub issue, one pull request per issue, Conventional Commits subjects
referencing the issue served, and the founder as sole author. Nothing anywhere
in this repository attributes authorship or assistance to an AI or generator
tool, and `CLAUDE.md` narrows this further: no session, run or conversation
identifier anywhere, and no `Co-Authored-By` trailer of any kind, including one
naming a human.

**Enforced by**: `.githooks/commit-msg` locally and
`.github/workflows/commit-hygiene.yml` on every pull request, both running
`scripts/check-commit-hygiene.py`, which checks author and committer identity,
AI identities inside trailer values, a fixed list of literal generator-footer
strings, Conventional Commits subjects and issue references on each commit, plus
attribution on the pull request title and body. The constitution's own Sync
Impact Report records both as landed.

**Partly closed, in Phase 1 setup.** `scripts/check-commit-hygiene.py` now
verifies each commit's signature against `.github/allowed-signers`, and CI
passes it the base branch's copy of that file so a pull request cannot authorise
its own key. Enforcement is still inert and the conclusion is unchanged: the
file lists no key, so the check reports that signing is not enabled and skips,
and only the founder can add one. `main` carries no branch protection, which is
the only thing that can gate what lands rather than what is proposed;
`docs/governance/branch-protection.md` records the settings it must have. That
range is also why the check cannot see the merge commit the forge creates: it
runs over `origin/<base>..HEAD`, which never contains it. Recorded in the
constitution's Sync Impact Report and in `CLAUDE.md` as issue #26.

The author half is settled and is no longer part of this gap. The nine merge
commits on `main` authored with the forge account's display name are compliant:
`decisions/0002` accepts `Carlos Pinto <capintobe@gmail.com>` as an author and
committer alongside `xcoder-es <capintobe@gmail.com>`, two spellings of one
identity bound by the one address Principle I names. The account is not renamed
and the display-name change this plan previously gave as the remedy is not
carried out. The `Co-Authored-By` ban is mechanical now — the check rejects
every co-authorship trailer whoever it names. The session-identifier ban is not,
and rests on review. *Complexity Tracking, row 1.*

### II. Featherweight Is Law

**Satisfied by**: all six budgets Principle II names — download size, installed
size, cold start, shell memory overhead, idle CPU and chrome input latency —
living in one file, `budgets.toml`, as eighteen entries; every change that adds
bytes or milliseconds stating its cost against that file, whether or not it
changes observable behaviour (FR-043); and a stated cost not being a
justification, so a change whose cost the pull request cannot justify is refused
on that ground by a founder decision recorded on the pull request.

**Enforced by**: `scripts/check-budgets.py`, which implements the three gates
the Success Criteria preamble defines — the budget-file gate (unconditional from
M0, comparing numbers already in the repository), the absolute gate, and the
regression gate — and by `.github/workflows/build.yml`, which runs it twice:
once for a full informational report and once blocking. An undeclared tolerance
is zero, not unbounded, and an unmeasured entry is not a pass.

It implemented a **subset** of the budget-file gate when this plan was
written, and the four conditions it did not test are the four Phase 1 setup
closed: an entry a criterion states that is **missing** from the file; an entry
recorded ratified that **names no founder decision**; a cross-check margin over
its limit; and an upward baseline reset that names no recorded founder decision.
Three of the four needed a schema the file did not carry, which the same phase
added. The gate now enforces the rule rather than a subset of it, and the wake
enumeration and the spike-exemption semantics landed with it. This paragraph
stays as the record of what was owed and of what paid it.

**Satisfied in two of the four ways this plan scheduled, and outstanding in
two.** (a) **Closed**: the file carries all eighteen entries the preamble states
— SC-002 warm and cold, SC-004 ten-tab, SC-005 window and wake-free sample, and
SC-006 tab switch and keystroke, on both platforms. (b) **Closed**: the schema
carries `figure`/`baseline` with a required `unit` of MB, ms or percent-of-core;
`founder_decision` on every ratified entry; `cross_check_margin` on both SC-004
entries; the spike-exemption and baseline-reset fields; the SC-005 wake
enumeration; and display refresh, runner label, operating-system version and
memory configuration in the runner blocks. (c) **Outstanding**: both runner
identities are empty and every baseline is `0.0`, so the regression half of the
four SC-001 entries is inert until a first measurement writes a baseline; the
blocking workflow step suppresses two budget-file clauses with
`--allow-unpinned-runners` and `--allow-unmeasured`, each named in the workflow
with what satisfies it. (d) A defect the merged gate carried, since fixed in
Phase 1 setup: `measure_download_size()` read `target/release/evreos-shell` and
`run_gates` keyed measurements on `(criterion, name)` with no platform, so one
Linux ELF built on a hosted Linux runner was compared against **both** the
`windows` and the `macos` download-size entries — and Linux is the deferred
platform, so neither entry's stated condition was met by it. Measurements are
now keyed on `(criterion, name, platform)`, the download size is read from the
platform's own published installer artefact on the host that builds it and
declared for that platform, and both download-size entries stand unmeasured
with that reason until the installer each entry's condition names exists.
**Plan**: the schema and the fourteen missing entries land before any
measurement does, the commit that first measures an entry also writes that
entry's baseline, and runner procurement starts in Phase 1 as the longest-lead
item in the whole plan. *Complexity Tracking, rows 2 and 3.*

### III. Rust Core, No Bundled Engine

**Satisfied by**: `crates/evreos-engine`, which defines the trait from what the
shell needs, names no platform, runtime or vendor and returns no handle to one;
`crates/evreos-engine-headless`, the second implementation Principle III
requires kept working from day one; and `crates/evreos-shell`, whose `navigate`
is generic over `Engine` so it cannot reach for anything a webview happens to
expose. Stable Rust, edition 2024, `rust-version = "1.85"`, no nightly on the
release path. Electron, CEF and any bundled Chromium are permanently rejected,
and FR-044 extends that to any web engine Evreos itself fetches, unpacks or
installs, at first run, on update or on demand.

**Enforced by**: the second implementation existing and building in CI on every
change; the trait's own change-control rule that a change lands in both
implementations in one commit; and `[workspace.lints.rust] unsafe_code =
"forbid"` with `#![forbid(unsafe_code)]` repeated in each crate.

**Where the plan changes this**: the trait is reshaped before the first platform
backend (see above). That is the seam working, and it is scheduled as T1–T2 so
that the conformance battery exists before the tier-2 delegate replacement,
which is the only place either shipping tier needs more than an additive
wrapper. **One carve-out is taken deliberately**: `evreos-engine-webview` opts
out of the workspace `forbid`, sets `#![deny(unsafe_op_in_unsafe_fn)]` and
requires a `// SAFETY:` note on every block, because every call a real backend
must make is `pub unsafe fn` and a backend is therefore not buildable in this
workspace today. Every other shipped crate keeps `forbid`; `evreos-probe` is the
second holder and is dev-only; a CI check asserts that no other crate lifts the
lint. Nothing in the constitution forbids `unsafe` — Principle III constrains
nightly features — so this is a repository policy decision, and the pull request
that makes it says so. *Complexity Tracking, row 4.* **One housekeeping item**:
`crates/evreos-shell/Cargo.toml` lists `evreos-engine-headless` under
`[dependencies]`, so at M0 the second implementation is on the release path. The
plan moves it off when the first real backend lands and states the byte delta
under FR-043. *[plan decision]*

### IV. Browser First, Super-App Second

**Satisfied by**: Story 1 being P1 and independently testable signed out;
FR-016a's single neutral menu entry being the whole of Apivo's presence on a
fresh profile, with no Apivo surface rendered anywhere until the member
activates it once; and every dismissal keyed to the app identity in the signed
manifest, persisting across restarts and updates. On injection, the design goes
further than the requirement: **v1 ships no page-injection mechanism at all.**
The capability catalogue contains no capability that writes into, reads from or
executes script in a web page the member visits; offer detection runs in the
shell against the current address, matched locally against a downloaded merchant
list; the offer surface is rendered in the shell's own chrome; and the member's
activation of that chrome control is what causes the FR-025 click-out and the
navigation. FR-018a is satisfied structurally rather than implemented, because a
cashback function fails the second part of its three-part exemption test — no
commercial interest — by construction, so no compliant in-page cashback path
exists to build.

**Enforced by**: the FR-016a two-profile diff test asserting that the menu
entry's accessible name, resolved style tokens and node shape are identical on a
fresh profile and on a signed-in profile with wallet state present; the
capability catalogue being a build constant, so a capability it does not
classify can never be granted; a byte-equality test between the service's
click-out URL field and the navigated address; and the absence of the mechanism
itself, which is the strongest enforcement available. Principle IV makes a
violation a release blocker.

**One conflict resolved rather than left standing**: ADR-0001's capability floor
sketches the wallet as built "using navigation gating, initialization scripts
and the cross-platform cookie API". An initialization script inserted into a
*merchant's* page is an insertion of content in FR-018a's ordinary sense, made
in a commercial interest, and so is not exempt. The ADR predates FR-018a and the
constitution supersedes, so that one mechanism is unavailable for that one
purpose. The ADR's conclusion — the wallet built once, natively in the shell —
is unaffected, as are navigation gating and the cookie API.

### V. All Money Is Server-Side

**Satisfied by**: the wallet rendering every entry the service reports in the
state the service reports, including declined and reversed, and never computing,
estimating, aggregating or omitting an amount; any device-held value typed as
stale with the time it was received and replaced outright on reconnection, never
merged or diffed; withdrawal as a two-step against a service-issued token, so
exactly-once stays wholly behind the API; and click-out URLs passed to
navigation byte for byte through a newtype constructible only from the API
response, so the client never constructs, templates or modifies an affiliate
link or any parameter of it.

**Enforced by**: the type system and CI rather than review — an `Amount` with no
arithmetic trait implementations and no public constructor other than the API
deserialiser, a distinct `Stale { amount, received_at }` with no path back to a
plain `Amount`, and a `trybuild` compile-fail test asserting that `a + b` and
`iter.sum()` do not typecheck. Making the prohibited program fail to compile is
the standard the constitution sets for anything measurable.

**Conditional on a dependency outside this repository**: FR-026's ban on client
aggregation holds even where the arithmetic would be correct, so every total the
wallet shows must be a field in the response. Whether the API supplies one is
unverified, and if it returns entries only, the wallet is unbuildable as
specified and either the API gains total fields or FR-026 is amended.
*Complexity Tracking, row 5.*

### VI. Privacy by Default, GDPR by Construction

**Satisfied by**: browsing working fully signed out, with no money surface
imposed on a member who never signs in; FR-007a's closed four-entry list of
transmissions that may carry an address, a typed search term, page content or
anything derived from them; the FR-003 field producing suggestions only from
data already on the machine, so it transmits nothing as the member types and no
suggestion service exists to be consented to; diagnostics off until the member
turns them on, carrying no identifier, reaching the service through a relay
structurally unable to read what it forwards, retained as counters rather than
reports, and hosted only in the European Union; and FR-036a as a prohibition on
what is built rather than a protection claim.

**Enforced by**: one egress crate, `evreos-net`, taking a `Purpose` argument
from a closed enum whose history-bearing variants are exactly FR-007a's four
entries, with a CI assertion over the dependency graph that no other workspace
crate depends on an HTTP or socket API. That closed enum makes an unenumerated
transmission fail to compile, which is its purpose; it does **not** discharge
FR-007a's amendment rule. FR-007a reads: "Adding an entry to the list is an
amendment to **this specification**, made in the pull request that would add the
transmission and checked against the Permanent Prohibition there; **it is never
an implementation decision**." So a new variant is not licensed by editing the
enum: it requires the same pull request to amend the specification's enumeration
and to check the change against the Permanent Prohibition on server-side
collection of browsing history. The enum is the mechanical half of that rule,
and CI asserts the two lists agree (task **T-NET-3**), so an enum edited alone
fails the build rather than shipping; the FR-007a network-capture test committed
to this repository and run in CI; and SC-014's capture, which is the only
instrument for engine traffic and therefore runs against the real engine on the
real platform on every release build, with DNS captured as well as TLS payloads.
Two concrete tier-1 defaults are closed by construction and asserted in that
capture rather than in a unit test: the `CoreWebView2Environment` is created by
Evreos with custom crash reporting enabled and the resulting dumps deleted
unread, because Microsoft documents that WebView2 process crashes otherwise send
minidumps — and a renderer minidump contains page memory, hence URLs; and
`IsReputationCheckingRequired` is set false through the raw handle on **every**
webview before its first navigation, because it defaults to true, operates
across all webviews sharing a user data folder, and re-enables for all of them
whenever a new one is created against that folder. That is a per-webview
invariant with a test, not a one-time call at startup.

**Not satisfiable yet, on one surface**: FR-039b forbids offering the diagnostic
signal at all until a relay operator is named and a written contract is in
force, and none is. The milestone that ships diagnostics must therefore be able
to ship with the whole feature dark and unofferable, with no report path in the
build — which is also the state SC-014's capture exercises on a fresh profile.
*Complexity Tracking, row 6.* The `Intercepted` probe question above is likewise
a Principle VI decision and is routed to the founder rather than to a backend.

### VII. Language and Place Are Independent Axes

**Satisfied by**: one interface catalogue per BCP-47 primary language subtag —
`de`, `el`, `en` — embedded in the binary at build time and indexed by a closed
Rust enum `Language { De, El, En }`, with `Place` as a separate type that never
appears in a catalogue key, and language and place carried as two separate
values in stored preferences, interface state and every request to an Apivo
service. FR-041 carries the same rule to the distribution page, which is not
interface text and is governed there.

**Enforced by**: the enum being the key type, which is the enforcement point — a
`LanguageIdentifier` carries language, script, region and variants, so typing
the key as one would re-admit `de-DE`, the exact failure FR-035 names; a CI
check failing on any region subtag in a catalogue filename or message key and on
any request builder serialising language and place into one field; and, for the
distribution page, a rendering of each of `de`, `el` and `en` before each
release showing no untranslated string and no fused value, published with the
release, with a failure blocking it.

### VIII. Rebrandable Shell

**Satisfied by**: one `BrandConfiguration` holding every brand name, colour,
endpoint and support address, with none hardcoded outside it — which is also why
the FR-035 catalogues need named-argument interpolation rather than a flat
table, since the brand cannot be baked into a string. Q-E13 settles that no
partner-branded distribution ships in v1 and none is promised.

**Enforced by**: a fixture brand building in CI on every change, which proves
the seam rather than asserting it, and which Principle VIII requires
independently of whether any partner build exists.

### IX. Apps Are Content, Not Releases

**Satisfied by**: app surfaces delivered server-side and updatable without a
browser release; each signed and verified before rendering or writing to the
FR-020 cache, under a trust root pinned in the shipped shell, with one signature
over the surface bytes, the app identity, the manifest digest and the version
together, and a refusal of any version below the cached copy's; a release
containing only the shell and its engine integration, with no app surface and no
cached copy of one in any installer or update; and each app's capabilities
declared in a signed, versioned manifest it cannot widen from inside, with
anything page-adjacent additionally requiring a per-app grant.

**Enforced by**: four independent mechanisms for FR-019b, **none of them the
signature** — because a pre-cached surface shipped in an installer would carry a
valid signature and so would satisfy FR-019a. Those are: a `VerifiedSurface`
type whose constructor is private to the verifier and whose only producer takes
bytes handed over by the delivery client, with the cache write path accepting
nothing else; a release-artefact scan in the idiom of `scripts/check-budgets.py`
that fails the release job on any surface-bundle magic, app manifest or
surface-cache path in the installer or installed tree; a post-install acceptance
test that launches offline and requires every app to present FR-020's stated
offline state rather than content; and the capture asserting that the first
render of any surface is preceded by that surface's delivery fetch. Effective
capabilities are the intersection of four sets — a per-app ceiling in the
shipped registry, the catalogue shipped in the release, the verified manifest's
declaration, and the member's grants — which bounds publishing-key compromise at
the cost that widening an app's ceiling needs a browser release. *Complexity
Tracking, row 7.* Money surfaces are not apps: FR-031 requires the wallet
delivered as part of the shell and usable in a build with no extension host, and
FR-016a lists the wallet and claim surfaces beside apps rather than among them.

### X. Accessibility Is Not Optional

**Satisfied by**: WCAG 2.1 AA on every shell surface, full keyboard operation,
scaling to 200% and correct German dead-key and Greek text entry as release
criteria rather than polish, plus two design rules the cohort forces: the FR-008
per-site blocking control surfaced at the moment of breakage rather than only in
settings, and every disabled control under FR-029 and FR-029a carrying
`aria-disabled` with the explanation programmatically associated rather than
being natively disabled — because a natively disabled control is removed from
the tab order and its name commonly not announced, so a build could satisfy
FR-029's letter while the member who most needs the explanation cannot reach it.

**Enforced by**: SC-008's pass on each tier driven with the platform's own
assistive technology, which is what ADR-0001 risk 7 requires — the ADR's
accessibility rationale is evidenced on Windows only and covers page content,
not the shell's own chrome; FR-036 owned as a shell test over the FR-003
combined field, the find-in-page field and every chrome text input, since the
address field is the most-used text input in the product and it is ours; and
FR-041's automated WCAG check, keyboard-only pass over the whole download path,
and three-language rendering on the published distribution page before each
release, published with the release, with a failure blocking it.

**Unresolved, and it gates spike S4 rather than the plan**: if the chrome is
drawn rather than built from platform-native widgets, AccessKit is the
mechanism, and its merged multiple-tree support is explicitly scoped to
AccessKit-provided subtrees — it does not consume a native WebView2 or WKWebView
accessibility tree, and nodes cannot reference nodes in a different tree, so a
chrome node cannot be `labelled-by` a content node. That is exactly the boundary
the specification's own Edge Case names as where this class of interface
commonly fails, against a release-blocking principle. N6 measures it before the
drawn-chrome candidate can be chosen. Separately, FR-034 states WCAG 2.1 AA and
names no mapping for non-web software, which is a founder decision (G10).
*Complexity Tracking, row 8.*

### Permanent Prohibitions

- **Ad injection** — enforced by FR-018b and satisfied structurally: no
  advertising
is placed in a web page by Evreos, by any app or under any commercial
arrangement, the cashback offer surface is rendered in the browser's own chrome,
and the capability catalogue — a build constant — contains no capability that
could place content in a page at all. The prohibition admits no consent
exception, so FR-018a's per-occasion rule does not reach it and a member tapping
"show offers here" changes nothing.
- **Silent affiliate attribution** — enforced by FR-030 and FR-033. Attribution
  is
attached only through a click-out URL the service issues for that occasion,
activated by a member action in the chrome; one installer artefact is served to
everyone with no per-partner or per-campaign build; the download URL's parameter
set is a closed allowlist of language and place, checked in the FR-041
verification that already runs before each release; and the claim code reaches
the client only through a QR the member scans or a code they type. A per-partner
installer, or a campaign identifier carried in the download URL and read back by
the installer, is the install-referrer trick Principle VI names, and is excluded
on that ground.
- **Server-side collection of browsing history** — enforced by FR-007a, whose
  list of
permitted transmissions is closed and exhaustive and which binds by the
transmission rather than by who receives it. The two halves are enforced
differently and the plan says which is which: the shell half by construction,
through the `evreos-net` chokepoint and its dependency-graph assertion; the
engine half only by SC-014's capture, because WebView2 and WKWebView open their
own sockets and no lint sees them. A transmission the system runtime makes while
serving Evreos is Evreos's transmission, which is why the two tier-1 runtime
defaults above are closed per-webview and asserted in the capture.

## Project Structure

### Documentation (this feature)

```text
specs/001-evreos-v1/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/
│   └── README.md        # Phase 1 output (/speckit-plan command)
├── checklists/
│   └── requirements.md  # /speckit-checklist output
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

Architectural decisions are recorded as ADRs in `docs/adr/`, as the Development
Workflow requires. `docs/adr/0001-rendering-engine.md` is Accepted and is cited
throughout as current. Two further ADRs are owed by this plan: root signing-key
custody (the two-level key design is unimplementable without it), and spike S4's
outcome — the windowing crate and what renders the chrome.

### Source Code (repository root)

```text
Cargo.toml                          # workspace: edition 2024, rust-version 1.85,
                                    #   unsafe_code = "forbid", release profile
budgets.toml                        # the one budget file Principle II requires
crash-reasons.toml                  # FR-039c's closed reason enumeration and
                                    #   FR-039d's OS-version granularity, beside
                                    #   budgets.toml so widening either is visible

crates/
├── evreos-engine/                  # EXISTS — the seam. Trait, Request, Page,
│                                   #   LoadError. Names no platform, runtime or
│                                   #   vendor. Gains the T1 reshape and, under a
│                                   #   `conformance` feature, the battery module
├── evreos-engine-headless/         # EXISTS — the second implementation FR-044
│                                   #   requires kept working from M0
├── evreos-shell/                   # EXISTS — the consumer that owns the
│                                   #   interface; the shell binary; the UI event
│                                   #   loop, the worker pool and the SC-005 timer
│                                   #   facility (a module, so it costs no bytes)
├── evreos-engine-webview/          # the only shipped crate holding `unsafe`;
│                                   #   #[cfg(target_os)] modules and Cargo
│                                   #   target-specific dependency tables
├── evreos-blocking/                # platform-free: corpus, adblock engine,
│                                   #   content-blocking conversion,
│                                   #   exception-closed partitioner, rule-count
│                                   #   budget, failure-taxonomy report
├── evreos-net/                     # the sole egress chokepoint; closed Purpose enum
├── evreos-i18n/                    # Language/Place types and the FR-035 catalogues
├── evreos-chrome/                  # whatever spike S4 selects
├── evreos-platform/                # default-browser registration, secure
│                                   #   credential store, update verification,
│                                   #   local rollout evaluation
├── evreos-signing/                 # preimage, Ed25519 strict verification
├── evreos-appreg/                  # app registry, roster, publishing delegation
├── evreos-capabilities/            # catalogue, ceiling, grants, intersection
├── evreos-surface/                 # verified surface hosting and the FR-020 cache
├── evreos-money/                   # Amount, Stale, wallet, claim, withdrawal, click-out
├── evreos-diag-state/              # FR-039a state machine, FR-039e cap set; no
│                                   #   network dependency at all
├── evreos-diag-transport/          # encapsulation, pinned keys, padding; the only
│                                   #   crate that can open a socket for a report
├── evreos-crash/                   # capture, shipped symbol table, symbolisation
├── evreos-probe/                   # DEV-ONLY — per-platform sampling
└── evreos-bench/                   # DEV-ONLY — trial driver, run record, discard ledger

scripts/
├── check-budgets.py                # EXISTS — the three gates, and no gate of its own
├── test_check_budgets.py           # EXISTS
├── check-commit-hygiene.py         # EXISTS
└── test_check_commit_hygiene.py    # EXISTS

tests/                              # DEV-ONLY fixtures none of which ship
├── corpus/                         # the ten-tab gating corpus, content-addressed,
│                                   #   with its loopback server
├── capture/                        # the FR-007a and SC-014 capture harness and analysis
└── signing/                        # the signing tool, so no signing code ships

.github/workflows/
├── build.yml                       # EXISTS — fmt, clippy, test, release build, budgets
└── commit-hygiene.yml              # EXISTS
```

**Structure Decision**: one Rust workspace for a desktop application, extending
the three crates already on `main` rather than replacing them. Four properties
decide the shape and each is load-bearing rather than tidy.

*One backend crate, not one per platform.* Cargo's target-specific dependency
tables already give the isolation, and two manifests would duplicate the shared
`wry`-facing wrapper. It is also the crate that holds the `unsafe` carve-out,
and exactly one manifest differing from the workspace policy is the property
that makes that carve-out reviewable.

*`evreos-blocking` names no platform, for the same reason `evreos-engine` does
not.* Its parsing half is squarely the layer ADR-0001 gives as the reason for
Rust, and keeping it platform-free is what lets the rule-count gate and the
conversion-failure taxonomy run on ordinary CI with no macOS present — which is
what turns Q-E12's site-by-site parity measurement into a short measurement
rather than an open-ended one.

*Dev-only members ship zero bytes*, which is what FR-043's per-pull-request cost
statement will say for them, and is only true if the workspace is arranged this
way from the start. It also keeps the memory sampler outside the memory budget
it measures.

*The conformance battery and the SC-005 timer facility are modules, not crates*
— the battery under a `conformance` feature inside `evreos-engine`, so that both
implementations are held to one meaning rather than merely compiling against one
signature, and the timer facility inside the shell so that its byte cost stays
nil while `build.rs` reads `budgets.toml`'s wake enumeration and makes an
unenumerated wake a compile error.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| **Principle I: commit signing is unenforced.** A signature check landed in Phase 1 setup and verifies against the base branch's `.github/allowed-signers`, but no key is listed so it reports signing not enabled and skips; `main` carries no branch protection, and the hygiene check runs over `origin/<base>..HEAD`, which never contains the merge commit being created. | Pre-existing repository state, recorded in the constitution's own Sync Impact Report and in `CLAUDE.md` as issue #26. Closing it requires a key only the founder holds and branch protection this plan does not configure. | Rewriting those merge commits was considered when the author half was still open and is now moot: `decisions/0002` makes those commits compliant, so there is no defect left to rewrite history for. Widening the check's range to include the merge commit is not available to a check that runs before the merge exists. |
| **Principle II, resolved for the file and the gate: `budgets.toml` carried four of eighteen entries and lacked six fields the preamble's budget-file gate is defined to fail on** — unit, `founder_decision`, `cross_check_margin`, spike exemption, the SC-005 wake enumeration, and display refresh — while both runner identities were empty and every baseline was `0.0`. The file now carries all eighteen entries and every one of those fields, and the gate tests every clause the preamble states. The identities and the baselines stay outstanding: both wait on hardware, which no change in this repository can supply. | The budget-file gate is unconditional from M0 and is specifically what bounds the advisory period on the two measuring gates, so an incomplete file is a gate that cannot fail. The schema and the fourteen missing entries therefore land **before** any measurement, and the commit that first measures an entry also writes its baseline. | Adding entries as each measurement lands was rejected: it leaves the gate unable to fail for exactly as long as the measurements are missing, which is the period the gate exists to cover. Encoding milliseconds in a `figure_mb` field was rejected: it makes the tolerance arithmetic silently wrong across units. |
| **Principle II, resolved: the download-size measurement compared one Linux ELF against both platform entries.** `measure_download_size()` read `target/release/evreos-shell` and `run_gates` keyed measurements on `(criterion, name)` with no platform. Linux is the deferred platform, and neither entry's stated condition — "the installer artefact CI publishes" — was met by it. | Recorded here when the plan was written as a confirmed defect in a merged gate, and fixed in Phase 1 setup rather than held for the first installer: measurements are keyed on `(criterion, name, platform)`, the download size is read from the platform's own published installer artefact on the host that builds it and declared for that platform, and each download-size entry stands unmeasured with that reason until its installer exists. The row stays as the record that the violation existed and of what closed it. | Holding the fix for the first installer, as this row first proposed, was rejected: the key was a gate defect independent of any artefact — a gate that compares one platform's number against another platform's entry is wrong whatever it measures — and the fix needs no installer to land. Deleting the measurement until an installer exists was rejected: an unmeasured entry blocks the budget-file gate, and a deferral that is stated (`--allow-unmeasured`, with what satisfies it named in the workflow) is auditable where a silently removed measurement is not. |
| **Principle III's neighbourhood: `evreos-engine-webview` lifts the workspace `unsafe_code = "forbid"`.** Every call a real backend must make — `Navigate`, `add_NavigationCompleted`, `add_ServerCertificateErrorDetected`, `setNavigationDelegate`, `addContentRuleList` — is `pub unsafe fn`, so a backend is not buildable in this workspace today. | The exception must be narrow, named and reviewable, and exactly one manifest differing is what makes it so. It lands with the first FFI line rather than after, because retrofitting means arguing about an `allow` already in the tree, which typically ends as a blanket allow on whichever crate hit it first. Nothing in the constitution forbids `unsafe`; Principle III constrains nightly features. | Relaxing the lint workspace-wide to `deny` was rejected: it spreads the exception to crates that will never need it. Splitting the unsafe into a `-sys`-style crate below the backend was rejected: the unsafe *is* the backend, so the split produces a boundary with nothing on one side. Driving the platform APIs through an out-of-process helper was rejected on SC-001 and SC-004, and it invents an IPC surface the seam exists to avoid. |
| **Principle V: the wallet may be unbuildable as specified.** FR-026 forbids the client aggregating any amount even where the arithmetic would be correct, so every total the wallet shows must be a field the service sent — and whether the Apivo API supplies per-state totals and a payable amount is unverified. | Recorded rather than resolved because Principle V places the answer outside this repository. If the API returns entries only, the resolution is either an API change or an amendment to FR-026, and both are decisions rather than implementation. | Computing the totals in the client "because the arithmetic is correct" was rejected by FR-026's own words and by Principle V's rationale: a client that computes money will eventually disagree with the ledger, and the member will believe the client. Hiding a state whose total cannot be shown was rejected by FR-026, which names dropping declined and reversed as the failure. |
| **Principle VI: the diagnostic signal cannot be offered at all, because no FR-039b relay operator is named.** The two operators located are US-incorporated, FR-039f requires EU infrastructure, and no EU-native OHTTP relay operator was located — a failed search, not a negative finding. | FR-039b states the consequence itself: where no operator is named or no contract is in force, the signal MUST NOT be offered and no report may be transmitted. The milestone that ships diagnostics must therefore be able to ship with the feature dark and with no report path in the build. Procurement and the DPIA start in Phase 1, in parallel with code. | Operating the relay ourselves through a second legal entity the founder controls was rejected, and rejected in writing because it is the shortcut that would otherwise be taken: FR-039b's point is a party that does not answer to the receiving service, and a controlled entity is in substance the "different party that nevertheless sees both the source address and the payload" the requirement names. Plain TLS to an EU endpoint under a no-log promise is the same arrangement FR-039b exists to prevent. |
| **Principle IX: a per-app capability ceiling in the shipped registry means widening an app's capabilities requires a browser release.** Nothing in FR-017 or FR-018 requires the ceiling; it is added by this design. | Without it, compromise of the online publishing key yields a manifest declaring every catalogued capability, and every non-page-adjacent one is then held with no member in the loop. With it, the blast radius is bounded to what the app already had, and FR-017's "MUST NOT be able to widen them from inside" holds structurally at the shell boundary rather than by the publisher's restraint. | Letting the manifest alone bound capabilities was rejected on that compromise argument. Refusing the whole app when its manifest names an unknown capability was rejected: it turns every capability addition into a hard break for members on older shells, where the intersection simply does not hold the unknown capability. The accepted cost is defensible because Principle IX keeps app *content* off the release cycle, and what an app may *do* is not content. |
| **Principle X: chrome accessibility is unsolved for the drawn-chrome candidate**, against a principle whose violations are release blockers. AccessKit's merged multiple-tree support does not consume a native WebView2 or WKWebView tree and nodes cannot reference nodes in a different tree, so a chrome node cannot be `labelled-by` a content node — the exact boundary the Edge Cases name. | It is carried in the plan's risk register rather than folded into "build the UI", and it gates spike S4 rather than the plan: N6 builds a minimal AccessKit chrome with one embedded webview on each runner and drives it with Narrator, NVDA and VoiceOver before the drawn-chrome candidate may be chosen. | Assuming the operating system composes the trees because the webview is a child window was rejected as unverified — plausible on Windows via the HWND-rooted UIA hierarchy, but nothing located establishes it for an AccessKit host. An in-toolkit screen reader was rejected because SC-008 and ADR-0001 risk 7 both require driving each surface with the platform's own assistive technology. |
| **FR-015 and SC-009: `Intercepted` is not producible by either platform backend.** No value in `COREWEBVIEW2_WEB_ERROR_STATUS` and no `NSURLError` code denotes interception, because a captive portal answers and the navigation succeeds. | SC-009's fourth case stays testable today because the headless engine scripts it, which is one more thing Principle III's second implementation buys. The variant's *production* is scoped rather than the variant deleted, and the remedy is routed to N3 and then to a founder decision under Principle VI and FR-007a, never to a backend. | A backend synthesising `Intercepted` from a platform error code was rejected: nothing in either API supports the synthesis, so the variant would be guesswork differing per platform — the indistinguishable-cause state FR-015 exists to forbid. Deleting the variant was rejected as premature and as a specification amendment rather than a refactor: FR-015 names four causes and SC-009 requires four exercised. |
| **SC-005 is ratified and tighten-only, and its scope includes engine processes whose idle floor has never been measured.** SC-005 bounds processor use over the same process set SC-004 counts — the runtime's browser, renderer, network and GPU processes — whose idle timers Evreos does not author, cannot enumerate and cannot remove. | Recorded here because the consequence is not a code change: if the engine's own floor exceeds 5 ms of processor time in some wake-free 1-second sample on an 8th-generation i3, SC-005 is unmeetable and the remedy is a specification amendment recording the founder decision, the measured evidence, and what discipline replaces the budget removed. N1 measures it in Phase 1, before the harness architecture is fixed. | Assuming the engine idles at zero was rejected as the assertion the constitution's measurement discipline exists to prevent; ADR-0001 records no idle-CPU floor either way. Scoping SC-005 to the shell process alone was rejected because SC-005 names SC-004's process set explicitly, precisely so work cannot be hidden by relocating it into a runtime process. |
| **FR-006 cannot be met in full on tier 2 at any floor this product could declare.** WebKit's public `WKUIDelegate.h` declares media-capture permission as available at the floor, declares geolocation permission far above it, and declares no notification-permission delegate at all, while FR-006 requires prompting per site for all four. | Stated as a finding rather than discovered during implementation. The plan does not build one four-permission surface from the tier-1 shape: N9 measures what a page actually observes at the floor, then each capability is routed per tier either to a prompt or to FR-037's hand-off, and FR-041 carries the resulting statement to the distribution page. | Raising the tier-2 floor was rejected as unavailable: no floor this product could declare reaches the version the geolocation delegate names. Shipping a prompt that grants something the engine will not deliver was rejected as exactly the failure the member must diagnose that FR-037 forbids, and FR-041 forbids asserting either presence or absence until it is measured. |
| **FR-002's suspension has no verified mechanism on tier 2 at its floor.** Tier 1 has one — `TrySuspend`, documented as pausing script timers and animations and letting the operating system reclaim renderer memory, gated on `IsVisible` false. Tier 2 has none: `WKPreferences.inactiveSchedulingPolicy` is above the floor and unexposed by `wry`, whose own background-throttling option is documented Unsupported on Windows and supported only above the floor on macOS. | FR-002 requires a *stated* policy, and a policy stated from an API that does not exist at the floor would be a claim about behaviour nobody measured. N5 measures what actually throttles a hidden background tab on each tier; if no lever suffices on a tier, FR-002's suspension has no mechanism there and that is a finding for the plan rather than a bug found later. | Discarding and reloading background tabs was rejected: FR-002 requires reversal without losing the page state visible to the member, and a reload loses form state and scroll position — which on a bank or government form is the failure that ends the install. Suspending on a timer was rejected because visibility, not elapsed time, is the gate the one verified lever requires. |