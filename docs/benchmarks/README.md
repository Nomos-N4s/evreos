# Benchmarks

This directory holds what the benchmark runners need that is not code: the
workflow convention below and, as the work that needs them lands, the
procurement record for the two reference machines, the image-preparation
procedures a measurement condition depends on, and the methodology SC-013
publishes. The harnesses and scripts live where their tasks put them; this
directory says how they get onto a pinned machine.

## The rule: a job that needs a pinned machine lands in benchmarks.yml

Every job that needs a pinned machine lands in
`.github/workflows/benchmarks.yml`, and never in `.github/workflows/build.yml`.

The reason is where each workflow runs. `build.yml` runs on GitHub's hosted
runners, and the Success Criteria preamble of the specification disqualifies
those for measurement: a fungible hosted machine is not a pinned runner, because
"neighbour noise on shared cloud machines moves memory and latency by more than
a real regression does." A figure produced on one cannot be recorded in
`budgets.toml`, published under SC-013, used to reset a baseline, or reported as
met on reference hardware. It is not an approximate measurement; it is not a
measurement. So a hardware-dependent job in `build.yml` would run on every pull
request, cost minutes, and produce a number nobody may use.

The converse holds too. A job that needs no pinned machine — a build, a test, a
check over files in the repository — does not land here. The jobs here wait on
a machine that exists once per tier and are restricted to pull requests from
this repository; a job with no reason to wait or to be restricted would be
slowed and narrowed for nothing. The build, the tests and the budget gate's
hosted half run in `build.yml`.

`scripts/check-budgets.py` already draws this line on the machine it runs on:
it measures only what it can measure honestly there — SC-001's size entries,
which are build output — and reports every hardware-dependent entry as
unmeasured rather than producing a number. This directory is the other half of
that line. The hardware-dependent entries are measured here, on the machine the
budget file names.

## Registering a job

Every later task that needs a pinned machine registers a job in
`benchmarks.yml`; none authors a workflow of its own. One workflow is the one
place where the trigger set, the runner resolution and the fork restriction are
stated, and a second workflow would be a second copy of each, free to drift.

A registered job follows three conventions.

### One job per tier

A figure is reported against its tier's pinned runner and against no other
machine, so a harness that runs on both tiers registers two jobs, one per tier,
each producing figures only for its tier's entries. A tier-1 job never produces
a tier-2 figure.

### The runner is resolved from the budget file, never named

A job never writes a machine or a label into `runs-on`. It resolves its runner
from the `runner_label` field the budget file records in that tier's
`[runners.tier1]` or `[runners.tier2]` block.

`budgets.toml` is already the one place that records which machine a figure is
measured on — model, operating-system version, memory and a durable machine
identifier, which is what SC-013's run record carries. A label in a workflow
would be a second record of the same fact. The two would agree until the day a
machine is swapped: the durable identity changes, every baseline series on that
tier restarts, and a label kept in the workflow would have to change in a
second file in the same commit or the job would measure on a machine the budget
file does not name. Resolving the label from the file rules that out by
construction.

A job's `runs-on` is decided before the job runs, so no job can read the file
for itself. One job on a hosted runner reads `budgets.toml` and exposes one
output per tier; every hardware-dependent job `needs` it and takes its runner
from the output. This is the shape:

```yaml
jobs:
  runner-labels:
    name: Resolve runner labels from budgets.toml
    runs-on: ubuntu-latest
    outputs:
      tier1: ${{ steps.read.outputs.tier1 }}
      tier2: ${{ steps.read.outputs.tier2 }}
    steps:
      - uses: actions/checkout@v4
      - id: read
        run: |
          python3 - >> "$GITHUB_OUTPUT" <<'PY'
          import tomllib
          with open("budgets.toml", "rb") as handle:
              runners = tomllib.load(handle)["runners"]
          for tier, runner in runners.items():
              print(f"{tier}={runner.get('runner_label', '')}")
          PY

  <harness>-tier1:
    needs: runner-labels
    # Skipped, not failed, while the tier is unpinned. The budget-file gate is
    # what reports an unpinned tier; a job failing for the same cause on every
    # pull request would say it twice, and would stay red for the whole time
    # between the first machine arriving and the second.
    if: needs.runner-labels.outputs.tier1 != ''
    runs-on: ${{ needs.runner-labels.outputs.tier1 }}
    steps:
      ...
```

The resolver passes an empty label through rather than failing on it, and the
job that consumes it skips, for the reason the comment gives: the two machines
arrive one at a time, and a tier-2 job that fails on every pull request because
tier 2 is not yet pinned reports nothing the budget-file gate has not already
reported. A skipped job is visible on the pull request as skipped; a red one
for a known cause is noise that hides the next real failure.

### Restricted to pull requests from this repository

Hardware-dependent jobs run on self-hosted machines, and this repository is
public. GitHub's own guidance is that self-hosted runners should not be used
with public repositories, because anyone who can fork the repository and open a
pull request can execute code on the runner — and here the runner is also the
machine holding a tier's baseline series. So every hardware-dependent job is
restricted to pull requests whose head is in this repository, and to
`workflow_dispatch`, which only someone with write access can start. The
procurement task adds the restriction when it pins the first machine, and every
job registered after it carries it. The guard job is exempt: it runs on a
hosted machine and does nothing.

## What is here today

One job, `guard`, a no-op on a hosted runner whose passing proves the file
parses. No hardware-dependent job is registered, because none can be:
`budgets.toml` carries a `runner_label` field on both tiers, added by the
schema task, but both are empty until each machine is procured — so there is no
machine to bind a job to, and the resolver above is not in the workflow yet
either,
because a resolver with nothing to resolve is a job that runs on every pull
request for no one. It lands with the first job that needs it, in the shape
above. The two runners themselves are not procured; the `identity` fields in
`budgets.toml` are empty, and `build.yml` says so at its
`--allow-unpinned-runners` flag, which the procurement task retires.

The workflow declares `pull_request` and `workflow_dispatch`. `pull_request`
because the regression gate compares every change against its baseline, and a
gate blocks a merge on the pull request, not after it. `workflow_dispatch` so a
run can be started by hand — to re-measure after an engine runtime update,
which can move every hardware-dependent figure with no change to Evreos, since
ADR-0001 commits tier 1 to the evergreen WebView2 runtime and tier 2 to the
WebKit the operating system ships — or to exercise a freshly pinned machine
before a job is bound to it.
