# Specification Quality Checklist: Evreos v1

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-08-30
**Feature**: [spec.md](../spec.md)

## Content Quality

- [ ] No implementation details (languages, frameworks, APIs) — names are present
  and load-bearing in six places, not four. Engine, binding and platform-API
  names appear in FR-044, Non-Goals, the Edge Cases entry on protected media,
  Unestablished scope, Platform Scope and Spikes: Electron, CEF and Chromium;
  WebKit, `wry`, `WKContentRuleList` and `WebKitUserContentFilterStore`;
  PlayReady, EME, Win32 and a WinUI2/UWP WebView2 host. The Validation record
  below states what each place carries and why. Alongside them, SC-004 names
  platform memory counters
  (`phys_footprint` via `task_vm_info` on macOS, Private Bytes —
  `PROCESS_MEMORY_COUNTERS_EX.PrivateUsage` — on Windows, Working Set — Private
  named only to exclude it) and the Linux `smaps` construct it rules out;
  SC-009a names macOS 13.0; FR-039b names a relay; and FR-044 names stable Rust.
  Each is a measurement boundary, a privacy mechanism or a constitutional
  mandate rather than a free technology choice, but they are API, platform and
  language names, and the item cannot be ticked while that question is open
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain
- [x] Requirements are testable and unambiguous
- [ ] Success criteria are measurable — Q-E9a names the reference machine for
  each tier, so the four criteria carrying hardware-dependent figures (SC-002,
  SC-004, SC-005, SC-006) are reproducible under SC-013; they cannot be reported
  as met, and their absolute gates cannot block, until those two machines are
  procured and pinned as CI runners
- [ ] Success criteria are technology-agnostic — SC-004 names platform memory
  counters and the Linux `smaps` construct it rules out, and SC-009a names macOS
  13.0; both are load-bearing, for reproducible measurement and for the tier-2
  floor respectively
- [x] All acceptance scenarios are defined
- [x] Edge cases are identified
- [x] Scope is clearly bounded
- [x] Dependencies and assumptions identified

## Feature Readiness

- [ ] All functional requirements have clear acceptance criteria — **the rule**: a
  requirement is covered when an acceptance scenario cites it in the trailing
  citation that names what that scenario accepts, or when a success criterion
  measures it. All 18 acceptance scenarios carry such a citation — *(FR-015)* —
  so the first limb is a computation over those 18 citations, and not over
  wherever an identifier happens to appear: FR-019a's reference to the FR-020
  cache and SC-005's enumeration of FR-014's update check are cross-references,
  not coverage. Run over the 61 functional requirements, the rule yields 27
  covered in full, 5 in part and 29 not at all.

  **Neither a scenario nor a criterion reaches** FR-003, FR-003a, FR-004, FR-006,
  FR-007, FR-009, FR-010, FR-014, FR-015a, FR-018b, FR-019a, FR-019b, FR-021,
  FR-023, FR-024, FR-026a, FR-029a, FR-031, FR-033, FR-036a, FR-037, FR-039b,
  FR-039c, FR-039d, FR-039e, FR-039f, FR-042, FR-043 or FR-044 — 29 in all.
  Three of those sit here on the citations rather than on the subject matter:
  Story 2 scenario 2 opens the catalogue but cites FR-025 alone, so nothing
  reaches **FR-024**; Story 2 scenario 1 exercises the deliberate scan but cites
  FR-032 alone, so nothing reaches **FR-033**; and **FR-043** is enforced by the
  budget-file gate the Success Criteria preamble defines, which is not a
  criterion. Adding the missing citation, rather than writing a new scenario, is
  what would move the first two.

  **Reached only in part**, each with what is missing — 5 in all:

  - **FR-005**: SC-008 measures interface scaling to 200%; nothing exercises
    find-in-page or page zoom.
  - **FR-016a**: Story 2 scenario 5 asserts the opt-in mandate — a member who
    never signs in has no money surface imposed. Discoverability and
    dismissibility are unexercised, and Principle IV makes all three release
    criteria.
  - **FR-018a**: Story 2 scenario 6 asserts that an offer activates only on an
    explicit action for that occasion. The rest of the injection rule — that no
    per-app grant supplies the occasion, and that a cashback offer alters no
    merchant page before the member acts — is unexercised.
  - **FR-030**: Story 2 scenario 6 cites it and asserts that attribution is
    never attached silently; nothing tests that attribution is never claimed for
    a purchase the member's action did not lead to.
  - **FR-041**: SC-009a covers the pre-download operating-system statement and
    the installer's refusal below the floor; nothing covers the accessibility
    and language obligations this requirement extends to the distribution page.
- [x] User scenarios cover primary flows
- [x] Feature meets measurable outcomes defined in Success Criteria
- [ ] No implementation details leak into specification — the same question as
  the first Content Quality item, and unticked for the same reason

## Notes

- Items marked incomplete require spec updates before `/speckit-clarify` or `/speckit-plan`.

### Clarifications and open decisions

Two clarification sessions are integrated, both dated 2026-08-30: six
clarifications in the first, eleven in the second. The spec's Clarifications
section records both.

Fifteen entries stand in Open Decisions, and none is open in full. Eleven are
settled outright: Q-E1, Q-E2, Q-E3, Q-E6, Q-E7, Q-E9a, Q-E11a, Q-E13, Q-E14,
Q-E15 and Q-E16. Two are settled with a residual recorded and explicitly
post-v1: Q-E4 (the tier-2 keychain) and Q-E5 (importing site credentials, which
reopens with Q-E4). Q-E9 is settled in part, two figures staying provisional
pending measurement — SC-002's, and SC-004's on tier 2. Q-E8 is closed as
superseded by Q-E13.

Four former entries are measurements rather than decisions and are carried in
the spec's Spikes section, keeping their identifiers so cross-references still
resolve: Q-E10, Q-E11, Q-E11b and Q-E12.

Q-E9a is a release prerequisite rather than a preference. The Success Criteria
preamble runs both measuring gates on the pinned benchmark runner and blocks the
build from M0, and it admits a figure as *met on reference hardware*, or
published under SC-013, only against a machine Q-E9a names. With the machines
named, procuring and pinning them is what remains.

### Constitutional mandate coverage

Each mandate below is held by the requirement named, so that a build can be held
to the requirement rather than to the principle's wording:

- **Principle II** — the single budget file and the per-change byte and
  millisecond cost: FR-043. The gate structure itself is defined once, in the
  Success Criteria preamble.
- **Principle III** — the engine seam proved by a second implementation, stable
  Rust with no nightly features on the release path, and no bundled engine in
  any release: FR-044.
- **Principle IV** — Apivo surfaces discoverable, opt-in and dismissible:
  FR-016a. Nothing injected into a page without an explicit action for that
  occasion: FR-018a.
- **Principle V** — no client-built affiliate deeplink: FR-025. Ledger amounts
  rendered rather than computed, and the cashback invariants left behind the
  service: FR-026 and FR-026a.
- **Principle VI** — browsing history stays on the machine: FR-007a. Opt-in
  diagnostics: FR-039. The *aggregate* condition: FR-039d, which counts crash
  reports under a symbol-keyed counter rather than retaining a report — a
  retained exemplar would be the same per-install payload under another name —
  and FR-039e, which sets the disclosure floor. Bounded crash-report content:
  FR-039c. EU hosting of every payload and derivative: FR-039f. The
  fingerprinting prohibition: FR-036a.
- **Principle VII** — BCP-47 primary-subtag keying, and language and place as
  separate values everywhere either appears: FR-035.
- **Principle VIII** — the single brand configuration and the fixture brand built
  in CI on every change: FR-042.
- **Principle IX** — capabilities declared in a signed manifest and not widenable
  from inside: FR-017. The per-app grant for anything page-adjacent: FR-018. The
  delivered surface itself signed and verified before rendering: FR-019a. A
  browser release carrying no app content: FR-019b.
- **Principle X** — WCAG 2.1 AA on every shell surface: FR-034. Full keyboard
  operation: FR-011. Scaling to 200%: FR-005. German dead-key and Greek text
  entry: FR-036. The distribution page, which is neither a shell surface nor
  interface text: FR-041.
- **Permanent Prohibitions** — advert injection: FR-018b, which carries the
  prohibition past FR-018a's consent rule because a Permanent Prohibition admits
  no consent exception. Silent affiliate attribution: FR-030 and FR-033.
  Server-side collection of browsing history: FR-007a.

Gaps identified but not yet carried as requirements are recorded in issue #28.

### Validation record

**No `[NEEDS CLARIFICATION]` markers are used.** Founder decisions are recorded
in the spec's Open Decisions section instead, which routes them to
`/speckit-clarify` and states they MUST NOT be resolved silently by the
specification. Success Criteria marks each figure they govern ratified or
provisional explicitly, and the CI gate exists either way, because Principle II
admits no un-gated budget.

**Implementation-detail review.** Engine, framework and language names are
present in the specification rather than kept out of it. What is kept out is the
engine *decision*, which lives in ADR-0001; the requirements themselves are
written against "the system web runtime" rather than against a named engine. The
names that do appear are load-bearing, and they appear in six places:

- **FR-044** names Electron, CEF, bundled Chromium and stable Rust, because
  Principle III rejects and mandates those by name.
- **Non-Goals** repeats Electron, CEF and bundled Chromium as the permanent
  Principle III rejection.
- **Unestablished scope** names PlayReady, EME, a WinUI2/UWP WebView2 host and
  open-source Chromium, because ADR-0001 risk 8 is a claim about those specific
  mechanisms and no vaguer statement is falsifiable.
- **Platform Scope** names `wry`, `WKContentRuleList`,
  `WebKitUserContentFilterStore`, WebKit and macOS 10.13, because the tier-2
  blocking gap is a gap in a named binding rather than in the platform, and that
  distinction is the whole content of the paragraph.
- **The Edge Cases entry on protected media** names PlayReady and the Win32
  host, for the same reason as Unestablished scope.
- **Spikes** carries the same names into Q-E11, Q-E11b and Q-E12, which are the
  measurements those paragraphs call for.

Four success criteria reference a system-provided web runtime as a measurement
boundary rather than as a technology choice: SC-001 excludes its bytes, SC-002
assumes it present, SC-003 covers its absence, and SC-004 excludes its
processes. Without that boundary the figures are not measurable, since what is
budgeted is the part Evreos ships. SC-004's counters and the `smaps` construct
it rules out are named because the two platforms expose different quantities and
an unnamed "memory" figure is not reproducible under SC-013; SC-009a's macOS
13.0 because the tier-2 floor is the criterion; and FR-039b's relay because the
unlinkability it requires cannot be met at the application layer alone.

**Platform tiering** appears in its own section rather than as a requirement,
because it bounds scope rather than describing behaviour.

**Known gap, deliberate.** SC-010, SC-011 and SC-012 are business outcomes
measured after release rather than at acceptance. They are retained because they
state the product's actual bar, and omitting them would hide it.
