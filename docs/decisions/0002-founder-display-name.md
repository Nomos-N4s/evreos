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

**2026-09-02** — Three claims this record makes about its own surroundings were
wrong, found by adversarial review of the change that landed it. The decision
itself is unaffected; all three are about what else the repository said.

- The Question says the constitution's Sync Impact Report, `CLAUDE.md` and
  `docs/governance/branch-protection.md` each gave the remedy as changing the
  forge account's display name. Two did. `branch-protection.md` named the
  display name as an author the rule forbids and proposed nothing, so it is
  corrected here rather than being an instance of what the sentence describes.
- The Consequences say three documents recorded the previous position and are
  corrected in the change that lands this record. There were four. The
  Constitution Check and the Complexity Tracking row of
  `specs/001-evreos-v1/plan.md` carried it too and were missed; both are
  corrected in the same pull request, one commit later.
- The Serves entry names `FOUNDER_AUTHORS` and `ALLOWED_COMMITTERS` as citing
  this record. Only the first did. `ALLOWED_COMMITTERS` derives its founder half
  from `FOUNDER_AUTHORS` and is served by this decision, so the entry is right
  about what the decision reaches and was wrong only about the citation. The
  citation is added rather than the claim withdrawn.

**2026-09-02** — This record says "nine merge commits on `main`" in three places.
The number was right when it was first written into the constitution's follow-up
list on 2026-08-30 and had drifted to sixteen by the day this record landed; it
goes on drifting with every merge, so it was wrong here on arrival and would be
wrong again by any date it was corrected to.

The decision does not turn on it. What it fixes is which author strings satisfy
Principle I, and that reaches every commit carrying one of them, past and future
— a set, not a list. The count was scene-setting that read as a scope.

The three documents that carried it alongside this one drop the number in the
change appending this entry, keeping the description that stays true. The number
is not restated here with a date, because a dated count in a record nobody
recomputes is the same trap one turn later; `git log origin/main --merges
--format='%an'` answers it whenever the answer is wanted.

**2026-09-02** — The entry above says the three other documents drop the number
in the change that appends it. `specs/001-evreos-v1/plan.md` carried it in TWO
places -- the Constitution Check narrative and the Complexity Tracking row --
and that change edited only the row, so the narrative kept the count and this
record was wrong about the repository from the moment it landed.

Which is the failure the entry above was written to end, one turn later and in
the entry itself. Correcting a claim in a record is worth nothing if the
correction is not checked the same way the original should have been: `grep -rn`
for the stale text across the tree, not a memory of which files were edited.
The narrative is corrected in the change appending this entry.
