# Founder-decision register

This directory is the record of founder decisions: one file per decision,
numbered, with a citation form other files use. It exists because three rules
require a decision to be *recorded* and, until now, nothing said where.
Principle II of the constitution lets a budget move only by recorded founder
decision. The Success Criteria preamble of `specs/001-evreos-v1/spec.md` makes
an entry's status *ratified* only when a recorded founder decision sets it, and
has the budget-file gate fail an entry recorded ratified that names none. The
constitution's amendment procedure requires a recorded founder decision too.
`budgets.toml` carries thirteen ratified entries, each naming `decisions/0001`
in its `founder_decision` field, and the gate fails a ratified entry that names
none. This register is what those entries name.

## What is recorded here

A founder decision is a choice that no requirement, plan or measurement makes
for an implementer, and that the specification says MUST NOT be resolved
silently: its Open Decisions section routes such questions to clarify and
states that rule. The answer is recorded here when something else in the
repository has to cite it by a stable name — a budget entry's
`founder_decision`, a baseline reset, a requirement amendment, a task that
implements the choice.

Two kinds of decision are recorded elsewhere and are not duplicated here:

- **Architectural decisions** are ADRs in `docs/adr/`, as the constitution's
  Development Workflow requires. The two registers are distinct and number
  independently: `docs/adr/0001` and `decisions/0001` are different records. An
  ADR the founder takes is cross-referenced from the index below without
  consuming a number here.
- **A decision the specification says is recorded on a pull request** stays on
  that pull request. This register holds what has to be citable from a file.

A record never becomes the authority for a figure or a requirement. It cites
the passage of the specification that states it, and where a record and the
specification disagree, the specification governs and the record is corrected —
the specification is what `budgets.toml`'s figures are checked against.

## Numbering and citation

- Files are named `NNNN-slug.md`: four digits, then a lowercase hyphenated
  slug. Numbers start at `0001` and are contiguous.
- A number is claimed by the pull request that adds the file, is held by that
  decision alone, and is never reused — not after a decision is superseded, and
  not after one is withdrawn. Two files claiming one number is a defect in
  whichever landed second.
- The citation form is `decisions/NNNN` — the number and nothing else. That is
  the value `budgets.toml`'s `founder_decision` field carries and the form
  every other document uses. It omits the slug so that correcting a slug cannot
  break a citation, and it omits the path so that it reads the same from any
  file.
- An open decision takes a number when its question is recorded, so that the
  tasks which wait on it can cite it before it is answered.

## Form of a record

Every record carries the same sections, in this order, so that a reader knows
where to look and a check could one day parse it.

1. A title, `# Decision NNNN: <what was decided>`.
2. A metadata list: **Status** — `Open`, `Decided`, or `Superseded by
   decisions/NNNN`; **Date** — the day the decision was taken, or for an open
   record the day the question was recorded; **Recorded** — the day the file
   landed, where it differs; **Deciders** — Founder; **Cite as** — the citation
   form above.
3. **Question** — what had to be decided, and why an implementer may not decide
   it: the requirement that leaves it open, or the principle that reserves it.
4. **Decision** — what was decided, in terms a reader can check against the
   file it governs. An open record says the decision is not yet taken and
   states the options honestly: one rejected candidate is recorded as one, not
   padded to a list.
5. **Evidence** — what the decision rests on, cited by file and section, and
   measurements by the committed record that holds them. Where there is none
   yet, the record says so; a decision taken ahead of measurement is recorded
   as that, because it is what makes a figure a ceiling rather than a
   prediction.
6. **Serves** — the requirement, criterion or budget entries the decision
   serves. For a budget entry: criterion, entry name, platform and figure with
   its unit, so that the budget file can be checked against the record line by
   line.
7. **Consequences** — what the decision binds from its date, and what result
   would reopen it.
8. **Corrections** — dated entries, appended and never edited, each saying what
   it corrected and why.

A decision, once landed, is not edited. A supporting claim found wrong is
corrected by an appended entry in the record's Corrections section that leaves
the decision text as it was, on the practice `docs/adr/0001-rendering-engine.md`
already follows. A decision that changes is a new record, and the only edit the
old record then receives is its Status line, pointing at the successor. A
ratified budget figure may afterwards only be tightened: the tightening is a
new record, superseding the old one for that entry alone, and the entry's
`founder_decision` moves to the new number. Relaxing a ratified figure is an
amendment to the specification on the terms the Success Criteria preamble sets,
and the amendment cites the record that carries the founder's reasons.

## Index

| Number | Decision | Status | Date | Serves |
| --- | --- | --- | --- | --- |
| [0001](0001-ratified-budget-figures.md) | Ratified budget figures | Decided | 2026-08-30 | Principle II; SC-001, SC-004 on tier 1, SC-005, SC-006 — thirteen budget entries |

The next free number is the one after the last row.

### Architectural decisions the founder took

Recorded as ADRs, cited by their own numbers, and holding no number here.

- `docs/adr/0001-rendering-engine.md` — host web content in operating-system
  webviews.
