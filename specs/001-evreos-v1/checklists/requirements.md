# Specification Quality Checklist: Evreos v1

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-08-30
**Feature**: [spec.md](../spec.md)

## Content Quality

- [ ] No implementation details (languages, frameworks, APIs) — names are present
  and load-bearing in four places: SC-004 names platform memory counters
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
- [ ] Success criteria are measurable — the four criteria carrying
  hardware-dependent figures (SC-002, SC-004, SC-005, SC-006) cannot be reported
  as met until Q-E9a names the reference machines, and SC-013's reproducibility
  bar rests on the same answer
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
  requirement is covered when an acceptance scenario cites it as what that
  scenario accepts, or when a success criterion measures it. Every acceptance
  scenario carries a trailing requirement citation — *(FR-015)* — so coverage is
  computed from those citations and from the criteria, and not from wherever an
  identifier happens to appear: FR-019a's reference to the FR-020 cache and
  SC-005's enumeration of FR-014's update check are cross-references, not
  coverage. The lists below are what the rule yields.

  **Neither a scenario nor a criterion reaches** FR-003, FR-003a, FR-004, FR-006,
  FR-007, FR-009, FR-010, FR-014, FR-015a, FR-018b, FR-019a, FR-019b, FR-021,
  FR-023, FR-026a, FR-029a, FR-031, FR-036a, FR-037, FR-039b, FR-039c, FR-039d,
  FR-039e, FR-039f, FR-042 or FR-044.

  **Reached only in part**, each with what is missing:

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
  - **FR-024**: the catalogue is opened in Story 2 scenario 2; nothing exercises
    language and place as independent parameters within it.
  - **FR-030**: Story 2 scenario 6 asserts that attribution is never attached
    silently; nothing tests that attribution is never claimed for a purchase the
    member's action did not lead to.
  - **FR-033**: Story 2 scenario 1 exercises the deliberate scan; nothing tests
    that attribution is never inferred from the installation.
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

Six clarifications are integrated; the spec's Clarifications section records
them.

Nineteen open decisions stand. Four are **partly** settled, each recording what
remains open: Q-E2 (the specific engine), Q-E4 (the tier-2 keychain), Q-E6 (the
rest of the diagnostic set) and Q-E9 (the SC-002 figure, and SC-004's tier-2
figure). Two are closed: Q-E5 as a consequence of Q-E4, and Q-E8 as superseded
by Q-E13. The remaining thirteen are open in full — Q-E1, Q-E3, Q-E7, Q-E9a,
Q-E10, Q-E11, Q-E11a, Q-E11b, Q-E12, Q-E13, Q-E14, Q-E15 and Q-E16. Q-E9a is a
release prerequisite rather than a preference, because the Success Criteria
preamble holds every absolute gate on a hardware-dependent figure advisory until
the reference machines are named, and Principle II admits no un-gated budget.

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

**Implementation-detail review.** Framework and engine names are kept out, and
the engine decision itself lives in ADR-0001 rather than here. Four success
criteria reference a system-provided web runtime as a measurement boundary
rather than as a technology choice: SC-001 excludes its bytes, SC-002 assumes it
present, SC-003 covers its absence, and SC-004 excludes its processes. Without
that boundary the figures are not measurable, since what is budgeted is the part
Evreos ships. The names that do appear are load-bearing rather than incidental,
and the first Content Quality item lists them: SC-004's counters and the `smaps`
construct it rules out, because the two platforms expose different quantities
and an unnamed "memory" figure is not reproducible under SC-013; SC-009a's macOS
13.0, because the tier-2 floor is the criterion; FR-039b's relay, because the
unlinkability it requires cannot be met at the application layer alone; and
FR-044's stable Rust, which Principle III mandates by name.

**Platform tiering** appears in its own section rather than as a requirement,
because it bounds scope rather than describing behaviour.

**Known gap, deliberate.** SC-010, SC-011 and SC-012 are business outcomes
measured after release rather than at acceptance. They are retained because they
state the product's actual bar, and omitting them would hide it.
