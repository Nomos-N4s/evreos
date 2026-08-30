# Specification Quality Checklist: Evreos v1

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-08-30
**Feature**: [spec.md](../spec.md)

## Content Quality

- [ ] No implementation details (languages, frameworks, APIs) — SC-004 names
  platform memory counters — `phys_footprint` via `task_vm_info` on macOS and
  Private Bytes (`PROCESS_MEMORY_COUNTERS_EX.PrivateUsage`) on Windows, with
  Working Set — Private named only to exclude it — and FR-039b names a relay.
  These are measurement and privacy mechanisms rather than
  technology choices, but they are API and infrastructure names, and the item
  cannot be marked complete while that question is open
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain
- [x] Requirements are testable and unambiguous
- [ ] Success criteria are measurable — four hardware-dependent criteria
  (SC-002, SC-004, SC-005, SC-006) cannot be reported as met until Q-E9a names
  the reference machines, and SC-013's reproducibility bar depends on the same
- [ ] Success criteria are technology-agnostic — SC-004 names platform memory
  counters and SC-009a names macOS 13.0; both are load-bearing for measurement
  and for the tier-2 floor respectively
- [x] All acceptance scenarios are defined
- [x] Edge cases are identified
- [x] Scope is clearly bounded
- [x] Dependencies and assumptions identified

## Feature Readiness

- [ ] All functional requirements have clear acceptance criteria — FR-003a,
  FR-007a, FR-014, FR-015a, FR-016a, FR-018a, FR-021, FR-023, FR-029a, FR-031, FR-039a,
  FR-018b, FR-019a, FR-039b, FR-039c, FR-039d, FR-039e, FR-039f, FR-042, FR-043 and
  FR-044 have
  neither a
  success criterion nor an acceptance scenario. This list is maintained with the
  requirements: a requirement added without a criterion is added here. Two earlier
  versions read as exhaustive and were not, and
  FR-041's accessibility and language obligations for the distribution page are
  covered by none (SC-009a covers only its operating-system statement and the
  installer refusal). An earlier version of this list named five and read as
  exhaustive
- [x] User scenarios cover primary flows
- [x] Feature meets measurable outcomes defined in Success Criteria
- [ ] No implementation details leak into specification — same question as the
  item above, and unticked for the same reason

## Notes

- Items marked incomplete require spec updates before `/speckit-clarify` or `/speckit-plan`.

### Re-validation after clarify and review, 2026-08-30

Six clarifications were integrated (see the spec's Clarifications section). An
earlier version of this section said five and reported settled-decision counts
that were wrong; it had been written before the sixth clarification landed, so
it never assessed the FR-029 / FR-029a split at all. Corrected below.

Two items improved materially rather than changing state:

- *Requirements are testable and unambiguous* was previously passing on the
  strength of wording. SC-011 was **untestable as written** — it required a
  retention figure the privacy posture made unmeasurable. Splitting it into two
  separately reported measurements (FR-039, FR-040) fixed the signed-in half.
  The signed-out half stayed untestable until review, because a cohort
  measure appeared to need a per-install key. FR-039a resolves it without one:
  the client evaluates its own retention locally and emits unlinkable reports
  carrying only a cohort week — an enrolment report, then either a retention or a
  withdrawal report — and the measure is the ratio of retention reports to
  enrolments net of withdrawals. Two earlier versions of this paragraph described
  designs the spec had already moved past, each stale in the round that wrote it;
  it is now checked against FR-039a whenever that requirement changes.
- *Scope is clearly bounded* is stronger now that the tier-2 operating-system
  floor is declared with both its consequences, the default search posture is
  fixed, and the site-credential limitation is stated rather than implied.

Adversarial review across four rounds then found items that were **not** passing,
contrary to the earlier claim that all still did. Rounds 3 and 4 additionally
carried four constitutional mandates that no requirement had held: Principle II's
budget file and per-change cost (FR-043), Principle IV's discoverable, opt-in and
no-injection mandates (FR-016a, FR-018a), Principle VI's *aggregate* condition
(FR-039d, which counts crash reports under a symbol-keyed key rather than
retaining a report — a retained exemplar would be the same per-install payload
under another name — and FR-039e, which sets the disclosure floor), Principle III's engine seam and no-bundled-engine rule (FR-044),
and Principle VII's BCP-47 keying and language/place
separation (FR-035). Three further gaps are recorded in issue #28 rather than
fixed here, because adding requirements reactively is what each round has been
correcting. Fixed in this branch: Principle VI's EU-hosting
mandate and crash reporting were absent; Principle II's requirement that every
named budget be CI-gated was contradicted by an ungated cold start; Principle
V's prohibition on client-built affiliate deeplinks had no requirement; Principle
VIII's brand seam had none either; SC-009 pointed at a section containing none of
what it claimed, giving it an empty population; and "credential" carried two
meanings that contradicted each other across FR-015a and FR-023.

Of the open decisions, Q-E2, Q-E4, Q-E6 and Q-E9 are **partly** settled — each
records what remains open, respectively the specific engine, the tier-2 keychain,
the rest of the diagnostic set, and the SC-002 figure. Q-E5 is settled as a
consequence of Q-E4 and Q-E8 is superseded by Q-E13. Q-E9a, Q-E11a, Q-E12, Q-E13
and Q-E14 were opened by review. An earlier version of this line called the first
group settled outright, which is the same error it was written to correct.

### Validation record

**No `[NEEDS CLARIFICATION]` markers are used.** The founder decisions are
recorded in the Open Decisions section instead, because the master prompt routes
them to `/speckit-clarify` and states they must be answered by the founder
rather than resolved by a specification. The figures they govern were all
placeholders originally; Success Criteria now marks each figure ratified or
provisional explicitly, and the gate exists either way, because Principle II
admits no un-gated budget.

**Implementation-detail review.** Engine, framework and language names were
deliberately kept out. Four success criteria reference a "system-provided web
runtime" as a measurement boundary rather than a technology choice: SC-001
excludes its bytes, SC-002 assumes it present, SC-003 covers its absence, and
SC-004 excludes its processes. Three further names entered during review and are
load-bearing rather than incidental: SC-004 names `phys_footprint` via
`task_vm_info` and Private Bytes (`PROCESS_MEMORY_COUNTERS_EX.PrivateUsage`), and
names Working Set — Private only to exclude it, because the two platforms expose
different quantities and an unnamed "memory" figure is not reproducible under
SC-013; SC-009a names macOS 13.0 because the tier-2 floor is the criterion; and
FR-039b names a relay because the unlinkability it requires cannot be met at the
application layer alone. Without that boundary the
figures are not measurable, since what is being budgeted is the part Evreos
ships. The engine decision itself lives in ADR-0001, not here.

**Platform tiering** appears in its own section rather than as a requirement,
because it bounds scope rather than describing behaviour.

**Known gap, deliberate.** SC-010, SC-011 and SC-012 are business outcomes
measured after release rather than at acceptance. They are retained because the master
prompt names them as success criteria and because omitting them would hide the
product's actual bar.
