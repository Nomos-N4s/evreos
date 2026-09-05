# Quickstart: Evreos v1 — the runnable validation guide

**Feature**: `001-evreos-v1` · **Phase**: 1 (design) · **Source of truth**:
`specs/001-evreos-v1/spec.md`, `.specify/memory/constitution.md`,
`docs/adr/0001-rendering-engine.md`

This document is how the feature is proved, not how it is built. It carries no
implementation bodies. Part A is what runs today against code that exists; Part
B is every scenario the plan owes, each with its prerequisites, its command and
the outcome that counts as a pass; Part C is the platform matrix, which is the
honest answer to "can I check this here?".

## How to read this document

- Every scenario states **Prerequisite → Command → Pass**. A scenario with no
pass condition a machine can evaluate is not a scenario; it is a wish, and it is
recorded in *Gaps* instead.
- Where a requirement forces something, the requirement is named at it. Where
this document proposes a mechanism the specification does not require, it is
marked **[design]**. Where the design needs something no requirement supplies,
it is marked **[gap]** and repeated in *Gaps* at the end. Nothing here is
presented as required unless a requirement is named for it.
- Platform tags: **[any]** runs on any developer machine including Linux;
**[tier 1]** needs a Windows 11 machine; **[tier 2]** needs a macOS 13+ machine;
**[tier 1 runner]** / **[tier 2 runner]** need *the pinned benchmark runner for
that tier* and no other machine, because the Success Criteria preamble reports
every measured figure against the runner for its platform and against no other;
**[CI]** runs in GitHub Actions.
- A command marked *(does not exist yet)* is the shape the plan owes, named so
that the change which lands it has a target. It is not claimed to run today.

---

# Part A — What runs today

## A0. Where the code is, and what it needs

`main` carries all of it. Both branches that produced this code are merged into
`main` — `feat/engine-seam` (pull request #34) and `feat/budget-gate` (pull
request #37) — so alongside the specification, the constitution and ADR-0001,
`main` carries the `Engine` trait, the headless second implementation, the
shell, the FR-015 navigation-failure tests, `budgets.toml`,
`scripts/check-budgets.py`, that script's own tests, and
`.github/workflows/build.yml`. Confirmed by `git ls-tree origin/main` and `git
log origin/main`.

There is therefore no branch to check out, and no branch to check out *to*: the
two feature branches still exist but carry nothing `main` does not. Everything
in Part A runs from a checkout of `main`:

```
git checkout main
```

**Toolchain.** Stable Rust, edition 2024, `rust-version = "1.85"` declared in
the workspace manifest — Principle III and FR-044 forbid nightly features on the
release path, and the floor is checked in so a toolchain that cannot build the
release path fails at resolve time. Python 3.11 or later for the budget gate
(`tomllib`).

**Network.** None required. `Cargo.lock` resolves to three packages, all local
path dependencies, and the workspace pulls no third-party crate at all. That is
a property worth keeping deliberately: the first change that adds a real
dependency tree is also the first change that meets SC-001's 20 MB download
entry with something other than our own code, and A6 explains why no
download-size figure is measured against it today.

## A1. Build

**Platform**: [any] · **Prerequisite**: A0.

```
cargo build --release
```

**Pass**: builds clean. `cargo fmt --all --check` and `cargo clippy
--all-targets --all-features -- -D warnings` both pass; CI runs both before it
runs anything else, so a failure here fails the build.

## A2. The navigation-failure tests

**Platform**: [any] · **Requirement**: FR-015, SC-009, FR-044, Principle III.

```
cargo test --all
```

**Pass**: eight tests in `crates/evreos-shell/tests/navigation_failures.rs`,
three in the engine crate's own module and one inside the shell binary — all
passing, with no other target reporting tests.

```
running 8 tests
test a_failed_load_is_never_a_successful_empty_page ... ok
test a_failure_does_not_replace_the_page_the_member_was_on ... ok
test an_unscripted_address_fails_rather_than_silently_succeeding ... ok
test current_reflects_a_commit_before_it_is_drained ... ok
test each_of_the_four_causes_is_distinguishable ... ok
test every_load_the_shell_asks_for_is_observable ... ok
test the_generic_entry_points_carry_no_send_bound ... ok
test the_shell_sees_the_address_that_loaded_not_the_one_requested ... ok
```

What those eight actually establish, stated narrowly because SC-009 asks for
more than they give:

- each of FR-015's four causes — unresolvable, certificate, intercepted,
authentication-required — is a distinct value producing a distinct message that
names the address;
- a failed load is not a successful empty page, which FR-015 names verbatim as a
defect;
- a failed load does not replace the page the member was on;
- an address nobody scripted fails visibly rather than succeeding emptily;
- a loaded page carries an address the shell reads off the page rather than
echoing what was typed. **What that test does not establish is redirects.**
`the_shell_sees_the_address_that_loaded_not_the_one_requested` scripts
`https://site.invalid/` and asserts that the returned page's address is
`https://site.invalid/`: requested and loaded are the same string, so no
redirect is exercised and nothing about redirects is proved. The property is
real and is stated as a requirement of the contract — `Page::address`'s doc
comment in `crates/evreos-engine/src/lib.rs` reads "the address that actually
loaded, which may differ from the requested one after a redirect" — but the test
does not yet prove it. A redirect case is a test worth adding, and adding it
needs a headless engine that can script a response whose address differs from
the request, which the headless engine cannot script today: it commits at the
requested address, and the scripted event sequences that can express a redirect
land with the contract's sequence support;
- every address the *engine* was asked to load is observable to a test, through
`HeadlessEngine::loads()`. That is a record of what the shell asked the engine
for, and not a record of what left the machine: it observes an engine that opens
no socket. FR-007a's boundary is outbound traffic, and B10 states the only
instrument for it — a capture on real hardware, because WebView2 and WKWebView
open their own sockets and no in-process recorder sees them. This test is a
precondition for an assertion about that boundary, not the assertion;
- a committed page is visible through `current()` before its events are drained
— the contract's emission-time clause, pinned so the update cannot silently
move to drain time;
- the engine-generic entry points carry no `Send` bound, proved by an engine
holding an `Rc` driven through them, which is the consumer-side half of the
guard the engine crate's own test module carries for the trait itself.

What they do **not** establish: anything about a real platform, and nothing
about the shell's own code. The file sits under `crates/evreos-shell/tests/`,
but it imports `evreos_engine` and `evreos_engine_headless` and nothing else, so
it drives the headless engine directly. What it exercises is the seam's contract
— `LoadError`'s closed set of four causes and its `Display` — and the headless
implementation of that contract, on a machine with no system webview. The
shell's own handling is `navigate()` in `crates/evreos-shell/src/main.rs`; it
lives in a binary crate, so an integration test cannot import it, and what
exercises it today is the unit test inside that binary plus A3's `cargo run`.
Giving the shell's half integration tests means moving its machinery into a
library target, and that is a change the plan owes. SC-009 separately requires the four causes
exercised "on every supported platform"; that exercise is B4 and needs a real
backend on a real machine of each tier.

## A3. Run the shell against the headless engine

**Platform**: [any] · **Requirement**: FR-044, Principle III.

```
cargo run -p evreos-shell
```

**Pass**: exactly this, on any machine including one with no system webview:

```
engine: headless
Example — https://example.invalid/
could not load: the identity of https://expired.invalid/ could not be verified: the certificate expired
could not load: https://nowhere.invalid/ could not be found
```

This is the whole of what M0 claims: the shell drives rendering through
`evreos_engine::Engine` and never through a concrete backend, and the second
implementation makes that a fact rather than an assertion (Principle III;
FR-044, which requires the headless implementation kept working from M0).

## A4. The budget gate's own tests

**Platform**: [any] · **Requirement**: Principle II, FR-043.

```
python3 scripts/test_check_budgets.py
```

**Pass**: `194/194 passed`, exit 0. The gate is CI's authority to fail a build,
so its own behaviour is tested rather than assumed. Beyond each gate clause, the
cases prove the script's closed list against an independent statement of the
preamble's eighteen, the schema of every sub-table, SC-005's wake arithmetic at
the count a closed hour holds, the release refusal, the measurement key — that
one host's artefact satisfies its own platform's entry and no other — and the
committed budget file itself, which fails only on its two unpinned runners.

## A5. The budget gate, and the deliberate failure

**Platform**: [any] · **Requirement**: Principle II, FR-043, Success Criteria
preamble.

Run it in the two modes CI runs it in. The full report first:

```
python3 scripts/check-budgets.py
```

It needs no build ahead of it: the gate reads nothing off the release binary
(A6).

**Pass condition today is a failure, and the failure is the point.** Expect exit
1 and these lines, as a Linux host prints them. The `FAIL` lines go to stderr
and the rest to stdout, so the order you see depends on how you capture them;
this is the order a terminal shows:

```
  FAIL     [budget file] runner tier1 (8th-generation Intel i3/i5 laptop, 8 GB) is not pinned: no durable identity, no runner_label, display_refresh not recorded; until it is procured and pinned no hardware-dependent figure is reproducible and no workflow job can resolve it
  FAIL     [budget file] runner tier2 (MacBook Pro (2017), 8 GB) is not pinned: no durable identity, no runner_label, display_refresh not recorded; until it is procured and pinned no hardware-dependent figure is reproducible and no workflow job can resolve it
  unmeasured on this machine: 18 entries
    - SC-001 download size (windows)  (a windows figure is measured on a windows host; this host builds no tier's artefact)
    - SC-001 installed footprint (windows)  (a windows figure is measured on a windows host; this host builds no tier's artefact)
    - SC-001 download size (macos)  (a macos figure is measured on a macos host; this host builds no tier's artefact)
    - SC-001 installed footprint (macos)  (a macos figure is measured on a macos host; this host builds no tier's artefact)
    - SC-002 warm start (windows)  (no pinned runner for this tier)
    - SC-002 cold start (windows)  (no pinned runner for this tier)
    - SC-002 warm start (macos)  (no pinned runner for this tier)
    - SC-002 cold start (macos)  (no pinned runner for this tier)
    - SC-004 ten-tab memory (windows)  (no pinned runner for this tier)
    - SC-004 ten-tab memory (macos)  (no pinned runner for this tier)
    - SC-005 60-minute window (windows)  (no pinned runner for this tier)
    - SC-005 wake-free 1-second sample (windows)  (no pinned runner for this tier)
    - SC-005 60-minute window (macos)  (no pinned runner for this tier)
    - SC-005 wake-free 1-second sample (macos)  (no pinned runner for this tier)
    - SC-006 tab switch (windows)  (no pinned runner for this tier)
    - SC-006 address-field keystroke (windows)  (no pinned runner for this tier)
    - SC-006 tab switch (macos)  (no pinned runner for this tier)
    - SC-006 address-field keystroke (macos)  (no pinned runner for this tier)
  FAIL     [budget file] SC-001 download size (windows): a windows figure is measured on a windows host; this host builds no tier's artefact; an unmeasured entry is not a pass
  FAIL     [budget file] SC-001 installed footprint (windows): a windows figure is measured on a windows host; this host builds no tier's artefact; an unmeasured entry is not a pass
  FAIL     [budget file] SC-001 download size (macos): a macos figure is measured on a macos host; this host builds no tier's artefact; an unmeasured entry is not a pass
  FAIL     [budget file] SC-001 installed footprint (macos): a macos figure is measured on a macos host; this host builds no tier's artefact; an unmeasured entry is not a pass
  measured: nothing on this linux host; this host builds no tier's installer artefact

Budget gates FAILED: budget file
```

Six `FAIL` lines, not two. Read them as three separate statements.

1. **The two runner failures are correct and are meant to be there.** Q-E9a
settled the rule and the models; procurement is what remains, and the
Assumptions entry on reference hardware records that pinning the two machines
"is what remains before a hardware-dependent absolute gate blocks the build".
Pinned means all three recorded — a durable identity, the `runner_label` a
workflow's `runs-on` resolves, and the display refresh SC-006's condition
needs — and the budget-file gate fails on a runner lacking any of them
precisely so that the advisory period on the absolute gates is bounded by a
gate rather than by good intentions. These two lines go away when the machines
are procured and pinned in `budgets.toml`, and not before.
2. **"unmeasured" is not "passed," and the gate blocks on it rather than only
saying so.** Eighteen entries are listed, each with the reason it is unmeasured
here. Fourteen are hardware-dependent entries on a tier with no pinned runner:
there is no machine to measure them on, the runner condition above already
reports that, and they do not block. SC-001's four are not hardware-dependent
and have no measurement, so they block, and each line says why: an SC-001
figure is measured on a host of the entry's platform — the download size where
the artefact is built, the installed footprint where it is installed — and a
Linux host is neither. On a Windows or macOS developer machine the two entries
of that host's own platform read `no measurement was produced` instead, the
download size because no installer artefact exists where that platform's
packaging build publishes it and the installed footprint because no harness
produces it; the other platform's two read as above. All four keep blocking
until an installer exists per platform.
3. **Nothing is measured, and the last line says so and why.** The download
size is "the installer artefact CI publishes", read from
`target/packaging/windows/*.msi` or `target/packaging/macos/*.pkg` on the host
that builds it and declared for that platform; this host builds neither, and
the release binary is not read. On a tier's host with a built installer the
line reads `measured: download size (<platform>) N MB`, and the figure is
compared against that platform's entry alone (A6).

Then the blocking mode, which is what CI's merge-blocking step runs. It carries
two flags, not one:

```
python3 scripts/check-budgets.py --allow-unpinned-runners --allow-unmeasured
```

**Pass**: exit 0 and `Budget gates passed.`, with the two runner failures
demoted to `advisory` and the four SC-001 entries annotated `deferred by
--allow-unmeasured`. Run with `--allow-unpinned-runners` alone the script still
exits 1: that flag suppresses only the runner condition, and the four unmeasured
entries fail regardless.

Both flags are in `.github/workflows/build.yml` today, and removing each is a
release prerequisite rather than a preference, with a named thing that satisfies
it. `--allow-unpinned-runners` is the only thing suppressing the budget-file
gate's runner condition, and it is satisfied by procuring and pinning the two
machines Q-E9a names. `--allow-unmeasured` is the only thing suppressing the
budget-file gate's unmeasured condition, and it is satisfied by building the
harness that produces each figure — today that is both SC-001 quantities on
both platforms: the download size, which is the installer artefact CI publishes
for that platform and is measured on the host that builds it, and the installed
footprint, which is the disk delta after first run completes; each needs an
installer that does not exist yet.

The workflow runs the full report at `continue-on-error: true` and the blocking
mode as a gate, so both outputs appear on every pull request and only the second
can fail the build.

## A6. What today's gate output does not yet mean

Three things are true of the gate as it stands, each verified by running it.
They are recorded here because a validation guide that presents a green gate as
a green product is the failure Principle II exists to prevent.

- **No download-size figure exists, and the gate prints none.** The merged gate
read `target/release/evreos-shell` — a Linux ELF built on a hosted Linux runner
— and keyed the figure on `(criterion, name)` with no platform, so one number
was compared against both the `windows` and the `macos` download-size entries,
and neither entry's stated condition — "the installer artefact CI publishes" —
was met by it. Measurements are now keyed on `(criterion, name, platform)`, an
entry's whole identity; the download size is read from the platform's own
published installer artefact on the host that builds it and declared for that
platform, so an entry of another platform is reported unmeasured with that
reason rather than compared against it, and a Linux host, being no tier's,
measures nothing. Neither installer exists, so both download-size entries stand
unmeasured until the installer each entry's condition names is built, and a
green blocking step says nothing about SC-001's download size on any host.
- **The regression half is inert.** Every entry records `baseline = 0.0`, and
the comparison is guarded on `baseline > 0`, so no entry currently has a
regression gate that can fire. The commit that first measures an entry
honestly must also write its baseline, or the gate stays inert indefinitely.
- **Every hardware-dependent absolute gate is advisory.** Fourteen of the
eighteen entries are measured on a tier's pinned runner, and neither runner is
pinned, so a breach of their stated figure cannot yet fail a build: a figure
measured on an unnamed machine is not reproducible under SC-013. The
budget-file gate's runner condition is what bounds that period, and
`--allow-unpinned-runners` is the only thing holding it open.

The schema the preamble's budget-file gate is defined over — the closed list of
eighteen, a unit on every figure, a founder decision on every ratified entry,
SC-004's cross-check margin, the spike exemption and the release job's refusal
of a build carrying one, SC-005's wake enumeration, and the display refresh in
each runner block — is in place ahead of the first real measurement, which is
the ordering the gate needs: an incomplete budget file is a gate that cannot
fail, and the budget-file gate is the thing bounding the advisory period on
every other gate. What remains is measurement, and each point above names what
produces it.

## A7. What cannot be validated on a machine with no system webview

Everything that needs a page. On a Linux developer machine, or on any machine
without WebView2 or WKWebView, the headless engine renders nothing and answers
from a script, so it can prove that the seam holds and that the shell's own
handling is correct. It cannot prove anything about the web.

Specifically, none of the following can be checked here, and no result obtained
here may be reported for them: FR-001 tabs and session restore, FR-002
background suspension, FR-004 downloads, FR-005 find-in-page and zoom, FR-006
permission prompts, FR-007 private windows, FR-008 blocking and its per-site
control, FR-009 document viewing, FR-012 import, FR-013 default-browser
registration, FR-015's four causes *as the platform reports them*, every app and
money surface, and every one of SC-002, SC-004, SC-005, SC-006, SC-007, SC-008,
SC-009, SC-009a and SC-014.

Two further limits that catch people out:

- A Windows 11 or macOS 13 **developer** machine is enough to *exercise* a
scenario and not enough to *report a figure*. The Success Criteria preamble
binds every measured figure to that tier's pinned runner and to no other
machine, so a fast laptop producing a green number produces nothing that may be
recorded, published under SC-013, or used to reset a baseline.
- Neither shipping tier's backend exists yet. Phase 0 research established that
the merged `Engine` trait's synchronous `load` could not be implemented over
either shipping backend without a nested message loop SC-006 forbids, that it
could not represent navigation the shell did not initiate, and that it could
not express an in-flight load SC-009 requires to be testable — which is why the
trait changed to the event contract before the first backend was written. What
it still lacks is the construction seam where the shared platform context
SC-004 depends on can live, and scenarios in Part B that name a backend are
gated on the remaining owed seam changes landing first.

---

# Part B — Scenarios as the plan lands

Each scenario states its requirement, its prerequisites, the command, the pass
condition, and the platform it needs. Ordering within Part B follows the
dependencies, not the requirement numbers.

## B1. User Story 1 — browse privately on an ordinary machine

**Requirement**: Story 1's independent test, verbatim from the spec — "Install
on the reference hardware, complete a full session — search, ten tabs, bookmark,
download, print, find-in-page, close and reopen — entirely signed out, and
confirm every ratified budget in Success Criteria holds, recording the measured
cold-start and warm-start times against the two provisional figures SC-002
states."

**Platform**: [tier 1 runner], then [tier 2 runner]. The reference hardware is
named by the test itself, so a session on any other machine is a rehearsal
rather than the test.

**Prerequisites**: the tier's pinned runner procured and its identity in
`budgets.toml`; the system-webview backend for that tier; the chrome (ADR-0001
spike S4's output) — a session with tabs, an address field and a download list
is a session with chrome; FR-008 blocking active on first launch without
configuration; the budget schema of A6.

**Command** *(does not exist yet)*:

```
scripts/session-acceptance.py --tier 1 --signed-out --record runs/<commit>/story1/
```

**Pass**, and each clause is separately falsifiable:

- the full session completes signed out, and nothing at any point asks for an
account (FR-022, and Story 2's fifth acceptance scenario, which is the same
property asserted from the other side);
- tracker blocking is active from first launch with no configuration (FR-008),
and its per-site control is reachable *at the moment of failure* rather than
only from settings — the spec's own edge case makes discoverability at the point
of breakage the test, because this cohort abandons rather than hunts;
- every **ratified** entry holds: SC-001's four, SC-004 on tier 1, SC-005,
  SC-006
(Q-E9);
- cold start and warm start are **recorded against**, not gated on, SC-002's two
provisional figures — all four SC-002 entries are provisional pending the
cold-start spike (B12), and the spec says the shell architecture is expected to
be shaped by what that spike finds;
- the session restores on close and reopen (FR-001).

Story 1's six acceptance scenarios map to B1 (scenarios 1, 3), B7 (2), B4 (4),
and to two of its own:

- **Per-site control persists** (scenario 5, FR-008): turn blocking off for one
site, restart, and find it still off for that site and on everywhere else.
- **Import** (scenario 6, FR-012): with an existing Chrome, Firefox or Edge
profile on the machine, import and find bookmarks and history present. FR-007a
expressly permits this as local computation; what it forbids is any of it
leaving the machine, which B10 checks.

## B2. User Story 2 — claim and follow cashback

**Requirement**: Story 2's independent test — "With a fixture merchant network,
complete the path from scanning a claim code to seeing a pending entry in the
wallet, then follow a withdrawal request to a terminal state."

**Platform**: [tier 1] first, then [tier 2]. Not runner-bound: this is a
behavioural test, not a figure.

**Prerequisites**: a fixture merchant network and a fixture Apivo service; the
shell-native wallet and claim surfaces (FR-031 requires the wallet delivered as
part of the shell); protocol-handler registration at install time so a scanned
code opens the claim flow (FR-032); sign-in with the credential in the OS secure
store (FR-023).

**Command** *(does not exist yet)*:

```
scripts/money-acceptance.py --fixture-service <url> --tier 1
```

**Pass**:

- a scanned claim code opens the claim flow directly after installation, with
  the
member navigating nowhere (FR-032, Story 2 scenario 1);
- opening an offer routes through a click-out URL the service issued for that
occasion, the member is told plainly that tracking is taking place, and the
navigated address is byte-identical to the URL the service returned — the client
constructs, templates and modifies nothing (FR-025, Principle V);
- the wallet shows pending, confirmed, declined and reversed exactly as the
fixture ledger reports them, with the payable amount where the service reports
one, and computes, estimates, aggregates and omits nothing (FR-026);
- a pending amount carries a plain-language explanation of why it is pending
(FR-027);
- a withdrawal request is recorded and followable to a terminal state (FR-028),
and a submission whose response is lost produces an explicit unknown state
rather than a retry or an inferred outcome (FR-026a);
- with the fixture service unreachable, the wallet presents a stale state
carrying the time it was last received, never a current balance, and on
reconnection the service's value replaces the cached one outright with no
reconcile, merge or diff (FR-026a, and the spec's edge case on the ledger and
the client disagreeing);
- a claim code that is already redeemed, expired, or belongs to another member
produces three distinct plain-language outcomes and no generic error (edge
cases);
- no offer alters, overlays or annotates a merchant's page at any point
(FR-018a, FR-018b) — the offer surface is in the browser's own chrome, and
interaction with page content authorises nothing.

**Blocked**: FR-029 claim-code redemption ships present and **disabled** until
the existing service is confirmed to hold campaign records and accept a
redemption (Q-E11a). So the redemption half of this scenario runs against the
fixture service only, and **SC-010 is not measurable and must not be scheduled
as an acceptance gate** until Q-E11a resolves — the spec says so directly.

The disabled state itself is testable now, and its test is the one FR-029a
writes: on a build with no backing service, find the control present and
reachable, read the stated explanation, and confirm that activating it produces
no outbound request and no error state. The "no outbound request" clause is
checked in the same capture B10 runs, not by inspection.

## B3. User Story 3 — apps updated without a browser release

**Requirement**: Story 3's independent test — "Publish an app change server-side
and confirm members see it without a browser update, while the app's declared
capabilities remain unchanged."

**Platform**: [tier 1], then [tier 2].

**Prerequisites**: the signature verifier and its pinned trust root; the shipped
app registry and capability catalogue; the surface cache; a fixture publisher
that can sign a surface. The verification core is exercisable against the
headless engine with no webview at all, which is why it can land early.

**Command** *(does not exist yet)*:

```
scripts/app-acceptance.py --publisher-fixture <dir> --tier 1
```

**Pass**:

- a published surface change reaches the member on next open, with no browser
release (FR-019, Story 3 scenario 3);
- the app's declared capabilities are unchanged by that update, and an app
attempting a capability beyond its manifest is refused — an app can never widen
its capabilities from inside (FR-017, scenario 4);
- a page-adjacent capability asks for a per-app grant on first use (FR-018,
scenario 5), and a capability the shipped catalogue does not classify is never
granted (FR-018's last sentence);
- with no network, opening an app presents a cached surface or a stated offline
state, never a blank screen (FR-020, scenario 6);
- epiloYES presents as an app with its own surface rather than a browser tab
(FR-016, scenario 1);
- language and place change independently of one another (FR-035, scenario 2).

Four verification properties are separately testable against the headless engine
and belong in `cargo test` rather than in an acceptance script — FR-019a states
each as a distinct refusal:

| Case | Expected |
| --- | --- |
| Surface signed under a root other than the pinned one | refused; cached copy retained; refusal stated |
| Signed app identity ≠ the app about to render | refused |
| Signed manifest digest ≠ the manifest whose capabilities would apply | refused |
| Delivered version < cached version | refused; cached copy retained; refusal stated |

An unverifiable surface is refused and the refusal **stated** — never shown as a
blank surface, which is what FR-020 already requires of the offline case.

## B4. SC-009 — the four navigation failures, per platform

**Requirement**: FR-015, SC-009. Also Story 1 acceptance scenario 4.

**Platform**: [tier 1] and [tier 2]. SC-009 says "on every supported platform",
so a green run on one tier is half the criterion.

**Prerequisites**: the reshaped `Engine` seam and the backend for that tier.

**Command** *(the headless half runs today; the per-tier half does not exist
yet)*:

```
cargo test --all                                  # today, the seam's half
scripts/navigation-failures.py --tier 1           # the platform's half
```

**Pass**: each of the four causes produces an error state naming the cause and
offering a next step, in the member's language; zero failures presented as
successful blank pages; zero loading indicators that do not resolve within 30 s.

`cargo test --all` reaches none of that today, and it is not the shell's half:
it establishes that the four causes are four distinct values whose four distinct
messages name the address, and nothing more. `LoadError`'s `Display` strings
offer no next step, and `crates/evreos-engine/src/lib.rs` documents them as
"deliberately not the member-facing copy, which is localised" under FR-035. The
next step and the language are the shell's, and A2 records that the shell's own
handling has no test at all.

Per cause, with what actually produces it on each tier:

| Cause | Tier 1 | Tier 2 |
| --- | --- | --- |
| Unresolvable address | a name-resolution error status from the navigation-completed event | the corresponding `NSURLError` code on the failure delegate callback |
| Untrusted or expired certificate | a certificate error status, whose detail populates the error state | the corresponding certificate `NSURLError` code |
| Request for authentication | an authentication-required status, corroborated by the basic-authentication event | the authentication-required code, or the authentication-challenge callback |
| Intercepting network | **open question — see below** | **open question — see below** |

Three of the four are distinguishable from each platform's own API on both
shipping tiers; Phase 0 research established that. **`Intercepted` is
distinguishable on neither.** A captive portal is not an error on either
platform: it answers, so the navigation succeeds. So this scenario has an open
question attached to it rather than a command:

> **Open**: is there any combination of platform signals that distinguishes an
> intercepting network from a successful load, without an outbound probe?
> **Settled by**: driving each tier's backend through a real captive portal on
> that tier's reference runner and recording the full signal tuple. If nothing
> distinguishes it, the question becomes a founder decision — whether an outbound
> probe is permissible at all, which FR-007a's closed list governs — and,
> separately, whether `Intercepted` remains in the enum or FR-015 is amended. It
> is not a question a backend implementation may settle by guessing.

Until that is settled, the `Intercepted` case is exercised through the headless
engine, which can script it, and is reported as exercised-in-the-shell rather
than exercised-on-the-platform. Reporting it otherwise would be the
indistinguishable-cause state FR-015 exists to forbid, dressed as a pass.

The 30-second clause has no home in `LoadError` at all: a stalled load is an
absence of an event, not a cause. It is the shell's timeout policy, and it is
testable only against a seam that can express an in-flight load — which the
event contract now can: a navigation with no terminal event is that state, and
the shell-side bound lands with the consumer that applies it.

## B5. SC-009a — the tier-2 floor

**Requirement**: SC-009a, FR-041's last sentence.

**Platform**: [tier 2], plus at least one machine below the floor.

**Prerequisites**: a tier-2 installer.

**Command** *(does not exist yet)*:

```
scripts/floor-acceptance.sh --os 13.0
scripts/floor-acceptance.sh --os <current release>
scripts/floor-acceptance.sh --os <each major version between>
scripts/floor-acceptance.sh --os <below floor>    # expects refusal
```

**Pass**: installs and launches on macOS 13.0, on the current release, and on
every major version between. On a machine below the floor, the pre-download
statement FR-041 requires is present on the distribution page **and** the
installer refuses with a named plain-language reason. Zero cases of the floor
being discovered only after installation.

## B6. SC-008 — the accessibility pass

**Requirement**: FR-034 (WCAG 2.1 AA on every shell surface), FR-011, FR-005,
FR-036, SC-008, Principle X. Principle X makes a failure a release blocker
rather than polish, and Governance names Principle X violations release
blockers.

**Platform**: [tier 1] and [tier 2]. ADR-0001's accessibility rationale is
evidenced on Windows only and covers **page content, not the shell's own
chrome** — the tab strip, address field and app surfaces carry their own
obligation, and what renders them is spike S4's output. So this pass is owed on
both tiers and cannot be inherited from the engine on either.

**Prerequisites**: the chrome exists (S4 decided). Every candidate chrome
renderer is disqualified by SC-006 before it reaches this scenario, so B9's
SC-006 measurement precedes it.

**Command** *(does not exist yet)*:

```
scripts/a11y-acceptance.py --tier 1 --surfaces all --report runs/<commit>/a11y/
```

**Pass**, all four clauses of SC-008 on every shell surface:

1. **WCAG 2.1 AA.** Automated check plus a manual pass; the automated half is
necessary and not sufficient. **[gap]** — the specification states WCAG 2.1 AA
for shell surfaces and supplies no mapping for non-web software, and several of
its success criteria (page titled, bypass blocks, multiple ways, consistent
navigation, consistent identification, language of parts) have no coherent
reading for a tab strip. A written interpretation must be committed before this
scenario has a pass condition; see *Gaps*.
2. **Full keyboard operation.** Every action reachable by pointer is reachable
   by
keyboard (FR-011), including getting *out* of a focus trap in page content — the
chrome/content boundary is where this class of interface commonly fails, and the
spec's edge case names it. Focus order and reading order remain coherent across
that boundary in both directions.
3. **200% scaling.** Every surface remains usable, legible and unclipped
(FR-005). Chrome layout scale and the engine's rasterization scale are set
together or the two disagree at 200% and content scales twice or not at all;
page zoom is a third, separate value.
4. **German dead-key and Greek text entry** (FR-036), in the FR-003 combined
field, the find-in-page field, and every chrome text input. This is the shell's,
not the engine's: the address field is the most-used text input in the product
and it is ours.

Driven with each platform's own assistive technology — Narrator and NVDA on tier
1, VoiceOver on tier 2 — which is what ADR-0001 risk 7 requires before WCAG 2.1
AA is claimed anywhere but on tier-1 page content.

> **Open**: does an accessibility tree published by the chrome compose coherently
> with an embedded WebView2 or WKWebView's own tree — one reading order, one
> focus order, no orphaned subtree? **Settled by**: a spike building a minimal
> chrome with one embedded webview on each tier's runner, driven by each
> platform's assistive technology, with the resulting tree captured and
> committed. Nothing located answers it, and if the answer is bad on the drawn
> chrome candidate, chrome accessibility becomes this project's own engineering
> problem against a release-blocking principle.

**FR-041's distribution page is a separate obligation and a separate test.** It
is neither a shell surface under FR-034 nor interface text under FR-035, and
FR-041 carries both obligations to it and states how they are verified: an
automated WCAG 2.1 AA check, a keyboard-only pass over the whole download path,
and a rendering of each of `de`, `el` and `en` showing no untranslated string
and no fused language-and-place value — in the text, in the download links, and
in their parameters. Verified on the **published** page before each release that
page advertises, published with the release, and a failure blocks that release.

## B7. Story 1 scenario 2 — background suspension and the idle figure

**Requirement**: FR-002, SC-005, and Story 1 acceptance scenario 2.

**Platform**: [tier 1 runner] and [tier 2 runner].

**Prerequisites**: the runner; the backend; ten tabs.

**Command** *(does not exist yet)*:

```
scripts/idle-soak.py --tier 1 --tabs 10 --hidden 9 --minutes 60
```

**Pass**: background tabs are suspended according to a **stated** policy, the
suspension is reversible without losing page state visible to the member
(FR-002), and processor use stays below SC-005's two bounds without audible fan
activity.

This scenario carries the plan's largest unretired exposure, and it is stated
here rather than discovered later:

> **Open**: what actually throttles a hidden background tab on each shipping tier
> at its floor? Phase 0 research established that `wry`'s background-throttling
> option is documented Unsupported on Windows and Supported only from macOS 14,
> so neither shipping tier gets it at its floor. FR-002 requires a *stated*
> policy, and a policy stated from an API that does not exist at the floor would
> be a claim about behaviour nobody measured. **Settled by**: ten tabs on each
> runner with nine hidden, CPU sampled against SC-005's window, and separately
> whether the tier-1 memory-usage-level lever moves SC-004's number. If neither
> suffices, FR-002's suspension has no mechanism on that tier and that is a
> finding for the plan.

> **Open**: does the engine's own idle floor fit inside SC-005's 5 ms wake-free
> 1-second sample and 18 s 60-minute window on each tier's reference machine?
> SC-005 counts "the same processes SC-004 counts", which includes the runtime's
> browser, renderer, network and GPU processes, whose idle timers Evreos does not
> author and cannot remove. SC-005 is **ratified**, therefore tighten-only, so if
> the floor exceeds the bound the remedy is an amendment to the specification
> recording the founder decision, the measured evidence and what discipline
> replaces the budget removed — not a code change. **Settled by**: a 60-minute
> idle measurement of a bare system-webview window with one suspended tab on each
> runner, run **before** SC-005 is treated as achievable.

SC-005's second half is not a measurement at all and must not be scheduled as
one: "no periodic timer outside the enumeration may exist on the idle path,
verified by design review and by instrumentation of scheduled work rather than
by observation, since no finite window can falsify a timer with a longer
period." The check is therefore at build time against the wake enumeration in
`budgets.toml`, plus a design review — a soak can corroborate it and can never
discharge it. **[design]** the specific enforcement mechanism.

## B8. SC-004 — ten-tab memory

**Requirement**: SC-004. Ratified on tier 1; provisional on tier 2, because
ADR-0001 records that what governs macOS memory at ten tabs is unestablished.

**Platform**: [tier 1 runner], [tier 2 runner]. Nowhere else.

**Prerequisites**: the runner; the backend; the host/factory seam above `Engine`
that owns the shared platform context — without it ten tabs mint ten contexts
and this entry is lost before product code exists; the ten-page corpus.

**Command** *(does not exist yet)*:

```
scripts/memory-soak.py --tier 1 --tabs 10 --hours 8 --corpus corpus/ten-tab/
scripts/memory-soak.py --tier 1 --tabs 10 --minutes 60     # per-change regression run
```

**Pass**: at or below 150 MB at **every** 5-second sample, from the first tab
opening until the session closes; the whole-machine cross-check delta does not
exceed the summed per-process figure by more than the margin declared for the
entry in `budgets.toml`, and an undeclared margin is zero. Counters are
`PROCESS_MEMORY_COUNTERS_EX.PrivateUsage` on Windows and `phys_footprint` on
macOS, summed over every process Evreos launches or causes to be launched —
never a resident-set counter, which SC-004 rejects by name because it would
report the eight-hour leak this criterion exists to catch as a reduction. Memory
the shell places in sections shared between its own processes is counted once
rather than dropped. The two counters are different quantities, so the two
tiers' figures are never compared with each other.

**Release rule, stated because it is easy to lose**: the full 8-hour soak MUST
pass on the exact commit a release artefact is built from, before that artefact
is published. A shortened run, or a soak of an earlier commit, does not release
an artefact.

The corpus is a pinned, content-addressed set of ten pages served from loopback,
published under SC-013 so a third party can rerun it; a live page's payload
changes between runs by more than a memory regression does. **[gap]** — no
requirement names the ten pages; see *Gaps*.

## B9. SC-006 — chrome input latency

**Requirement**: SC-006, ratified. Also the gate that disqualifies chrome
renderer candidates before B6.

**Platform**: [tier 1 runner], [tier 2 runner], on a display driven at 60 Hz.

**Prerequisites**: the runner; a display at 60 Hz; the chrome, because both
interactions are with the shell's own chrome and there is no shell-side
instrumentation point until it exists. The injector and the present-timeline
reader can be built before the chrome and are the half that does not wait.

**Command** *(does not exist yet)*:

```
scripts/latency.py --tier 1 --interaction tab-switch --trials 1000
scripts/latency.py --tier 1 --interaction address-keystroke --trials 1000
```

**Pass**: a visible response within 16 ms at the 99th percentile of at least
1000 trials per interaction, **and no trial over 16 ms at all** — a single trial
over 16 ms fails the gate.

Three rules the harness must implement rather than the operator remember:

- **Individual trials are never discarded**, for any reason. Discarding the two
worst of a thousand is a 99.8th-percentile bar and reinstates the dropped frame
this criterion forbids.
- **A whole invocation** may be discarded only for a recorded, externally
observable cause the harness detects — a competing process, thermal throttling,
a failed instrumentation check — never for an outlier judged environmental after
the fact.
- **The discard budget is two per head commit, counted cumulatively across every
run of the gate on that commit.** Re-running does not reset it; the third
discard fails the gate; only a new head commit carries a new budget. A harness
that knows only about its own invocation grants two discards per run, which is
unlimited retries with extra steps. The ledger is therefore durable state
indexed by head commit SHA, read before the gate decides, and every discard is
published under SC-013 with its cause and its commit.

The bar stays 16 ms where a machine's native refresh is higher: it is a
human-perception budget, not a hardware-relative one.

**[gap]** — the local profile the address-field keystroke is measured against is
not fixed by any requirement, and FR-003 combines search, history and bookmarks
so the response time is a function of how much local data there is. A figure
measured on a fresh profile and one measured on a year-old profile are different
quantities under the same entry, which SC-013's third party cannot reproduce.
See *Gaps*.

## B10. SC-014 and FR-007a — the traffic capture

**Requirement**: SC-014, FR-007a's conformance paragraph, Principle VI, and the
Permanent Prohibition on server-side collection of browsing history. Two
distinct obligations, and they are not the same test.

**FR-007a's conformance test** is committed to this repository and run in CI. It
exercises first launch, typing in the FR-003 field **without submitting**, a
submitted search, navigation to a site with subresources, a private window, and
a hand-off; it fails on any outbound request from Evreos that no entry in
FR-007a's closed list accounts for.

**SC-014's capture** is a scripted session on a fresh profile — first run, a
search, ten navigations across sites, a download, a private window, then close
and reopen — with all outbound traffic captured and transport encryption
terminated at the harness so payloads are readable. The capture, the script and
the analysis are published under SC-013. It is rerun on the exact commit a
release artefact is built from.

**Platform**: [tier 1] and [tier 2], on real hardware per tier. This cannot be a
portable CI job: WebView2 and WKWebView open their own sockets and no dependency
lint sees them, so the capture is the only instrument for the engine's half. It
must capture DNS as well as TLS payloads, because FR-007a names name resolution
explicitly.

**Command** *(does not exist yet)*:

```
scripts/traffic-capture.py --tier 1 --script sessions/sc-014.json --out runs/<commit>/capture/
scripts/traffic-analysis.py runs/<commit>/capture/ --report runs/<commit>/analysis.md
```

**Pass**: every URL-bearing payload in the capture is one FR-007a permits —
inherent in a function the member invoked on that occasion, carrying no more
than that function needs — and each is listed in the published analysis with the
invoking function named. Any URL-bearing payload the analysis does not list, and
any diagnostic report at all on a profile where diagnostics were never enabled,
fails the criterion.

Two things the capture must specifically assert, both established by Phase 0
research as live from the first Windows build and both failing silently
otherwise:

- **The tier-1 runtime's reputation service is off**, and stays off. FR-007a's
final paragraph makes a runtime feature that sends visited addresses on its own
Evreos's transmission and requires it turned off; a reputation service is the
case that paragraph names by example. The setting is per-webview, takes effect
only on the next navigation, and resets to enabled for every webview sharing a
user data folder when a new one is created — so "kept off" is a per-webview
invariant asserted in the capture, never a one-time call at startup.
- **The tier-1 runtime's own crash reporting sends nothing to its vendor.** A
renderer minidump contains page memory, hence URLs and page content; no entry in
FR-007a's list accounts for it, FR-039 requires diagnostics off until the member
turns them on, and FR-039c bans capturing page memory at all. The vendor's
custom-crash-reporting option is what suppresses it, and any dumps it produces
locally are deleted unread rather than parsed.

Two open questions attach:

> **Open**: what else does the tier-1 runtime transmit on its own — field-trial
> seeds, component updates, secure-DNS auto-upgrade to a resolver the member
> never chose, predictive preconnect from its own loading predictor? FR-007a's
> "no prefetch, preconnect or name resolution derived from history" bites on the
> last. **Settled by**: the capture itself on the tier-1 runner, with every
> destination classified. Anything unaccounted-for is turned off or reported as a
> limitation. Vendor documentation informs this; it is not evidence about the
> built artefact.

> **Open**: does the harness CA needed to read payloads change what the capture
> observes — HSTS, pinning, or the runtime declining interception — such that
> some traffic escapes unrecorded? **Settled by**: running the capture with and
> without termination and reconciling the connection counts; any connection
> present in the unterminated run and absent in the terminated one is traffic the
> analysis cannot see and must be named as such.

**[gap]** — SC-014's criterion reads on "every URL-bearing payload", and a
conforming build emits at least the FR-014 update check in that scripted
session, which is a payload bearing a URL and which FR-007a permits because it
carries none of the four governed things. Read literally, SC-014 fails a build
for doing something FR-007a allows. Which reading governs is a founder decision.
See *Gaps*.

## B11. Budget measurements, entry by entry

Eighteen entries, nine per platform, and the list is closed. What each needs:

| Entry | Runner needed | Gates from M0 | Notes |
| --- | --- | --- | --- |
| SC-001 download size ×2 | No runner, but the **target platform's installer** | Absolute and regression block from M0 (not hardware-dependent) | Unmeasured with its reason until each platform's installer exists; then read from that platform's published artefact on the host that builds it, keyed by platform — see A6 |
| SC-001 installed footprint ×2 | No runner, but a **clean machine image** of the target platform | Both block from M0 | Disk delta after first run completes, excluding member data; a compiled blocking rule list materialised at first run lands inside this measurement |
| SC-002 warm start ×2 | Tier runner | Regression blocks; absolute advisory until the runner is pinned | All four **provisional**, pending the cold-start spike (B12) |
| SC-002 cold start ×2 | Tier runner | As above | Cold start is "first launch after installation, on a fresh profile"; **[gap]** the machine's own cache and prefetch state is not fixed by the criterion, and two labs following the text exactly will differ |
| SC-004 ten-tab memory ×2 | Tier runner | As above | B8. Ratified tier 1, provisional tier 2 |
| SC-005 window figure ×2 | Tier runner | As above | B7 |
| SC-005 wake-free sample ×2 | Tier runner | As above | B7. The enumeration half is a build-time check, not a measurement |
| SC-006 tab switch ×2 | Tier runner, 60 Hz display | As above | B9 |
| SC-006 address keystroke ×2 | Tier runner, 60 Hz display | As above | B9 |

**The SC-001 A/B every plan needs and no requirement names.** The first change
that adds `wry` and its platform dependency tree is the first time SC-001's
ratified 20 MB meets a real dependency tree. Measure it as two commits on the
same branch — backend absent, backend present — each run through
`scripts/check-budgets.py`, and let the difference be the stated cost FR-043
requires that pull request to carry. Both SC-001 entries are non-hardware-
dependent, so their absolute gate blocks from M0: the number is enforced, not
asserted. The commit that first measures an entry must also write its baseline,
or the regression half of that entry's gate stays inert (A6).

**Spike exemption.** A change whose purpose is to establish a figure that does
not yet exist is exempt from that one entry's absolute gate and from nothing
else. It never lifts the regression gate and never lifts the budget-file gate.
The exemption is recorded on that entry naming the pull request and the figure
it measures, and a build produced while an exemption is unretired must not be
released or tagged. The exemption is available only to a change that ships no
behaviour, and code reachable in a shipped binary is behaviour whatever flag
guards it — so a spike behind a disabled feature flag is not exempt.

## B12. The spikes

These are measurements. Each one's command produces a number or a recorded
observation; none of them may be answered by choosing, and this document
predicts no result for any of them.

| Spike | Question | Platform | Blocks |
| --- | --- | --- | --- |
| **Cold-start spike** | What is the engine's own initialisation floor, warm and cold? | [tier 1 runner], [tier 2 runner] | SC-002's four provisional entries; the spec says the shell architecture is expected to be shaped by what it finds |
| **macOS memory spike** (Q-E9, ADR-0001 risk 9) | What actually shares state between webviews on macOS, and what governs memory at ten tabs? | [tier 2 runner] | SC-004's provisional tier-2 entry |
| **Q-E10** | Does affiliate attribution survive tracking prevention on tier 2? | [tier 2] | ADR-0001 risk 1 requires this tested before the wallet is designed around cookies — an ordering constraint on B2, not only a risk |
| **Q-E11** | Does PlayReady reach the Win32 WebView2 host, does it cover any service members use, and at which security level? | [tier 1] | Nothing may be claimed **or excluded** about content protection until it retires |
| **Q-E11b** | Does a third-party WKWebView host reach the platform's content-protection system through EME at all, and which services members use depend on it? | [tier 2] | As above; the spike must establish the demand as well as the capability |
| **Q-E12** | Can FR-008 be met on macOS 13 by compiled rule lists, at what cost, or must the floor move to macOS 14? | [tier 2 runner] | The tier-2 floor decision, and FR-008 as a P1 acceptance scenario |

The cold-start spike's method is available today at no platform cost, and it
falls out of the seam Principle III already forced into existence: build the
shell twice on the same commit and the same runner, once linked to the headless
engine and once to the system-webview backend. The shell code is identical, the
headless engine renders nothing, and the difference is the runtime's
contribution. For that to work, both builds must reach the identical,
shell-emitted "interactive" event — which means the headless configuration must
drive a real window rather than a console loop. That is a requirement on the
shell, not on the harness: the harness cannot invent an event the shell does not
emit. **[gap]** — the specification never defines "interactive"; see *Gaps*.

Two spikes should be scheduled together rather than separately, because they
share one dependency: app-surface network confinement on tier 2 and Q-E12's
blocking parity both reach WebKit's compiled rule lists through the same route.

Q-E10's method also has a second run nobody would think to schedule: run it
twice on each tier, with the shipped blocking configuration enabled and
disabled, since the lists Evreos ships under FR-008 may themselves break the
click-out redirect chain the cashback flow depends on. Whether they do is
unverified.

## B13. Checks that run on ordinary CI and need no runner at all

These are the cheap half, and putting them early is what makes the expensive
scenarios short.

| Check | Requirement | Command *(none exists yet)* |
| --- | --- | --- |
| The fixture brand builds | FR-042, Principle VIII, Q-E13 | `cargo build --release --features fixture-brand` — proves the seam on every change rather than asserting it; no partner build is promised |
| No brand name, colour, endpoint or support address outside the brand configuration | FR-042 | a source scan, failing on a hardcoded value |
| No region subtag in any catalogue key or filename, and no request field fusing language and place | FR-035, Principle VII | a catalogue and request-builder scan |
| No app surface, cached copy, or manifest in a release artefact | FR-019b | a release-artefact scan in the idiom of `scripts/check-budgets.py`; **necessary because signature verification cannot enforce this** — a pre-cached surface would carry a valid signature and satisfy FR-019a, which is why FR-019b exists separately |
| Post-install, offline, every app presents FR-020's stated offline state rather than content | FR-019b, FR-020 | install, assert the surface cache is absent or empty before any network activity, then launch offline |
| No workspace crate but the platform-FFI one lifts `unsafe_code = "forbid"` | **[design]** — nothing in the constitution or the spec forbids `unsafe`; Principle III constrains nightly features, not unsafety | a manifest scan. The policy itself is already on `main` and does not need deciding again: the workspace root sets `unsafe_code = "forbid"` under `[workspace.lints.rust]`, and all three crates repeat `#![forbid(unsafe_code)]` in their own source. What does not exist is the check that no crate lifts it, and the carve-out the platform-FFI crate will need — the pull request that writes that carve-out is the one that should say so |
| The two `Engine` implementations mean the same thing | **[gap]** — FR-044 requires the headless implementation kept working, not that both implementations agree | a conformance battery both implementations run |
| Filter-list conversion drops no more rules than the committed baseline, per failure reason | FR-008, ADR-0001 risk 5 | runs on any platform, so it does not wait on the tier-2 runner |
| Compiled rule lists stay under the 150,000 **emitted JSON rule** ceiling, per list, and every partition carries the full exception set | FR-008 | the ceiling is checked on the emitted artefact, not on source rules; a size-based split silently disables exception rules and the sites that breaks are the bank and government sites the spec names as abandonment triggers |

## B14. FR-015a — the site-credential autofill test

**Requirement**: FR-015a. This is a **release blocker per tier**, not a
deferrable question, and it is worth pulling out of the list because its shape
is unusual.

**Platform**: [tier 1] and [tier 2], each at its floor.

**Prerequisite**: a backend on that tier.

**Pass**: the result — the platform version tested, the reference machine, and
presence or absence — is committed to this repository and owned by the founder.
A tier MUST NOT be released until its result is committed. FR-041 forbids the
distribution page asserting either presence or absence until the result exists,
so an untaken test leaves the member without the statement FR-041 requires
before installing.

Where autofill is absent, the behaviour to test is: the limitation stated
**before** installation, and an offer to open the site in the hand-off browser
when a site-credential field is detected. Detection is local, inspects only
whether a password-type input is present, and transmits and retains no page
content. Evreos stores no site credentials of its own in v1 (Q-E4, Q-E5).

---

# Part C — Platform matrix

| Scenario | Linux dev machine | Windows 11 dev machine | macOS 13+ dev machine | Tier-1 runner | Tier-2 runner |
| --- | --- | --- | --- | --- | --- |
| A1 build, A2 tests, A3 shell, A4–A5 budget gate | yes | yes | yes | yes | yes |
| B3 signature verification core, B13 CI checks | yes | yes | yes | yes | yes |
| B4 SC-009, shell half | yes | yes | yes | yes | yes |
| B4 SC-009, platform half | no | yes | yes | yes | yes |
| B1, B2, B5, B14 behavioural acceptance | no | tier 1 only | tier 2 only | yes | yes |
| B6 SC-008 accessibility | no | tier 1 only | tier 2 only | yes | yes |
| B10 SC-014 capture | no | tier 1 only | tier 2 only | yes | yes |
| B7, B8, B9, B11 figures | no | **exercise only, no figure** | **exercise only, no figure** | tier-1 figures | tier-2 figures |

The two "exercise only" cells are the trap. A Windows or macOS developer machine
runs the harness and produces numbers; those numbers may not be recorded in
`budgets.toml`, published under SC-013, used to reset a baseline, or reported as
met on reference hardware. The preamble binds each figure to its tier's pinned
runner and to no other machine, and the reference machine is the *oldest*
configuration that tier's floor admits at 8 GB — a modern laptop passing is not
evidence that the reference machine passes.

**Procurement is on the critical path for validation, not only for the gates.**
SC-002, SC-004, SC-005 and SC-006 all wait on it; so does the chrome decision,
because S4's candidates are discriminated by SC-006 on the tier-1 runner; so
does the SC-005 idle floor, whose answer may be a specification amendment. Buy a
cold spare of identical configuration per tier and write the swap procedure
down: the budget file records a durable machine identifier, so swapping in a
different machine changes that identifier and, honestly applied, restarts every
baseline series on that tier.

One configuration note that belongs to whoever wires the runners up: the
hardware-dependent jobs need self-hosted runners, and this repository is public.
Restrict those jobs to non-fork pull requests. Everything not hardware-dependent
stays on hosted runners.

---

# Gaps

Each of these is something a scenario above needs and no requirement supplies.
None is smuggled in as though it were required. Each names what would settle it.

1. **"An interactive window appears" is undefined.** SC-002 states the figure
   and
never defines the endpoint; no definition was located in the specification, the
constitution or ADR-0001. An undefined endpoint cannot be reproduced by a third
party, so SC-013 fails on SC-002 however carefully the milliseconds are
measured. *Settled by*: a founder reading recorded in the specification. One
candidate worth costing: bind it to SC-006's instrument — the first presented
frame at which an injected address-field keystroke is accepted and produces a
visible response — so the two criteria share one definition and cannot drift
apart.
2. **SC-013's reproduction band is the per-entry tolerance, which the preamble
requires to be justified by run-to-run variation on one machine.** SC-013
applies it across machines of a reference *class*. These are different
quantities and the second is normally larger. *Settled by*: measuring the same
commit on two or three further machines of each reference class and comparing
the spread against the declared tolerances. If the spread is larger, one of the
two rules needs an amendment — and it cannot be fixed by widening the tolerance,
which would breach the preamble's own justification rule and its 5% cap.
3. **SC-014's "every URL-bearing payload" versus FR-007a's history-bearing
scope.** A conforming build emits the FR-014 update check in SC-014's scripted
session; it bears a URL and carries none of the four things FR-007a governs.
*Settled by*: a founder decision landing as a spec amendment — either restating
SC-014's criterion in terms of history-bearing payloads, or adding a committed
closed list of permitted non-history destinations the analysis reads. Not an
implementer's call: it changes what the criterion means. Suppressing the update
check during the capture is not available, because it makes the capture a
measurement of a build nobody ships.
4. **WCAG 2.1 AA has no stated mapping for non-web software.** FR-034 applies it
to every shell surface; several of its success criteria have no coherent reading
for a tab strip, and a reviewer with no written mapping will either wave them
through or block arbitrarily. *Settled by*: a written interpretation committed
to this repository before B6 has a pass condition. Note that FR-041's
distribution page is a web page, so plain WCAG 2.1 AA applies there and the two
tests are different tests.
5. **Nothing requires the two `Engine` implementations to mean the same thing.**
FR-044 requires the headless implementation kept working; the current tests
exercise the headless engine alone. As it stands the second implementation
proves the seam *compiles* twice, not that it *means* the same thing twice.
*Settled by*: a conformance battery both implementations run, which is a plan
decision costed under FR-043, not a requirement.
6. **The ten pages of the SC-004 corpus are not named.** SC-004 states the count
and the boundary; nothing names the pages, and SC-013's reproducibility needs
them pinned and content-addressed. *Settled by*: a recorded founder decision
naming them — the cohort's daily surfaces rather than a synthetic benchmark,
archived on a recorded date, including at least one first-party app surface,
since FR-016/FR-019 apps render inside the shell and their memory is Evreos's
memory under SC-004's boundary.
7. **The local profile for SC-006's address-field entry is not fixed.** FR-003
combines search, history and bookmarks, so keystroke response is a function of
local data volume, and neither SC-006's stated conditions nor `budgets.toml`
names one. *Settled by*: a generated profile of published size and shape,
recorded as that entry's measurement condition, with a larger corpus run as a
non-blocking scaling check. A live member-shaped profile is not available: it
would contain browsing history, which FR-007a governs and which cannot be
published under SC-013.
8. **SC-002's cold-start condition does not fix the machine's own cache state.**
Every clause of "no cached profile state on the machine" is about Evreos's
state; none is about the operating system's file cache, prefetch database or
standby list — and start-up time is exactly the quantity that difference moves,
plausibly by more than the ≤5% tolerance. *Settled by*: paired cold-start
trials, rebooted versus not, on one commit; the answer decides whether the
entry's condition must mandate a reboot for SC-013 to hold.
9. **The budget file schema has no unit field.** SC-002 and SC-006 are
milliseconds and SC-005 is a percentage plus two processor-time bounds, against
a `figure_mb`/`baseline_mb` schema. The preamble requires each entry to carry
its figure and its measurement condition and names no unit. *Settled by*: a
schema change landed with the fourteen missing entries, before any measurement
lands — encoding milliseconds in a field named `_mb` makes the tolerance
arithmetic silently wrong across units.
10. **The diagnostic signal cannot be validated at all until a relay contract
    exists.** FR-039b: "Where no operator is named or no such contract is in
    force, the diagnostic signal MUST NOT be offered and no report may be
    transmitted." So the release gate on FR-039 is a contract, not code, and the
    build must have a state in which no report path exists — which is also the
    state SC-014's capture exercises on a fresh profile. *Settled by*: a signed
    contract with a named operator and a stated effective date, which FR-039b also
    requires to appear in the pre-consent disclosure. Procurement, not
    engineering, and it starts in Phase 1.

---

## What this document is not

It is not a tutorial, and it carries no implementation. It is also not a list of
things that will pass. Several scenarios above are written so that they can
fail, and two of them — SC-005's idle floor against a ratified tighten-only
figure, and FR-006's permission prompts at the tier-2 floor, where the
platform's public geolocation permission delegate is annotated far above macOS
13 and no notification delegate is declared at all — have outcomes whose remedy
is an amendment to the specification rather than a change to the code. Learning
that early costs an afternoon on a runner. Learning it after the harness is
built costs the harness's assumptions too.
