# Specification Quality Checklist: Evreos v1

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-08-30
**Feature**: [spec.md](../spec.md)

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain
- [x] Requirements are testable and unambiguous
- [x] Success criteria are measurable
- [x] Success criteria are technology-agnostic (no implementation details)
- [x] All acceptance scenarios are defined
- [x] Edge cases are identified
- [x] Scope is clearly bounded
- [x] Dependencies and assumptions identified

## Feature Readiness

- [x] All functional requirements have clear acceptance criteria
- [x] User scenarios cover primary flows
- [x] Feature meets measurable outcomes defined in Success Criteria
- [x] No implementation details leak into specification

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
  The signed-out half stayed untestable until review, because a cohort measure
  cannot be computed from counts carrying no per-install key; FR-039a now
  authorises a bounded, non-fingerprinting install identifier.
- *Scope is clearly bounded* is stronger now that the tier-2 operating-system
  floor is declared with both its consequences, the default search posture is
  fixed, and the site-credential limitation is stated rather than implied.

Adversarial review then found items that were **not** passing, contrary to the
earlier claim that all still did. Fixed in this branch: Principle VI's EU-hosting
mandate and crash reporting were absent; Principle II's requirement that every
named budget be CI-gated was contradicted by an ungated cold start; Principle
V's prohibition on client-built affiliate deeplinks had no requirement; Principle
VIII's brand seam had none either; SC-009 pointed at a section containing none of
what it claimed, giving it an empty population; and "credential" carried two
meanings that contradicted each other across FR-015a and FR-023.

Of the open decisions, Q-E2, Q-E4 and Q-E9 are settled, Q-E5 is settled as a
consequence of Q-E4, Q-E6 and Q-E8 are partly settled or narrowed, and Q-E9a,
Q-E11a, Q-E12 and Q-E13 were opened by review. The remainder are recorded in
Open Decisions with what specifically stays open.

### Validation record

**No `[NEEDS CLARIFICATION]` markers are used.** The founder decisions are
recorded in the Open Decisions section instead, because the master prompt routes
them to `/speckit-clarify` and states they must be answered by the founder
rather than resolved by a specification. The figures they govern were all
placeholders originally; Success Criteria now marks each figure ratified or
provisional explicitly, and the gate exists either way, because Principle II
admits no un-gated budget.

**Implementation-detail review.** Engine, framework and language names were
deliberately kept out. Two success criteria reference a "system-provided web
runtime" as a measurement boundary rather than a technology choice: SC-001
excludes its bytes and SC-004 excludes its processes. Without that boundary the
figures are not measurable, since what is being budgeted is the part Evreos
ships. The engine decision itself lives in ADR-0001, not here.

**Platform tiering** appears in its own section rather than as a requirement,
because it bounds scope rather than describing behaviour.

**Known gap, deliberate.** SC-010, SC-011 and SC-012 are business outcomes
measured after release rather than at acceptance. They are retained because the master
prompt names them as success criteria and because omitting them would hide the
product's actual bar.
