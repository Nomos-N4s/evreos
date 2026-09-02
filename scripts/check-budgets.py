#!/usr/bin/env python3
"""Enforce the budgets Principle II requires and FR-043 names.

WHAT THIS GATE IS, and what it deliberately is not.

Principle II: hard budgets "MUST live in one budget file in this repository and
MUST be enforced by CI gates that fail the build on regression." The Success
Criteria preamble defines three gates and says they are defined there and only
there. This script is those three, and it adds none of its own.

  BUDGET FILE  fails when the file does not describe a gateable state: an entry
               a criterion states is missing, a baseline above the entry's
               stated figure, or a runner not pinned. Not hardware dependent --
               it compares numbers in a file -- so it blocks from M0
               unconditionally. It is what bounds the advisory period on the
               absolute gate, rather than leaving that to good intentions. It
               also fails on a file this script cannot read as the schema
               states it -- a figure with no unit, or in a unit its criterion
               does not state; a spike exemption, baseline reset or wake
               lacking a field its schema names -- because a verdict over a
               misread file is a verdict on nothing.

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

A FIGURE IS COMPARED ONLY IN ITS CRITERION'S UNIT. Every entry states its unit,
and the unit is the one its criterion states -- MB, ms or percent-of-core -- so
a measurement produced in that unit is compared against a figure in the same
unit and never against one a different criterion states. This script's own
measurement is in MB, for SC-001; the harness figures arrive in the unit their
criterion states.

This script measures only what it can measure honestly on the machine it runs
on. SC-001's download and installed-footprint entries are build output and are
measured here. The hardware-dependent entries -- SC-002, SC-004, SC-005, SC-006
-- are measured by the benchmark harness on a pinned runner, and this script
reports them as unmeasured rather than inventing a number. An unmeasured entry
is not a pass.
"""
import argparse
import datetime
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

# The units a figure may be stated in, and the unit each criterion states its
# figures in. A criterion absent here states no figure and carries no entry:
# SC-003 is a required experience, verified by acceptance test.
UNITS = {"MB", "ms", "percent-of-core"}
UNIT_OF = {
    "SC-001": "MB",
    "SC-002": "ms",
    "SC-004": "MB",
    "SC-005": "percent-of-core",
    "SC-006": "ms",
}

# Field names the schema retired. An entry still written with them was not
# migrated, and is named as such rather than reported as merely incomplete.
RETIRED_FIELDS = ("figure_mb", "baseline_mb")

# The phrase every unpinned-runner failure carries. --allow-unpinned-runners
# defers exactly the failures that carry it and nothing else.
UNPINNED = "is not pinned"


def is_number(value):
    """An int or float. A bool is neither: TOML has no way to write one where a
    number is meant, and Python would otherwise accept `true` as 1."""
    return isinstance(value, (int, float)) and not isinstance(value, bool)


def is_positive_number(value):
    return is_number(value) and value > 0


def is_text(value):
    """A string with something in it. Whitespace is not a value."""
    return isinstance(value, str) and bool(value.strip())


def is_pull_request_number(value):
    return isinstance(value, int) and not isinstance(value, bool) and value > 0


def is_date(value):
    """A TOML date, written unquoted. tomllib parses one into datetime.date."""
    return isinstance(value, datetime.date)


# The sub-tables an entry may carry and the wake table, each as the fields it
# must carry with what each accepts. A sub-table carries exactly these fields:
# a misspelt one is refused rather than ignored, because an ignored field is how
# a reset that "names a decision" comes to name nothing.
SPIKE_EXEMPTION_FIELDS = (
    ("pull_request", is_pull_request_number, "the pull request's number"),
    ("figure", is_text, "the figure the spike measures, as its criterion states it"),
)
BASELINE_RESET_FIELDS = (
    ("date", is_date, "a TOML date, unquoted"),
    ("measured_cost", is_number, "a number in the entry's unit"),
    ("requirement_served", is_text, "the requirement the cost serves"),
    ("founder_decision", is_text, "the recorded founder decision, as decisions/NNNN"),
)
WAKE_FIELDS = (
    ("name", is_text, "a name"),
    ("period", is_positive_number, "a period in seconds"),
    ("processor_time_bound", is_positive_number, "a bound in ms of processor time"),
    ("justifying_requirement", is_text, "the requirement that justifies the wake"),
)


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


def runner_missing(runner):
    """What a runner block still lacks before it is pinned; empty once it is.

    Pinned means all three recorded: a durable identity, without which no
    figure measured on it is reproducible under SC-013; a runner label,
    without which no workflow job can resolve it; and the display refresh,
    without which SC-006's stated condition is unverifiable on it. All three
    are written when the machine is procured, so until then a runner fails the
    budget-file gate for the same reason three times over, reported once.
    """
    missing = []
    if not is_text(runner.get("identity", "")):
        missing.append("no durable identity")
    if not is_text(runner.get("runner_label", "")):
        missing.append("no runner_label")
    if not is_positive_number(runner.get("display_refresh", 0)):
        missing.append("display_refresh not recorded")
    return missing


def check_record(gate, label, record, fields):
    """A sub-table must carry exactly the fields its schema names, well typed."""
    if not isinstance(record, dict):
        names = ", ".join(name for name, _, _ in fields)
        gate.block(f"{label} must be a table {{ {names} }}")
        return
    for name, accepts, description in fields:
        if name not in record:
            gate.block(f"{label} lacks {name}")
        elif not accepts(record[name]):
            gate.block(f"{label}: {name} must be {description}")
    known = {name for name, _, _ in fields}
    for name in record:
        if name not in known:
            gate.block(f"{label} carries an unknown field {name}")


def check_budget_file(budgets, gate):
    """The file must describe a state a gate can be run against."""
    runners = budgets.get("runners", {})
    if not runners:
        gate.block("no runners declared; every measured figure is reported against one")

    for tier, runner in runners.items():
        model = runner.get("model", "unnamed")
        if not isinstance(runner.get("runner_label", ""), str):
            gate.block(f"runner {tier} ({model}): runner_label must be a string")
        hz = runner.get("display_refresh", 0)
        if not is_number(hz) or hz < 0:
            gate.block(
                f"runner {tier} ({model}): display_refresh must be a number of Hz, "
                "0 until recorded"
            )
        missing = runner_missing(runner)
        if missing:
            gate.block(
                f"runner {tier} ({model}) {UNPINNED}: {', '.join(missing)}; until "
                "it is procured and pinned no hardware-dependent figure is "
                "reproducible and no workflow job can resolve it"
            )

    entries = budgets.get("entry", [])
    if not entries:
        gate.block("no entries declared")

    seen = set()
    for entry in entries:
        identity = ("criterion", "name", "platform")
        if any(not is_text(entry.get(field)) for field in identity):
            gate.block(f"an entry without a criterion, a name and a platform: {entry}")
            continue

        label = entry_label(entry)
        key = (entry["criterion"], entry["name"], entry["platform"])
        if key in seen:
            gate.block(f"{label}: declared twice")
        seen.add(key)

        if entry.get("status") not in {"ratified", "provisional"}:
            gate.block(f"{label}: status must be ratified or provisional")

        for old in RETIRED_FIELDS:
            if old in entry:
                gate.block(
                    f"{label}: {old} is a retired field; the schema is figure and "
                    "baseline in a stated unit"
                )

        # The unit is required and is the criterion's. Without one a figure is
        # not a number a measurement can be compared against; in another
        # criterion's unit it is not the entry this criterion states.
        criterion = entry["criterion"]
        stated = UNIT_OF.get(criterion)
        unit = entry.get("unit")
        units = ", ".join(sorted(UNITS))
        if stated is None:
            gate.block(f"{label}: {criterion} states no budget entry")
        if unit is None:
            gate.block(f"{label}: no unit; one of {units} is required")
        elif unit not in UNITS:
            gate.block(f"{label}: unit {unit!r} is not one of {units}")
        elif stated is not None and unit != stated:
            gate.block(
                f"{label}: unit {unit}; {criterion} states its figures in {stated}"
            )
        unit_text = unit if unit in UNITS else "(no unit)"

        decision = entry.get("founder_decision")
        if decision is not None and not is_text(decision):
            gate.block(f"{label}: founder_decision must cite a recorded decision")

        margin = entry.get("cross_check_margin")
        if margin is not None:
            if entry["criterion"] != "SC-004":
                gate.block(
                    f"{label}: cross_check_margin is declared only on SC-004, whose "
                    "whole-machine cross-check it bounds"
                )
            if not is_number(margin):
                gate.block(f"{label}: cross_check_margin must be a number")

        if "spike_exemption" in entry:
            check_record(
                gate, f"{label}: spike_exemption", entry["spike_exemption"],
                SPIKE_EXEMPTION_FIELDS,
            )
        if "baseline_reset" in entry:
            check_record(
                gate, f"{label}: baseline_reset", entry["baseline_reset"],
                BASELINE_RESET_FIELDS,
            )

        figure = entry.get("figure")
        baseline = entry.get("baseline")
        if figure is None or baseline is None:
            gate.block(f"{label}: missing figure or baseline")
            continue
        if not is_number(figure) or not is_number(baseline):
            gate.block(f"{label}: figure and baseline must be numbers")
            continue

        # A provisional figure binds a baseline exactly as a ratified one does.
        # A provisional figure is a ceiling for as long as it stands, which is
        # the whole of its function.
        if baseline > figure:
            gate.block(
                f"{label}: baseline {baseline} {unit_text} is above the stated figure "
                f"{figure} {unit_text}; a reset may never place a baseline above it"
            )

        tolerance = entry.get("tolerance_pct", 0.0)
        if not is_number(tolerance):
            gate.block(f"{label}: tolerance_pct must be a number")
            continue
        if tolerance > MAX_TOLERANCE_PCT:
            gate.block(
                f"{label}: tolerance {tolerance}% exceeds the {MAX_TOLERANCE_PCT}% cap"
            )
        if tolerance < 0:
            gate.block(f"{label}: negative tolerance")

    # SC-005's wake enumeration. An empty enumeration is a statement -- nothing
    # on the idle path is scheduled -- and is read as one; each wake declared
    # carries every field its schema names.
    wakes = budgets.get("wake")
    if wakes is not None:
        if not isinstance(wakes, list):
            gate.block("wake must be an array of tables, `wake = []` when empty")
        else:
            names = set()
            for position, wake in enumerate(wakes, start=1):
                name = wake.get("name") if isinstance(wake, dict) else None
                label = f"wake {name!r}" if is_text(name) else f"wake {position}"
                check_record(gate, label, wake, WAKE_FIELDS)
                if is_text(name):
                    if name in names:
                        gate.block(f"{label}: enumerated twice")
                    names.add(name)


def defer_unpinned_runners(gate):
    """Move the unpinned-runner failures to advisory, and only those.

    This is --allow-unpinned-runners: a stated deferral until the machines
    Q-E9a names are procured. Every other budget-file failure keeps blocking,
    which is what makes it a deferral rather than a way to turn the gate off.
    """
    moved = [message for message in gate.blocking if UNPINNED in message]
    gate.blocking = [message for message in gate.blocking if message not in moved]
    gate.advisory.extend(moved)


def measure_download_size():
    """The size of the artefact a release would ship, in MB, SC-001's unit.

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
    pinned = {tier: not runner_missing(runner) for tier, runner in runners.items()}
    tier_of = {"windows": "tier1", "macos": "tier2"}

    for entry in budgets.get("entry", []):
        label = entry_label(entry)
        figure = entry.get("figure")
        baseline = entry.get("baseline")
        unit = entry.get("unit")
        if not is_number(figure) or not is_number(baseline):
            # No figure to compare. The budget-file gate reports the entry;
            # a verdict here would be a verdict on nothing.
            continue
        if unit != UNIT_OF.get(entry["criterion"]):
            # A measurement arrives in the unit the criterion states. Against a
            # figure in any other unit the comparison is between two different
            # quantities, so none is made; the budget-file gate reports it.
            continue

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

        tolerance = entry.get("tolerance_pct", 0.0)

        hardware = entry["criterion"] in HARDWARE_DEPENDENT
        tier = tier_of.get(entry["platform"])
        runner_pinned = pinned.get(tier, False)

        if measured > figure:
            message = f"{label}: measured {measured:.3f} {unit} exceeds {figure} {unit}"
            if hardware and not runner_pinned:
                absolute.advise(f"{message} (advisory: {tier} runner not pinned)")
            else:
                absolute.block(message)

        # The regression gate compares one machine against itself, so it blocks
        # regardless of whether the runner has been named.
        allowed = baseline * (1 + tolerance / 100.0)
        if baseline > 0 and measured > allowed:
            regression.block(
                f"{label}: measured {measured:.3f} {unit} is worse than baseline "
                f"{baseline} {unit} by more than the declared {tolerance}%"
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
        help="do not fail the budget-file gate on a runner that is not pinned; "
        "for use before the reference machines are procured",
    )
    args = parser.parse_args()

    budgets = load_budgets(args.budgets)

    file_gate = Gate("budget file")
    check_budget_file(budgets, file_gate)

    if args.allow_unpinned_runners:
        defer_unpinned_runners(file_gate)

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
