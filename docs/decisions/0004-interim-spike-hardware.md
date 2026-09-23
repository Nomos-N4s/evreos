# Decision 0004: Interim hardware for the chrome spikes

- **Status**: Decided
- **Date**: 2026-09-05
- **Recorded**: 2026-09-05
- **Deciders**: Founder
- **Cite as**: `decisions/0004`

## Question

`specs/001-evreos-v1/tasks.md` orders the N6 accessibility measurement (T015)
and the S4 chrome-candidate comparison (T016) behind the reference machines
T013 procures: its dependency rule reads "T013 blocks every task measuring on a
pinned runner, including all four spikes." `docs/benchmarks/runner-procurement.md`
records the state of that procurement plainly: neither machine is procured, and
what is outstanding is a purchase, which cannot be made from inside this
repository. No purchase date exists. Followed strictly, that ordering blocks
ADR-0002, and behind it the application window (T038) and every chrome surface
in every story, for as long as the purchase takes.

Whether those two measurements may instead run now, on hardware that exists,
is not an implementer's call three times over. The ordering being modified sits
in a merged design document. The claim scope of evidence taken off the
reference class touches what a NON-NEGOTIABLE principle's release gate may
later rely on. And the hardware being named as the instrument is the founder's
own machine, which nobody else may volunteer.

What the specification itself requires is narrower than the ordering: SC-013
attaches to published benchmark methodology and figures — "a third party
rerunning them on a machine of the reference class Q-E9a names obtains the same
figures" — and T016 already classifies its own outputs as "indicative and not
as SC-006 or SC-004 measurements." Nothing in the constitution or the
specification requires a spike's evidence to be taken on a pinned runner; the
ordering rule is what requires it, and this record is the founder re-sequencing
that rule for two named tasks.

## Decision

T015's N6 measurement and T016's coarse candidate comparison may run on the
interim instrument named below, before the reference machines exist, under the
claim scope stated here. The pinned-runner runs are owed, not replaced: both
measurements re-run on the reference machines when procured, and ADR-0002
carries that as a reopen condition in the terms the Consequences section fixes.

### The interim tier-1 instrument

One machine, the founder's development laptop, pinned by tuple:

- **Machine**: Acer Predator PH315-53 (notebook)
- **CPU**: Intel Core i7-10750H, 6 cores, 10th generation
- **Memory**: 8 GB
- **Storage**: SSD (NVMe, behind the platform's RST controller)
- **Display**: internal 1920x1080 panel, 144 Hz native. Any latency sampling
  drives it at 60 Hz, and the measurement file records the driven rate — a
  faster panel flatters every input-to-repaint figure, which is the same reason
  the budget file's runner blocks carry a `display_refresh` field.
- **Operating system**: Windows 11 Pro, version 25H2, build 26200.9168 at the
  date of this record; the build at measurement time is recorded in the
  measurement file.
- **Web runtime**: WebView2 Runtime, evergreen channel, 152.0.4191.62 at the
  date of this record; the version at measurement time is recorded in the
  measurement file.
- **Screen readers**: Narrator as shipped with the operating-system build;
  NVDA at the version the measurement file records (not installed at the date
  of this record).

Against the tier-1 floor Q-E9a names — an 8th-generation Intel i3/i5 laptop
with 8 GB — this machine's memory sits exactly at the floor and its CPU is two
generations newer. Both facts are stated so no reader has to discover them.

This machine is an instrument, never a runner. It takes no `runner_label`, no
`[runners.*]` entry, and no field anywhere in `budgets.toml`; that file is
untouched by this decision. Nothing measured on it enters a budget entry as a
measurement, a baseline or a tolerance.

### Tier 2

No interim tier-2 machine exists. This decision pre-authorises a class rather
than a machine: a physical Mac at, or as near as obtainable to, the macOS 13
floor, pinned by the same tuple in the measurement record when one is used.
Screen-reader evidence taken over remote desktop is not acceptable — VoiceOver
drives the local audio and input paths. Until such a machine is used or the
tier-2 pinned runner arrives, tier-2 N6 is unmeasured, and every record that
cites an interim result says so.

### What the interim N6 result may claim

N6 asks whether a chrome node published through AccessKit and a content node
inside the platform webview can be joined — labelled across trees, focus
crossing in reading order — and what each screen reader announces at the
boundary. That answer is determined by software: the operating-system build,
UI Automation, the web-runtime version, the AccessKit adapter and the screen
reader, every one of which the tuple records. It does not turn on CPU vintage
or memory. Therefore:

- A **negative** result eliminates the drawn-chrome candidate on tier 1 with
  full force. An accessibility API that cannot express the join on this
  machine will not express it on the floor machine running the same evergreen
  runtime. The elimination stands unless the pinned-runner re-run contradicts
  it, which reopens ADR-0002.
- A **positive** result admits the candidate provisionally, for the recorded
  tuple. The transfer argument is that tier 1's web runtime is evergreen
  (ADR-0001): the pinned runner will run the same runtime or a newer one. It
  is still re-confirmed on the pinned runner, because release evidence under
  Principle X and SC-008 must be tier-pinned.
- Neither result says anything about tier 2, whose web runtime is frozen to
  the operating system (ADR-0001): evidence from any interim Mac transfers
  only to the OS version it was taken on, and the macOS 13 floor stays open
  until measured there.

### What the interim S4 comparison may claim

Exactly two things. The **relative ordering** of the surviving candidates
under the coarse method T016 states — the structural differences between the
candidates, such as one of them paying a message hop where another does not,
are expected to preserve their sign across hardware of this class. And
**gross disqualification** — a candidate that cannot meet a 16 ms
input-to-repaint sample on this machine, at 60 Hz, has no route to meeting it
on the floor, and is eliminated everywhere.

Not claimable, closed list:

- No figure from this machine is published under SC-013 or claimed to satisfy
  it.
- No figure enters `budgets.toml` — not as a measurement, a baseline, a
  tolerance, or a reset.
- Nothing measured here is SC-008 or Principle X release evidence.
- No gate's advisory status changes: `--allow-unpinned-runners` and
  `--allow-unmeasured` are retired by procurement and by measurement on the
  pinned runners, exactly as before, never by this record.
- That a candidate *fits* SC-006's 16 ms or SC-004's memory headroom on the
  reference class is not claimable from any interim figure — only ordering
  and gross disqualification are.
- Nothing about tier 2, beyond what a recorded interim Mac tuple itself
  establishes for its own OS version.

Both measurement files — `docs/measurements/n6-chrome-accessibility.md` and
`docs/measurements/s4-chrome-candidates.md` — open with a provenance block
naming this record, the instrument tuple as measured, and this claim scope.

## Evidence

- `docs/benchmarks/runner-procurement.md`, Status: "Neither machine is
  procured. The rig is not procured." — and the reason: the outstanding step
  is a purchase, which cannot be made from inside this repository.
- `specs/001-evreos-v1/research.md` §5.3: the mechanism N6 tests is
  architectural — AccessKit's multiple-tree support "does not consume a native
  WebView2 or WKWebView accessibility tree", "nodes cannot reference nodes in
  a different tree" — which is what makes the answer a property of the
  recorded software tuple rather than of the silicon underneath it. The same
  section records the open half: whether the OS composes the trees through the
  HWND-rooted UIA hierarchy is "plausible on Windows … but nothing located
  establishes it", which is why N6 is a measurement and not a finding.
- `docs/adr/0001-rendering-engine.md`, platform table: tier 1's WebView2 is
  evergreen; tier 2's WKWebView is frozen to the user's OS. That asymmetry is
  the whole of the transfer argument above, in both directions.
- `specs/001-evreos-v1/spec.md`, SC-013: what it governs is the published
  benchmark methodology and figures and their reproduction on the reference
  class. It is quoted in the Question; the interim figures stay outside it by
  the closed list above.
- `specs/001-evreos-v1/tasks.md`, T016: the comparison's own figures are
  "indicative and not as SC-006 or SC-004 measurements" as the task was
  merged — the interim route narrows where they are taken, not what they
  claim.

## Serves

- T015 and T016, whose amended text in `specs/001-evreos-v1/tasks.md` cites
  this record.
- Principle X's ordering evidence: N6 exists so the drawn-chrome candidate is
  admitted or eliminated before ADR-0002 selects, and this record is what lets
  that happen before procurement.
- ADR-0002, which will carry this record's reopen conditions verbatim.
- No budget entry. This record deliberately serves none, and the budget-file
  gate should never see `decisions/0004` in a `founder_decision` field.

## Consequences

Binding from the date above. T015 and T016 may take their measurements on the
interim instrument, under the claim scope stated, with the provenance block on
both measurement files. The pull request landing this record amends
`specs/001-evreos-v1/tasks.md` in the same change, so the ordering a reader
finds there and the ordering this record sets cannot disagree.

What is owed, from the same date:

- When the tier-1 reference machine is procured and pinned, T015 re-runs in
  full as originally written on it, and on tier 2 at the floor when that
  machine is pinned. Results are appended to the measurement file beside the
  interim section — records are append-only — never over it.
- The S4 coarse comparison re-runs on the tier-1 pinned runner for the
  selected candidate and any runner-up the ADR names.
- ADR-0002 carries both of these as reopen conditions, verbatim: "a
  pinned-runner re-run contradicting an interim result recorded under
  decisions/0004 reopens this decision", and "a tier-2 N6 result at the macOS
  floor failing the join for the selected candidate reopens this decision."

What reopens this record itself: the reference machines being procured before
the spikes run, at which point the interim route has no subject and the
original text of T015 and T016 applies unamended; or the instrument being
found to differ from the recorded tuple, at which point nothing measured on it
is covered by this record until a correction records the true tuple.

## Corrections

None yet.
