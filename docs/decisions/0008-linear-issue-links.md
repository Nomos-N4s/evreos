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

- The founder's direction on 2026-09-27, given while T059 was in progress:
  pull requests link to Linear, and the GitHub-issue requirement is dropped by
  an amendment in a separate pull request.
- T059's pull request, #102, was opened linked to CAR-116 alone under that
  direction, and says so in its body. The GitHub issue opened for it earlier,
  #101, was closed as not planned.
- `AGENTS.md` already lists `Refs CAR-N` beside `Refs #N` as a commit's issue
  reference, and `scripts/check-commit-hygiene.py` accepts both.

## Serves

- The constitution's Development Workflow, its first bullet.
- `CLAUDE.md`'s Workflow section, which restates that bullet and the commit
  reference.

## Consequences

Binding from the date above. The constitution moves to 2.1.0, and `CLAUDE.md`,
the pull request template and `README.md` follow it.

What replaces the discipline this relaxes, as the amendment procedure asks:
nothing is withdrawn. Every pull request is still linked to one issue, and every
commit still references it, which the hygiene check enforces as before; the
issue may now be a Linear one. The pull request's link rests on review, as it
did before, because the check never read the body for it.

What reopens this: the work moving off Linear, or a Linear issue no longer
being readable by whoever reviews the pull request.

## Corrections

None.
