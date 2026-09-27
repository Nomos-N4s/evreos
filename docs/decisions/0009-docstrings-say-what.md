# Decision 0009: A docstring says what the code does and cites why

- **Status**: Decided
- **Date**: 2026-09-27
- **Deciders**: Founder
- **Cite as**: `decisions/0009`

## Question

How much a docstring or code comment may say. Three of the seven checks under
`scripts/checks/` carry module docstrings of a hundred lines or more, which
restate the specification and argue their rules afresh. New code copies that
density. Every such sentence is a claim
a reviewer must check against another document, and it goes stale when either
side changes. The rule belongs in the constitution, which only a recorded
founder decision can amend, so an implementer may not decide it.

## Decision

A docstring or code comment states what the code does -- its inputs, outputs,
effects and limits -- briefly. The reason for a rule lives in the document that
makes it -- the constitution, the specification or its plan and research, a
decision record, an ADR or `CLAUDE.md` -- and is cited by its identifier, never
paraphrased or argued afresh. No docstring or comment claims a guarantee
beyond what the code it documents does.

The rule binds a file when a pull request opened after the amendment merges
edits it. Existing docstrings are brought in line then, not in a sweep.

It is stated in the constitution's Development Workflow, and `CLAUDE.md`
points to it.

## Evidence

- Review round 1 on #106, recorded at
  https://github.com/Nomos-N4s/evreos/pull/106#issuecomment-5856938037,
  confirmed 29 findings, counting duplicates. Six were in the check's
  docstring rather than its behaviour -- R1-8, R1-26, R1-27, R1-28, R1-29 and
  R1-32 -- and R1-17 was partly so. Four of those -- R1-26, R1-27, R1-28 and
  R1-29 -- stated something false about the specification, the code or the
  platform. Seven more were gaps the docstring had promised to cover: R1-2,
  R1-3, R1-4, R1-5, R1-6, R1-14 (a duplicate of R1-2) and R1-15.
- The founder's direction on 2026-09-27, given in conversation while T061 was
  in progress, and recorded in Linear issue CAR-353 the same day.

## Serves

- The constitution's Development Workflow.
- `CLAUDE.md`'s Workflow section, which points to it.

## Consequences

Binding from the merge of the pull request that amends the constitution.
Nothing opened before that merge is covered, #106 included. The constitution
moves to 2.2.0, and five open tasks in `specs/001-evreos-v1/tasks.md` -- T112,
T114, T125, T137 and T161 -- follow it: the doc comments they prescribe cite a
reason rather than argue it.

What reopens this: a review finding a defect that a docstring's brevity hid.

## Corrections

None.
