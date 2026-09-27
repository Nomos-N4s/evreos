# Decision 0009: A docstring says what the code does and cites why

- **Status**: Decided
- **Date**: 2026-09-27
- **Deciders**: Founder
- **Cite as**: `decisions/0009`

## Question

How much a docstring or code comment may say. The checks under
`scripts/checks/` carry module docstrings of a hundred lines and more that
restate the specification, argue each rule afresh, and state what a passing
check guarantees. New code copies that density. Every such sentence is a claim
a reviewer must check against another document, and it goes stale when either
side changes. Changing the practice changes how review rounds under the
Development Workflow are conducted, so an implementer may not decide it.

## Decision

A docstring or code comment states what the code does -- its inputs, outputs,
effects and limits -- briefly. The reason for a rule lives in the document that
makes it, the specification, a decision record or an ADR, and is cited by its
identifier, never paraphrased or argued afresh. No docstring claims a guarantee
beyond what the code it documents does.

The rule binds a file when a change edits it. Existing docstrings are brought
in line then, not in a sweep.

It is stated in the constitution's Development Workflow, and `CLAUDE.md`
points to it.

## Evidence

- Review round 1 on #106, recorded at
  https://github.com/Nomos-N4s/evreos/pull/106#issuecomment-5856938037,
  confirmed 29 findings. Seven were in the check's docstring rather than its
  behaviour: R1-8, R1-26, R1-27, R1-28, R1-29 and R1-32, and half of R1-17.
  Three of those misstated the specification or the code they paraphrased.
  Five more -- R1-2, R1-3, R1-4, R1-5 and R1-15 -- were gaps the docstring
  had promised to cover.
- The founder's direction on 2026-09-27, given in conversation while T061 was
  in progress, and recorded in Linear issue CAR-353 the same day.

## Serves

- The constitution's Development Workflow.
- `CLAUDE.md`'s Workflow section, which points to it.

## Consequences

Binding from the merge of the pull request that amends the constitution.
Nothing opened before that merge is covered, #106 included. The constitution
moves to 2.2.0.

What reopens this: a review finding a defect that a docstring's brevity hid.

## Corrections

None.
