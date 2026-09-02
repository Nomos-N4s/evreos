# Decision 0001: Ratified budget figures

- **Status**: Decided
- **Date**: 2026-08-30
- **Recorded**: 2026-09-02
- **Deciders**: Founder
- **Cite as**: `decisions/0001`

## Question

Should the placeholder performance budgets be ratified now, or replaced after
measurement? The specification carries this as Q-E9.

It is the founder's to answer and not an implementer's. Principle II lets a
budget move only by recorded founder decision and makes tighter the default
direction. The Success Criteria preamble of `specs/001-evreos-v1/spec.md`
defines an entry's status as *ratified* only when a recorded founder decision
sets it, and *provisional* until such a decision replaces it, so no status in
`budgets.toml` can be set by the change that writes the number.

## Decision

Ratify now, as tighten-only figures, the size, memory, idle and interaction
budgets: SC-001's four entries, SC-005's four, SC-006's four, and SC-004 on
tier 1 — thirteen of the eighteen entries the preamble closes over. Hold the
other five provisional: SC-002's four entries, warm start and cold start on each
platform, until the cold-start spike measures engine initialisation on the
reference machine for each tier; and SC-004 on tier 2, until the spike into
what governs macOS memory at ten tabs reports.

The answer was given in two steps on the same day, and this record follows the
later one. The clarify answer ratified the memory budget without distinguishing
the tiers. The Open Decisions entry for Q-E9 then held SC-004's tier-2 entry
provisional on the evidence ADR-0001 records, and the specification states that
where the two differ Q-E9's Open Decisions entry governs, as the later record.

### The thirteen ratified entries

Figures and conditions as the criteria state them; the entry names are the ones
`budgets.toml` uses.

| Criterion | Entry | Platform | Figure |
| --- | --- | --- | --- |
| SC-001 | download size | windows | 20 MB — the installer artefact CI publishes |
| SC-001 | download size | macos | 20 MB — the same |
| SC-001 | installed footprint | windows | 60 MB — disk delta after first run completes, excluding member data |
| SC-001 | installed footprint | macos | 60 MB — the same |
| SC-004 | ten-tab memory | windows | 150 MB at every 5-second sample, over a soak of at least 8 hours |
| SC-005 | 60-minute window | windows | 0.5 percent of one core across at least 60 minutes — 18 s of processor time |
| SC-005 | 60-minute window | macos | the same |
| SC-005 | wake-free 1-second sample | windows | 0.5 percent of one core in every 1-second sample with no enumerated wake — 5 ms of processor time |
| SC-005 | wake-free 1-second sample | macos | the same |
| SC-006 | tab switch | windows | 16 ms at the 99th percentile of at least 1000 trials, no trial over 16 ms, on a display driven at 60 Hz |
| SC-006 | tab switch | macos | the same |
| SC-006 | address-field keystroke | windows | the same |
| SC-006 | address-field keystroke | macos | the same |

Each is a ceiling from 2026-08-30 and may afterwards only be tightened.

### The five provisional entries

| Criterion | Entry | Platform | Figure | Awaits |
| --- | --- | --- | --- | --- |
| SC-002 | warm start | windows | 800 ms | the cold-start spike |
| SC-002 | warm start | macos | 800 ms | the cold-start spike |
| SC-002 | cold start | windows | 2000 ms | the cold-start spike |
| SC-002 | cold start | macos | 2000 ms | the cold-start spike |
| SC-004 | ten-tab memory | macos | 150 MB | the macOS-memory-at-ten-tabs spike |

A provisional figure is a ceiling for as long as it stands and binds a baseline
exactly as a ratified one does; provisional describes the figure, never whether
a gate exists. Each may be replaced once, by a founder decision taken on the
spike's committed result, and is ratified and tighten-only from the moment that
decision lands. That decision is a new record in this register, and its number
is what the entry then carries as `founder_decision`. Until then a provisional
entry names no decision, which the budget-file gate permits: the clause it
enforces is that a *ratified* entry names one.

## Evidence

The decision rests on the following, cited so that each can be re-read rather
than trusted.

- `specs/001-evreos-v1/spec.md`, Clarifications, the 2026-08-30 entry for Q-E9:
  the founder's answer, and the note that the answer ratified memory without
  distinguishing the tiers.
- The same file, Open Decisions, Q-E9: the narrowing to SC-004 on tier 1, the
  count — three figures, five entries — and the statement that this entry
  governs where the two differ.
- The same file, Success Criteria preamble: the definition of an entry and its
  status; the closed list of eighteen; the rule that a ratified figure may
  afterwards only be tightened, and that a provisional one may be replaced once
  on spike evidence.
- The same file, the criteria themselves: SC-001, all four entries ratified;
  SC-002, all four provisional because a large share of each figure is the
  engine's own initialisation rather than Evreos's code; SC-004, ratified on
  tier 1 and provisional on tier 2 because what governs macOS memory at ten
  tabs is unestablished; SC-005 and SC-006, ratified.
- `docs/adr/0001-rendering-engine.md`, rationale 2, for the arithmetic that
  makes the size and memory figures plausible as ceilings: bundled Chromium
  adds 80–120 MB before any product code, Microsoft's own documentation puts
  the fixed-version WebView2 runtime at over 250 MB, and an empty Electron
  application boots at 150–200 MB of resident memory, which is the entire
  shell-overhead budget at zero tabs. Its risk 9 is why SC-004's tier-2 entry
  is held: sharing a process pool is a documented no-op on macOS, so nothing
  yet accounts for memory at ten tabs there. Its evidence status says of those
  figures that they are sound enough to eliminate options by an order of
  magnitude and are not budgets.
- `specs/001-evreos-v1/research.md`, §12.1, for the two spikes by their method:
  two builds differing only at the engine seam, rebooted before each cold
  trial, for SC-002; process count and footprint at ten tabs under a shared and
  a per-view configuration on the tier-2 runner, for SC-004 on tier 2.

What the evidence is not. No entry has been measured on the reference machine
for its tier: neither runner is procured, and `budgets.toml` records both
identities as empty. The thirteen figures are the specification input's
placeholders, ratified as ceilings by decision and not confirmed by
measurement. That is what *ratified* means in the preamble and it is what makes
each a gate rather than a prediction; it is stated here so that no later
document cites this record as though it were a measurement.

One risk to the ratification is known and is recorded rather than left for a
harness to find. SC-005 bounds processor use over the same process set SC-004
counts — the engine's own host, content, network and GPU processes, whose idle
timers Evreos does not author — and that engine's idle floor has never been
measured. `specs/001-evreos-v1/plan.md`, Complexity Tracking, and
`specs/001-evreos-v1/research.md`, §12.2, carry the measurement as N1, taken on
each tier's runner before SC-005 is treated as achievable. If the floor exceeds
5 ms in a wake-free 1-second sample or 18 s across a 60-minute window, SC-005
is unmeetable as ratified, and the remedy is an amendment to the specification
recording the founder decision, the measured evidence and what discipline
replaces the budget removed — a new record here, never a code change and never
an edit to this one.

## Serves

- Principle II of the constitution: budgets in one file, gated in CI, moved
  only by recorded founder decision, tighter by default.
- FR-043, which names `budgets.toml` as that file.
- The Success Criteria preamble's budget-file gate, which fails an entry
  recorded ratified that names no founder decision. The thirteen entries above
  cite this record as `founder_decision`. At the time of writing the file
  carries SC-001's four entries, ratified, and no `founder_decision` field; the
  schema extension and the completion of the file to eighteen entries — tasks
  T005 and T009 in `specs/001-evreos-v1/tasks.md` — write `decisions/0001` on
  every ratified entry, and the gate's missing clause, T010, is what then fails
  a ratified entry without it.
- Q-E9 in the specification's Open Decisions, which this record gives a citable
  form and does not restate.

## Consequences

- From 2026-08-30 each of the thirteen figures may only be tightened. A
  tightening is a new record in this register superseding this one for that
  entry alone, and that entry's `founder_decision` moves to the new number; the
  other twelve keep citing this record. Relaxing one is an amendment to the
  specification recording the founder decision, the measured evidence and what
  discipline replaces the budget removed.
- The five provisional entries carry no `founder_decision` until the decision
  that replaces each lands on its spike's committed result.
- What reopens this record for an entry is a measured floor above its figure on
  that tier's pinned runner: N1's engine idle floor against SC-005, or the spike
  results against the provisional figures they settle. Either lands as an
  amendment, not as an edit here.

## Corrections

None.
