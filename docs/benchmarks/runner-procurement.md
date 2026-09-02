# Reference machine procurement

The two machines every figure in `budgets.toml` is measured on, the rule that
picks them, and what has to happen when one is replaced. Nothing here is
optional: a figure measured on an unnamed machine is not reproducible under
SC-013, so it cannot be what a build fails on, and until both machines are
pinned the absolute gates run advisory behind `--allow-unpinned-runners`.

## The rule

**Each tier's reference machine is the oldest configuration that tier's
declared operating-system floor admits, at 8 GB of memory.** Q-E9a settled this
on 2026-08-30 and it is recorded as `decisions/0001`'s neighbour in
`budgets.toml`. The rule binds; the models below follow from it and change when
the rule's inputs change, never independently.

The rule is what makes the budgets mean something. Evreos' first cohort arrives
on hardware they already own, bought years before, not on a current laptop. A
budget met on a current machine and missed on a six-year-old one is a budget
that does not describe the product anyone actually runs.

## The two machines

| | Tier 1 | Tier 2 |
|---|---|---|
| Platform | Windows | macOS |
| Floor | Windows 11 | macOS 13 |
| Model | 8th-generation Intel i3/i5 laptop, 8 GB | MacBook Pro (2017), 8 GB |
| Why this one | The oldest processor generation Windows 11's published requirements admit, in the cheapest laptop tier | The oldest portable macOS 13 admits, excluding the fanless 12-inch MacBook, whose throttling under sustained load moves a measurement by more than the 5% tolerance cap allows |
| Cold spare | One, same specification | One, same specification |

A cold spare per tier exists because a dead reference machine otherwise stops
every measurement in the project until a replacement is sourced and prepared.
The spare is bought with the primary, not after the primary fails.

## What is recorded in `budgets.toml`

Each tier's `[runners.*]` block carries, as it arrives:

- `platform` and `os_floor` — the rule's inputs
- `model` — the machine the rule selected
- `os_version` and `os_build` — the exact release measured on, because an
  operating-system update can move a figure on its own
- `memory` and `storage` — the configuration, since the rule fixes memory at
  8 GB and storage type moves start-up figures
- `display_refresh` — the rate the panel is driven at, because SC-006's
  interaction figures are bounded below by a frame interval
- `runner_label` — what a workflow's `runs-on` resolves to for that tier
- `identity` — the durable machine identifier

All of `display_refresh`, `runner_label` and `identity` are empty or zero until
the machine arrives. The budget-file gate fails on an unpinned tier for exactly
that reason, which is what bounds the advisory period on the absolute gates
rather than leaving it to good intentions.

## Swapping a machine

A swap is not a maintenance detail. It is a change to what every figure on that
tier means.

1. Prepare the replacement from the image-preparation script, not by hand.
2. Set the tier's `identity`, `runner_label`, `os_version`, `os_build` and
   `display_refresh` to the new machine's in one commit.
3. **Reset every baseline on that tier to `0.0` in the same commit.** A durable
   identity changing restarts that tier's baseline series: a baseline is a
   figure measured on one machine, and comparing a new machine's measurement
   against an old machine's baseline is a regression gate firing on a hardware
   difference. The regression half of an entry is inert while its baseline is
   not positive, so the series restarts cleanly rather than reporting noise.
4. Record the swap and its date here, with what was replaced and why.
5. Re-measure. The first measurement after a swap writes the new baseline.

Using the cold spare is a swap. It changes the durable identity, so it restarts
the baseline series exactly as a new purchase does. The spare is insurance
against downtime, not against re-measurement.

## What retires the two CI flags

`.github/workflows/build.yml` runs the budget gate twice: once informational,
once blocking with two deferrals named in the workflow beside what satisfies
each.

- **`--allow-unpinned-runners`** is retired by this task's other half: both
  tiers pinned, with a durable identity and a runner label recorded. It is
  **not** retired here, because neither machine has been bought. Removing it
  while the identities are empty would fail every build for a reason the
  repository cannot fix.
- **`--allow-unmeasured`** is retired when every one of the eighteen entries
  carries a measurement for its platform, which needs the measuring harnesses
  and each platform's installer artefact. The release-gate task removes it.

Neither flag is removed by a change that does not satisfy it. A flag removed
early is a red build; a flag left after its condition is met is an unnoticed
hole, which is why each is named in the workflow beside its retiring condition.

## The SC-006 latency rig

SC-006 caps a tab switch and an address-field keystroke at 16 ms at the 99th
percentile. Measuring that honestly means timing an injected input to light
leaving the panel. A software present-to-display proxy is what CI can run, and
a proxy has a bias: it reports the compositor's present call, not the photon.

**The rig is a photodiode and a capture device** able to time an injected input
to light leaving the panel, built from a published design so that a third party
can rebuild it and reproduce the figure. It is the one instrument the software
proxy is characterised against, and nothing else in the plan buys it. Record in
each tier's runner block, as it arrives: the photodiode and capture device by
make and model, and the published design the rig is built from.

The rig is bought once and used on both tiers. It characterises the proxy's
bias once per tier, and that delta is appended to the benchmark methodology
rather than folded silently into a figure.

**Where the rig cannot be obtained**, that is recorded here as a stated
limitation of the SC-006 figures, and the latency measurement publishes its
proxy as a proxy with its bias unbounded. It is never published as photon-out.
A figure whose instrument is unstated is worse than an absent one: an absent
figure blocks the gate, and an overstated one passes it.

## Status

**Neither machine is procured. The rig is not procured.** Every runner field
that depends on hardware is empty, the budget-file gate fails on both tiers,
and the absolute gates are advisory behind `--allow-unpinned-runners`. What is
outstanding is a purchase, which cannot be made from inside this repository.
