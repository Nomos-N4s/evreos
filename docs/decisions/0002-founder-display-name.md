# Decision 0002: The founder's forge display name is an accepted author

- **Status**: Decided
- **Date**: 2026-09-01
- **Recorded**: 2026-09-02
- **Deciders**: Founder
- **Cite as**: `decisions/0002`

## Question

Principle I names one author: `xcoder-es <capintobe@gmail.com>`. The forge
account the founder commits and merges from carries the display name `Carlos
Pinto`, so every merge the forge creates is authored `Carlos Pinto
<capintobe@gmail.com>` — the same person and the same address under a different
name.

Nine merge commits on `main` are authored that way. The constitution's Sync
Impact Report, `CLAUDE.md` and `docs/governance/branch-protection.md` each
recorded this as history not meeting Principle I, with the remedy given as
changing the forge account's display name to `xcoder-es`.

Whether to change the account or to accept the name is the founder's, not an
implementer's: it is a question about who the author is, which is the fact
Principle I exists to fix.

## Decision

**Accept `Carlos Pinto <capintobe@gmail.com>` as an author and a committer
alongside `xcoder-es <capintobe@gmail.com>`.** The display name is not changed.

The two names denote one person, identified by one address that Principle I
already names. Nothing about sole authorship changes: the set has two spellings
of one identity and no second author.

The address is what binds. A commit authored `Carlos Pinto` under any other
address is refused exactly as before, and so is `xcoder-es` under another
address. The set is closed at these two entries; adding a third is a new
decision, not a maintenance edit.

## Evidence

- `.specify/memory/constitution.md` Principle I names the author.
- The Sync Impact Report of the same file records the nine merge commits and
  proposes the account rename as the remedy.
- `scripts/check-commit-hygiene.py` compares the author string exactly, so a
  display name is a distinct author to it however the address matches.

No measurement bears on this. It is a decision about identity, taken with the
history in hand.

## Serves

- Principle I, by fixing which author strings satisfy it.
- `scripts/check-commit-hygiene.py`'s `FOUNDER_AUTHORS` and
  `ALLOWED_COMMITTERS`, which cite this record.
- The nine merge commits on `main`, which this decision brings into compliance
  rather than leaving as a standing exception.

## Consequences

Binding from the date above. The forge account keeps its display name, and the
rename recorded elsewhere as the remedy is not carried out.

What reopens this: a second person committing, at which point the address stops
identifying one author and the set has to be reconsidered from Principle I
rather than extended.

Three documents recorded the previous position and are corrected in the change
that lands this record: the constitution's Sync Impact Report, the Authorship
section of `CLAUDE.md`, and `docs/governance/branch-protection.md`. Each now
cites this decision rather than describing history as non-compliant.

## Corrections

None.
