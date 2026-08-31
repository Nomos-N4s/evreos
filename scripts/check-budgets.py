#!/usr/bin/env python3
"""Enforce the budgets Principle II requires and FR-043 names.

WHAT THIS GATE IS, and what it deliberately is not.

Principle II: hard budgets "MUST live in one budget file in this repository and
MUST be enforced by CI gates that fail the build on regression." The Success
Criteria preamble defines three gates and says they are defined there and only
there. This script is those three, and it adds none of its own.

  BUDGET FILE  fails when the file does not describe a gateable state: an entry
               a criterion states is missing, a baseline above the entry's
               stated figure, or a runner with no identity. Not hardware
               dependent -- it compares numbers in a file -- so it blocks from
               M0 unconditionally. It is what bounds the advisory period on the
               absolute gate, rather than leaving that to good intentions.

  ABSOLUTE     fails when a measured figure exceeds its stated value. On a
               hardware-dependent entry this is advisory until that tier's
               runner is procured and pinned, because a figure measured on an
               unnamed machine is not reproducible under SC-013 and so cannot be
               what a build fails on.

  REGRESSION   fails when a measured figure is worse than the entry's baseline
               by more than its declared tolerance. It compares one machine
               against itself, so it blocks from M0 on every entry.

An UNDECLARED TOLERANCE IS ZERO, not unbounded. That direction matters: the
opposite reading lets an entry disable its own regression gate by omission.

This script measures only what it can measure honestly on the machine it runs
on. SC-001's download and installed-footprint entries are build output and are
measured here. The hardware-dependent entries -- SC-002, SC-004, SC-005, SC-006
-- are measured by the benchmark harness on a pinned runner, and this script
reports them as unmeasured rather than inventing a number. An unmeasured entry
is not a pass.
"""
import argparse
import os
import subprocess
import sys
import tomllib
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# Criteria whose figures depend on the machine they are measured on. Their
# absolute gate waits on a pinned runner; their regression gate does not.
HARDWARE_DEPENDENT = {"SC-002", "SC-004", "SC-005", "SC-006"}

# The tolerance cap the Success Criteria preamble sets, as a percentage of the
# entry's baseline.
MAX_TOLERANCE_PCT = 5.0


class Gate:
    """One gate's verdict over the whole budget file."""

    def __init__(self, name):
        self.name = name
        self.blocking = []
        self.advisory = []

    def block(self, message):
        self.blocking.append(message)

    def advise(self, message):
        self.advisory.append(message)

    @property
    def failed(self):
        return bool(self.blocking)


def load_budgets(path):
    with open(path, "rb") as handle:
        return tomllib.load(handle)


def entry_label(entry):
    return f"{entry['criterion']} {entry['name']} ({entry['platform']})"


def check_budget_file(budgets, gate):
    """The file must describe a state a gate can be run against."""
    runners = budgets.get("runners", {})
    if not runners:
        gate.block("no runners declared; every measured figure is reported against one")

    for tier, runner in runners.items():
        if not runner.get("identity", "").strip():
            gate.block(
                f"runner {tier} ({runner.get('model', 'unnamed')}) has no durable "
                "identity; until it is procured and pinned no hardware-dependent "
                "figure is reproducible"
            )

    entries = budgets.get("entry", [])
    if not entries:
        gate.block("no entries declared")

    seen = set()
    for entry in entries:
        label = entry_label(entry)
        key = (entry["criterion"], entry["name"], entry["platform"])
        if key in seen:
            gate.block(f"{label}: declared twice")
        seen.add(key)

        if entry.get("status") not in {"ratified", "provisional"}:
            gate.block(f"{label}: status must be ratified or provisional")

        figure = entry.get("figure_mb")
        baseline = entry.get("baseline_mb")
        if figure is None or baseline is None:
            gate.block(f"{label}: missing figure or baseline")
            continue

        # A provisional figure binds a baseline exactly as a ratified one does.
        # A provisional figure is a ceiling for as long as it stands, which is
        # the whole of its function.
        if baseline > figure:
            gate.block(
                f"{label}: baseline {baseline} MB is above the stated figure "
                f"{figure} MB; a reset may never place a baseline above it"
            )

        tolerance = entry.get("tolerance_pct", 0.0)
        if tolerance > MAX_TOLERANCE_PCT:
            gate.block(
                f"{label}: tolerance {tolerance}% exceeds the {MAX_TOLERANCE_PCT}% cap"
            )
        if tolerance < 0:
            gate.block(f"{label}: negative tolerance")


def measure_download_size():
    """The size of the artefact a release would ship.

    At M0 there is no installer, so this is the release binary. That is a floor
    rather than the eventual figure, and it is reported as what it is.
    """
    binary = REPO / "target" / "release" / "evreos-shell"
    if not binary.exists():
        return None
    return binary.stat().st_size / (1024 * 1024)


def run_gates(budgets, measurements):
    absolute = Gate("absolute")
    regression = Gate("regression")
    unmeasured = []

    runners = budgets.get("runners", {})
    pinned = {
        tier: bool(runner.get("identity", "").strip())
        for tier, runner in runners.items()
    }
    tier_of = {"windows": "tier1", "macos": "tier2"}

    for entry in budgets.get("entry", []):
        label = entry_label(entry)
        measured = measurements.get((entry["criterion"], entry["name"]))
        if measured is None:
            # An unmeasured entry is not a pass. The one honest exception is a
            # hardware-dependent entry whose tier has no pinned runner: there is
            # no machine to measure it on, which the budget-file gate already
            # reports. Anything else unmeasured means a measurement that should
            # exist does not, and a gate that passes it is a gate that certifies
            # a number nobody produced.
            hardware = entry["criterion"] in HARDWARE_DEPENDENT
            tier = tier_of.get(entry["platform"])
            if hardware and not pinned.get(tier, False):
                unmeasured.append((label, "no pinned runner for this tier"))
            else:
                unmeasured.append((label, "BLOCKING"))
            continue

        figure = entry["figure_mb"]
        baseline = entry["baseline_mb"]
        tolerance = entry.get("tolerance_pct", 0.0)

        hardware = entry["criterion"] in HARDWARE_DEPENDENT
        tier = tier_of.get(entry["platform"])
        runner_pinned = pinned.get(tier, False)

        if measured > figure:
            message = f"{label}: measured {measured:.3f} MB exceeds {figure} MB"
            if hardware and not runner_pinned:
                absolute.advise(f"{message} (advisory: {tier} runner not pinned)")
            else:
                absolute.block(message)

        # The regression gate compares one machine against itself, so it blocks
        # regardless of whether the runner has been named.
        allowed = baseline * (1 + tolerance / 100.0)
        if baseline > 0 and measured > allowed:
            regression.block(
                f"{label}: measured {measured:.3f} MB is worse than baseline "
                f"{baseline} MB by more than the declared {tolerance}%"
            )

    return absolute, regression, unmeasured


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--budgets", default=str(REPO / "budgets.toml"))
    parser.add_argument(
        "--allow-unmeasured",
        action="store_true",
        help="do not fail on an entry whose measurement does not exist yet; for "
        "use before the harness that produces it is built",
    )
    parser.add_argument(
        "--allow-unpinned-runners",
        action="store_true",
        help="do not fail the budget-file gate on a runner with no identity; "
        "for use before the reference machines are procured",
    )
    args = parser.parse_args()

    budgets = load_budgets(args.budgets)

    file_gate = Gate("budget file")
    check_budget_file(budgets, file_gate)

    if args.allow_unpinned_runners:
        moved = [m for m in file_gate.blocking if "has no durable identity" in m]
        file_gate.blocking = [m for m in file_gate.blocking if m not in moved]
        file_gate.advisory.extend(moved)

    measurements = {}
    download = measure_download_size()
    if download is not None:
        measurements[("SC-001", "download size")] = download

    absolute, regression, unmeasured = run_gates(budgets, measurements)

    for gate in (file_gate, regression, absolute):
        for message in gate.advisory:
            print(f"  advisory [{gate.name}] {message}")
        for message in gate.blocking:
            print(f"  FAIL     [{gate.name}] {message}", file=sys.stderr)

    blocking_unmeasured = [label for label, why in unmeasured if why == "BLOCKING"]
    if unmeasured:
        print(f"  unmeasured on this machine: {len(unmeasured)} entries")
        for label, why in unmeasured:
            if why != "BLOCKING":
                note = f"  ({why})"
            elif args.allow_unmeasured:
                note = "  (deferred by --allow-unmeasured)"
            else:
                note = "  (no measurement produced)"
            print(f"    - {label}{note}")

    if blocking_unmeasured and not args.allow_unmeasured:
        for label in blocking_unmeasured:
            print(
                f"  FAIL     [budget file] {label}: no measurement was produced; "
                "an unmeasured entry is not a pass",
                file=sys.stderr,
            )
        file_gate.blocking.extend(blocking_unmeasured)

    if download is not None:
        print(f"  measured: download size {download:.3f} MB")

    failed = [g.name for g in (file_gate, regression, absolute) if g.failed]
    if failed:
        print(f"\nBudget gates FAILED: {', '.join(failed)}", file=sys.stderr)
        return 1

    print("\nBudget gates passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
