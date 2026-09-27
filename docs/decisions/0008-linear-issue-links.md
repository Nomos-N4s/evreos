# Decision 0008: A pull request may be linked to its Linear issue alone

- **Status**: Decided
- **Date**: 2026-09-27
- **Deciders**: Founder
- **Cite as**: `decisions/0008`

## Question

The constitution's Development Workflow requires every change to reach `main`
through "a pull request linked to a GitHub issue", and `CLAUDE.md` restates it:
`Closes #N` in the body, with the issue opened first if none exists. The work
itself is planned and tracked in Linear: each task in
`specs/001-evreos-v1/tasks.md` has a Linear issue, and every commit already
references it (`Refs CAR-N`), which `scripts/check-commit-hygiene.py` accepts.

So each pull request needs a second issue, on GitHub, opened only to satisfy
the link and duplicating the Linear issue it restates. The rule sits in the
constitution, which only a recorded founder decision can amend, so an
implementer may not drop it.

## Decision

A pull request may be linked either to a GitHub issue or to the Linear issue
that tracks the work. A Linear-linked pull request names that issue in its body,
as `Refs CAR-N`, or `Closes CAR-N` where merging finishes it. Each commit
references the same issue, as it already may. A GitHub issue is not opened for
work Linear already tracks.

The link stays mandatory. What changes is which tracker may carry it.

## Evidence

- The founder's direction on 2026-09-27, given in conversation while T059 was
  in progress, and committed nowhere else: pull requests link to Linear, and
  the GitHub-issue requirement is dropped by an amendment in a separate pull
  request. The description of Linear issue CAR-351, created under the
  founder's Linear account the same day, records it.
- T059's pull request, #102, was opened linked to CAR-116 alone under that
  direction, before the rule changed, and says so in its body. The GitHub
  issue opened for it earlier, #101, was closed as not planned. The pull
  request carrying this amendment, #103, is linked to CAR-351 alone in the
  same way. Until the amendment merges, both fall short of the rule then in
  force, and both need the founder's override, stated on each before its
  merge, to land.
- `AGENTS.md` already lists `Refs CAR-N` beside `Refs #N` as a commit's issue
  reference, and `scripts/check-commit-hygiene.py` accepts both.

## Serves

- The constitution's Development Workflow, its first bullet.
- `CLAUDE.md`'s Workflow section, which restates that bullet and the commit
  reference.

## Consequences

Binding from the merge of the pull request that amends the constitution, not
from the date above; nothing opened before that merge is covered by it. The
constitution moves to 2.1.0, and `CLAUDE.md`,
the pull request template, `README.md` and Principle I's compliance statement
in `specs/001-evreos-v1/plan.md` follow it.

What replaces the discipline this relaxes, as the amendment procedure asks.
Every pull request is still linked to one issue, and every commit still
references it, which the hygiene check enforces as before; the issue may now be
a Linear one. The pull request's link rests on review, as it did before,
because the check never read the body for it. A Linear link gives up three
things a GitHub issue gave:

- GitHub's own link between the pull request and its issue. The Linear
  integration posts a link-back comment on the pull request instead, as it did
  on #102 and #103.
- The issue's closing when the pull request merges. `Closes CAR-N` leaves that
  to the Linear integration, and `Refs CAR-N` closes nothing, as it never did.
- The issue's being readable by anyone who can read the repository. The Linear
  workspace is private to the founder's team, so a reviewer outside it reads
  the pull request body, which states the issue's scope, and not the issue.

What reopens this: the work moving off Linear, or the repository gaining a
reviewer who must read the issues and cannot read the Linear workspace.

## Corrections

None.
