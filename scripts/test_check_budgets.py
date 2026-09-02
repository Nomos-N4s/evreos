#!/usr/bin/env python3
"""Tests for the budget gate. The gate is CI's authority to fail a build, so
its own behaviour is checked rather than assumed."""
import datetime
import importlib.util
import io
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "budgets", Path(__file__).resolve().parent / "check-budgets.py"
)
budgets = importlib.util.module_from_spec(spec)
spec.loader.exec_module(budgets)

PASSED = FAILED = 0


def check(name, condition):
    global PASSED, FAILED
    if condition:
        PASSED += 1
    else:
        FAILED += 1
        print(f"FAIL: {name}", file=sys.stderr)


# The closed list as the preamble states it, written here independently of the
# script so that the script's copy is checked against something other than
# itself: nine entries per platform, eighteen in all, and no other.
NAMES = {
    "SC-001": ("download size", "installed footprint"),
    "SC-002": ("warm start", "cold start"),
    "SC-004": ("ten-tab memory",),
    "SC-005": ("60-minute window", "wake-free 1-second sample"),
    "SC-006": ("tab switch", "address-field keystroke"),
}
PLATFORMS = ("windows", "macos")
CLOSED_LIST = [
    (criterion, name, platform)
    for criterion, names in NAMES.items()
    for name in names
    for platform in PLATFORMS
]
DEFAULT = ("SC-001", "download size", "windows")

# One figure per criterion is enough for a fixture: what the gate reads is that
# a figure exists in the criterion's unit, not which of the criterion's figures
# it is.
FIGURES = {"SC-001": 20, "SC-002": 800, "SC-004": 150, "SC-005": 0.5, "SC-006": 16}


def stated_entry(criterion, name, platform):
    """One well-formed entry from the closed list: ratified by decisions/0001
    and unmeasured, so baseline 0.0 and tolerance 0.0, with SC-004's margin
    declared at 0.0 as the committed file declares it."""
    entry = {
        "criterion": criterion,
        "name": name,
        "platform": platform,
        "figure": FIGURES[criterion],
        "unit": budgets.UNIT_OF[criterion],
        "status": "ratified",
        "founder_decision": "decisions/0001",
        "baseline": 0.0,
        "tolerance_pct": 0.0,
    }
    if criterion == "SC-004":
        entry["cross_check_margin"] = 0.0
    return entry


def pinned_runner(tier, platform):
    """A runner with every field the budget-file gate requires of a pinned one.

    `os_version` and `memory` are here because FR-043 names them: a runner
    recording neither is not pinned however much else it records.
    """
    return {
        "platform": platform,
        "model": "a laptop",
        "os_version": "some release",
        "memory": "8 GB",
        "display_refresh": 60,
        "runner_label": f"evreos-{tier}",
        "identity": f"{tier}-abc123",
    }


# FR-043 names the operating-system version and the memory configuration, so a
# runner recording neither is not pinned. Placed with the helper above because
# they are about what `pinned_runner` must carry to earn its name.
def unpinned_without(field):
    runner = pinned_runner("tier1", "windows")
    runner.pop(field)
    return budgets.runner_missing(runner)


def identity(entry):
    return (entry["criterion"], entry["name"], entry["platform"])


def budget_file(**overrides):
    """A complete, well-formed file: the eighteen entries, both runners pinned,
    an empty wake enumeration. `entry` overrides fields on the one entry it
    names -- SC-001 download size (windows) unless it carries its own
    criterion, name or platform -- and an identity outside the closed list is
    added as a nineteenth entry. `runner` overrides tier 1; `wake` replaces the
    enumeration."""
    override = dict(overrides.pop("entry", {}))
    key = (
        override.get("criterion", DEFAULT[0]),
        override.get("name", DEFAULT[1]),
        override.get("platform", DEFAULT[2]),
    )
    entries = [stated_entry(*each) for each in CLOSED_LIST]
    for entry in entries:
        if identity(entry) == key:
            entry.update(override)
            break
    else:
        entries.append({**stated_entry(*DEFAULT), **override})
    runners = {
        "tier1": pinned_runner("tier1", "windows"),
        "tier2": pinned_runner("tier2", "macos"),
    }
    runners["tier1"].update(overrides.pop("runner", {}))
    return {"runners": runners, "entry": entries, "wake": overrides.pop("wake", [])}


def single_entry(**overrides):
    """A file carrying one entry -- SC-001 download size (windows), baseline
    1.0 -- for the measuring gates, which read the entries one at a time and
    report each: with eighteen, the case under test is lost among seventeen
    unmeasured ones. It is never passed to the budget-file gate, which rightly
    fails it on the seventeen it lacks."""
    entry = {**stated_entry(*DEFAULT), "baseline": 1.0}
    entry.update(overrides.pop("entry", {}))
    runner = pinned_runner("tier1", "windows")
    runner.update(overrides.pop("runner", {}))
    return {"runners": {"tier1": runner}, "entry": [entry], "wake": []}


def entry_in(b, criterion, name, platform):
    """The one entry of `b` with this identity."""
    for entry in b["entry"]:
        if identity(entry) == (criterion, name, platform):
            return entry
    raise KeyError((criterion, name, platform))


def without(b, criterion, name, platform):
    """`b` with that entry removed."""
    b["entry"] = [e for e in b["entry"] if identity(e) != (criterion, name, platform)]
    return b


def file_gate(b):
    g = budgets.Gate("f")
    budgets.check_budget_file(b, g)
    return g


# Well-formed sub-tables, for the cases that vary one field of each. The
# exemption names the default entry's own figure, as one must.
EXEMPTION = {"pull_request": 57, "figure": "download size"}
RESET = {
    "date": datetime.date(2026, 9, 1),
    "measured_cost": 1.5,
    "requirement_served": "FR-014",
    "founder_decision": "decisions/0002",
}
WAKE = {
    "name": "update check",
    "period": 21600,
    "processor_time_bound": 50,
    "justifying_requirement": "FR-014",
}


# --- budget-file gate ---------------------------------------------------------

check("a complete file passes the budget-file gate",
      not file_gate(budget_file()).failed)

check("an unpinned runner fails the budget-file gate",
      file_gate(budget_file(runner={"identity": ""})).failed)

check("whitespace is not an identity",
      file_gate(budget_file(runner={"identity": "   "})).failed)

check("a baseline above the stated figure fails",
      file_gate(budget_file(entry={"baseline": 25.0})).failed)

check("a baseline equal to the figure is permitted",
      not file_gate(budget_file(entry={"baseline": 20.0})).failed)

check("a tolerance above the cap fails",
      file_gate(budget_file(entry={"tolerance_pct": 5.1})).failed)

check("a tolerance at the cap is permitted",
      not file_gate(budget_file(entry={"tolerance_pct": 5.0})).failed)

check("a negative tolerance fails",
      file_gate(budget_file(entry={"tolerance_pct": -1.0})).failed)

check("an unknown status fails",
      file_gate(budget_file(entry={"status": "ratified-ish"})).failed)

b = budget_file()
del b["entry"][0]["status"]
check("an absent status fails", file_gate(b).failed)

check("an empty file fails rather than vacuously passing",
      file_gate({"runners": {}, "entry": []}).failed)

b = budget_file()
b["entry"].append(dict(b["entry"][0]))
check("a duplicated entry fails", file_gate(b).failed)

b = budget_file()
del b["entry"][0]["baseline"]
check("a missing baseline fails", file_gate(b).failed)

check("a figure that is not a number fails",
      file_gate(budget_file(entry={"figure": "20"})).failed)

check("a boolean is not a figure",
      file_gate(budget_file(entry={"figure": True})).failed)

b = budget_file()
del b["entry"][0]["criterion"]
check("an entry with no criterion fails rather than crashing the reader",
      file_gate(b).failed)

# --- the closed list --------------------------------------------------------
# Compared against the preamble's list rather than read off the file, because
# a file missing an entry cannot report its own omission. The list is checked
# here against an independent statement of it, so the script's copy is proved
# against the preamble rather than against itself.

check("the script's closed list is the preamble's eighteen, and no other",
      budgets.CLOSED_LIST == frozenset(CLOSED_LIST)
      and len(budgets.CLOSED_LIST) == 18)

check("...and every criterion on it states a unit, and no other criterion does",
      set(budgets.STATED_ENTRIES) == set(budgets.UNIT_OF))

ABSENT = "absent from the file"

g = file_gate(without(budget_file(), "SC-004", "ten-tab memory", "macos"))
check("an entry the preamble states and the file lacks fails",
      g.failed and len(g.blocking) == 1 and ABSENT in g.blocking[0])
check("...and the failure names the entry with its platform, since a figure "
      "stated per platform is one entry per platform",
      any("SC-004 ten-tab memory (macos)" in m for m in g.blocking))

b = without(budget_file(), "SC-002", "warm start", "windows")
g = file_gate(without(b, "SC-006", "tab switch", "macos"))
check("each absent entry is named",
      len(g.blocking) == 2 and all(ABSENT in m for m in g.blocking))

g = file_gate({"runners": budget_file()["runners"], "entry": [], "wake": []})
check("a file with no entries is failed on each of the eighteen, so the list is "
      "read from the preamble and not off the file",
      len([m for m in g.blocking if ABSENT in m]) == 18)

check("an entry no criterion states fails, since the list is closed",
      file_gate(budget_file(entry={"name": "cache size"})).failed)

check("an entry on a platform outside the two tiers fails on the same ground",
      file_gate(budget_file(entry={"platform": "linux"})).failed)

b = budget_file()
b["entry"][0]["name"] = "downlaod size"
g = file_gate(b)
check("a misspelt name is both an entry no criterion states and a stated entry "
      "that is absent",
      len(g.blocking) == 2 and any(ABSENT in m for m in g.blocking))

# --- the unit --------------------------------------------------------------
# A figure with no unit is not a number a measurement can be compared against,
# and a figure in a unit its criterion does not state is not that criterion's
# entry. Both are refused rather than inferred.

b = budget_file()
del b["entry"][0]["unit"]
check("an entry with no unit fails", file_gate(b).failed)

check("a unit outside MB, ms and percent-of-core fails",
      file_gate(budget_file(entry={"unit": "GB"})).failed)

check("a unit the criterion does not state fails",
      file_gate(budget_file(entry={"unit": "ms"})).failed)

check("SC-002 is stated in ms",
      not file_gate(budget_file(entry={"criterion": "SC-002", "name": "warm start",
                                       "figure": 800, "unit": "ms"})).failed)

window = {"criterion": "SC-005", "name": "60-minute window", "figure": 0.5,
          "unit": "percent-of-core"}
check("SC-005 is stated in percent-of-core",
      not file_gate(budget_file(entry=window)).failed)

check("SC-006 is stated in ms",
      not file_gate(budget_file(entry={"criterion": "SC-006", "name": "tab switch",
                                       "figure": 16, "unit": "ms"})).failed)

check("a criterion that states no budget entry fails",
      file_gate(budget_file(entry={"criterion": "SC-003"})).failed)

check("the retired figure_mb field fails, so an unmigrated entry is named as such",
      file_gate(budget_file(entry={"figure_mb": 20})).failed)

check("the retired baseline_mb field fails",
      file_gate(budget_file(entry={"baseline_mb": 0.0})).failed)

# --- the runner block -------------------------------------------------------
# Pinned means five things recorded: identity, runner_label, os_version, memory
# and display_refresh. FR-043's own sentence names three of them -- model,
# operating-system version, memory configuration, durable identifier -- so an
# absent os_version or memory is a requirement unmet rather than a convenience
# missing. All five are written together when the machine is procured, so any
# of them missing is the same unpinned state, reported once and deferred by one
# flag.

check("a runner with no runner_label is not pinned",
      file_gate(budget_file(runner={"runner_label": ""})).failed)

check("a runner with display_refresh 0 is not pinned",
      file_gate(budget_file(runner={"display_refresh": 0})).failed)

check("a runner with no operating-system version is not pinned",
      file_gate(budget_file(runner={"os_version": ""})).failed)

check("a runner with no memory configuration is not pinned",
      file_gate(budget_file(runner={"memory": ""})).failed)

# The floor is not the version. A tier declares what it admits; a figure is
# measured on one release, and an update moves the figure without touching the
# floor, so os_floor cannot stand in for os_version.
check("os_floor does not satisfy the operating-system version requirement",
      "no operating-system version" in " ".join(
          unpinned_without("os_version")))
check("a runner recording no memory says which requirement is unmet",
      "FR-043 requires" in " ".join(unpinned_without("memory")))

# The three fields recorded for reproducibility rather than gated: FR-043's
# list does not name them, and this gate enforces the requirement rather than a
# preference.
# These named a field and then never removed it -- the same trivially-true
# assertion three times, which would have passed had the field been gated.
# `unpinned_without` is the helper written for exactly this.
for ungated in ("os_build", "storage", "latency_rig"):
    runner = pinned_runner("tier1", "windows")
    runner[ungated] = ""
    check(f"{ungated} recorded empty does not make a runner unpinned",
          budgets.runner_missing(runner) == [])
    runner.pop(ungated)
    check(f"{ungated} absent entirely does not make a runner unpinned",
          budgets.runner_missing(runner) == [])

# ...while each gated field, removed, does. Without these the gating is
# asserted only in the direction that already held.
for gated in ("identity", "runner_label", "os_version", "memory"):
    check(f"{gated} absent makes a runner unpinned",
          unpinned_without(gated) != [])

b = budget_file()
del b["runners"]["tier1"]["runner_label"]
del b["runners"]["tier1"]["display_refresh"]
check("a runner block without the two fields is not pinned", file_gate(b).failed)

unprocured = {"identity": "", "runner_label": "", "os_version": "",
              "memory": "", "display_refresh": 0}
g = file_gate(budget_file(runner=unprocured))
check("a runner missing all five is reported once, not five times",
      len(g.blocking) == 1 and budgets.UNPINNED in g.blocking[0])

check("a negative display_refresh fails",
      file_gate(budget_file(runner={"display_refresh": -60})).failed)

check("a display_refresh that is not a number fails",
      file_gate(budget_file(runner={"display_refresh": "60"})).failed)

check("a runner_label that is not a string fails",
      file_gate(budget_file(runner={"runner_label": 7})).failed)

# The deferral is exactly the unpinned condition and nothing else.
g = file_gate(budget_file(runner={"runner_label": ""}, entry={"baseline": 25.0}))
budgets.defer_unpinned_runners(g)
check("--allow-unpinned-runners defers the unpinned runner",
      len(g.advisory) == 1 and budgets.UNPINNED in g.advisory[0])
check("...and defers nothing else", g.failed and len(g.blocking) == 1)

g = file_gate(budget_file(runner={"display_refresh": -60}))
budgets.defer_unpinned_runners(g)
check("a malformed display_refresh is not deferred as merely unpinned", g.failed)

# --- founder_decision -------------------------------------------------------
# A ratified figure is one a recorded founder decision set, so a ratified entry
# names that decision, in the register's citation form. A provisional entry
# names none: no decision has set its figure, which is what provisional means.

b = budget_file()
del b["entry"][0]["founder_decision"]
g = file_gate(b)
check("a ratified entry naming no founder decision fails",
      g.failed and len(g.blocking) == 1
      and "names no founder decision" in g.blocking[0])

b = budget_file(entry={"status": "provisional"})
del b["entry"][0]["founder_decision"]
check("a provisional entry naming none passes, since no decision has set its "
      "figure",
      not file_gate(b).failed)

check("a founder decision citing the register passes",
      not file_gate(budget_file(entry={"founder_decision": "decisions/0001"})).failed)

check("an empty founder decision fails",
      file_gate(budget_file(entry={"founder_decision": ""})).failed)

check("a founder decision that is not a string fails",
      file_gate(budget_file(entry={"founder_decision": 1})).failed)

# A description of a decision is not a citation to one: the gate can read a
# citation off the file and can read nothing off a description.
for written in ("Q-E9", "the founder, 2026-08-30", "decisions/1", "0001",
                "decisions/0001 "):
    check(f"a founder decision written {written!r} is not a citation and fails",
          file_gate(budget_file(entry={"founder_decision": written})).failed)

# --- cross_check_margin -----------------------------------------------------
# SC-004 declares its margin on every entry, as a percentage under the same
# cap as a tolerance. An undeclared margin is zero rather than unbounded, so
# omitting it does not switch the cross-check off -- and the file is refused
# for omitting it because the preamble requires the declaration.

TEN_TAB = {"criterion": "SC-004", "name": "ten-tab memory", "figure": 150}

check("a cross-check margin on SC-004 passes",
      not file_gate(budget_file(entry={**TEN_TAB, "cross_check_margin": 2.0})).failed)

b = budget_file()
del entry_in(b, "SC-004", "ten-tab memory", "windows")["cross_check_margin"]
g = file_gate(b)
check("SC-004 declaring no cross-check margin fails",
      g.failed and len(g.blocking) == 1 and "no cross_check_margin" in g.blocking[0])
check("...and the failure states the direction, that an undeclared margin is "
      "zero and not unbounded",
      any("zero rather than unbounded" in m for m in g.blocking))

check("a negative cross-check margin fails",
      file_gate(budget_file(entry={**TEN_TAB, "cross_check_margin": -0.5})).failed)

check("a cross-check margin above the cap fails",
      file_gate(budget_file(entry={**TEN_TAB, "cross_check_margin": 5.1})).failed)

check("a cross-check margin at the cap is permitted",
      not file_gate(budget_file(entry={**TEN_TAB, "cross_check_margin": 5.0})).failed)

check("a cross-check margin of zero is permitted, being what an unmeasured entry "
      "declares",
      not file_gate(budget_file(entry={**TEN_TAB, "cross_check_margin": 0.0})).failed)

check("the margin's cap is the tolerance's: the preamble sets one limit and says "
      "the margin is declared exactly as a tolerance is",
      budgets.MAX_CROSS_CHECK_MARGIN_PCT == budgets.MAX_TOLERANCE_PCT == 5.0)

check("a cross-check margin on a criterion other than SC-004 fails",
      file_gate(budget_file(entry={"cross_check_margin": 2.0})).failed)

check("a cross-check margin that is not a number fails",
      file_gate(budget_file(entry={**TEN_TAB, "cross_check_margin": "2"})).failed)

# --- spike_exemption --------------------------------------------------------
# Recorded on an entry while a spike establishes a figure that does not yet
# exist. The budget-file gate reads its schema and refuses one naming another
# entry's figure; what the exemption lifts is proved under the measuring gates.

check("a complete spike exemption passes",
      not file_gate(budget_file(entry={"spike_exemption": dict(EXEMPTION)})).failed)

for field in EXEMPTION:
    record = dict(EXEMPTION)
    del record[field]
    check(f"a spike exemption lacking {field} fails",
          file_gate(budget_file(entry={"spike_exemption": record})).failed)


def exemption(**fields):
    return budget_file(entry={"spike_exemption": {**EXEMPTION, **fields}})


check("a spike exemption naming no pull request number fails",
      file_gate(exemption(pull_request=0)).failed)

check("a spike exemption with an unknown field fails",
      file_gate(exemption(figur="x")).failed)

check("a spike exemption that is not a table fails",
      file_gate(budget_file(entry={"spike_exemption": 57})).failed)

g = file_gate(exemption(figure="cold start"))
check("a spike exemption naming another entry's figure fails, since an exemption "
      "never extends to another entry",
      g.failed and len(g.blocking) == 1
      and "never extends to another entry" in g.blocking[0])

check("a spike exemption never lifts the budget-file gate: a baseline above the "
      "figure fails with one recorded",
      file_gate(budget_file(entry={"baseline": 25.0,
                                   "spike_exemption": dict(EXEMPTION)})).failed)

# --- baseline_reset ---------------------------------------------------------
# A reset is the provenance of a baseline moved upward, which only a recorded
# founder decision may do, and it may never place the baseline above the
# entry's stated figure. The fixtures write the baseline the reset moved to,
# because a reset on a baseline no measurement has written is not a case worth
# stating either way.


def reset(**fields):
    return budget_file(entry={"baseline": 12.0,
                              "baseline_reset": {**RESET, **fields}})


check("a complete baseline reset passes", not file_gate(reset()).failed)

for field in RESET:
    record = dict(RESET)
    del record[field]
    check(f"a baseline reset lacking {field} fails",
          file_gate(budget_file(entry={"baseline": 12.0,
                                       "baseline_reset": record})).failed)

check("a baseline reset whose date is a string fails",
      file_gate(reset(date="2026-09-01")).failed)

check("a baseline reset naming an empty decision fails",
      file_gate(reset(founder_decision="")).failed)

g = file_gate(reset(founder_decision="the founder agreed on 2026-09-01"))
check("a baseline reset naming no recorded decision fails, a description not "
      "being a citation",
      g.failed and len(g.blocking) == 1 and "decisions/NNNN" in g.blocking[0])

check("a baseline reset with a cost that is not a number fails",
      file_gate(reset(measured_cost="1.5 MB")).failed)

g = file_gate(budget_file(entry={"baseline": 25.0, "baseline_reset": dict(RESET)}))
check("a baseline reset that leaves the baseline above the stated figure fails",
      g.failed and len(g.blocking) == 1)
check("...and the failure names the reset as what placed it there",
      any("baseline_reset" in m and "above the stated figure" in m
          for m in g.blocking))

check("a baseline reset that leaves the baseline at the stated figure is permitted",
      not file_gate(budget_file(entry={"baseline": 20.0,
                                       "baseline_reset": dict(RESET)})).failed)

check("a provisional figure binds a reset exactly as a ratified one does",
      file_gate(budget_file(entry={"status": "provisional", "baseline": 25.0,
                                   "baseline_reset": dict(RESET)})).failed)

# --- the wake enumeration ---------------------------------------------------
# SC-005's. Absent it fails and empty it passes; each wake carries its four
# fields and a bound inside the per-wake cap; and the bounds together stay
# inside the window cap at the count the worst 60-minute window holds of each,
# floor(3600 / period) + 1, a window that opens on one firing and closes on
# another.

check("the caps are SC-005's own: 50 ms a wake, 500 ms an hour",
      budgets.WAKE_BOUND_CAP_MS == 50 and budgets.WAKES_WINDOW_CAP_MS == 500)

b = budget_file()
del b["wake"]
g = file_gate(b)
check("an absent enumeration fails",
      g.failed and len(g.blocking) == 1 and "no wake enumeration" in g.blocking[0])
check("...and the failure says how a file with no wake states that",
      "`wake = []`" in g.blocking[0])

check("an empty enumeration passes, being a statement rather than an omission",
      not file_gate(budget_file(wake=[])).failed)

check("a complete wake passes", not file_gate(budget_file(wake=[dict(WAKE)])).failed)

for field in WAKE:
    record = dict(WAKE)
    del record[field]
    check(f"a wake lacking {field} fails", file_gate(budget_file(wake=[record])).failed)

check("a wake with a period of zero fails",
      file_gate(budget_file(wake=[{**WAKE, "period": 0}])).failed)

check("a wake with a negative processor-time bound fails",
      file_gate(budget_file(wake=[{**WAKE, "processor_time_bound": -1}])).failed)

check("a wake enumerated twice fails",
      file_gate(budget_file(wake=[dict(WAKE), dict(WAKE)])).failed)

check("a wake that is not a table fails",
      file_gate(budget_file(wake=["update check"])).failed)

check("an enumeration that is not an array fails",
      file_gate(budget_file(wake=WAKE)).failed)

# The per-wake cap: a bound declared above 50 ms declares a wake SC-005 does
# not permit.
g = file_gate(budget_file(wake=[{**WAKE, "processor_time_bound": 51}]))
check("a wake bounded above 50 ms fails",
      g.failed and len(g.blocking) == 1 and "caps a wake at" in g.blocking[0])

check("a wake bounded at 50 ms passes",
      not file_gate(budget_file(wake=[{**WAKE, "processor_time_bound": 50}])).failed)

# The count a worst-case window holds.
check("a wake with a period above an hour fires once in a 60-minute window",
      budgets.firings_in_window(21600) == 1)
check("a wake with a period of exactly an hour fires twice, on the window's "
      "opening and on its close",
      budgets.firings_in_window(3600) == 2)
check("a wake every ten minutes fires seven times",
      budgets.firings_in_window(600) == 7)


def wake(name, period, bound=50):
    return {"name": name, "period": period, "processor_time_bound": bound,
            "justifying_requirement": "FR-014"}


# The window cap, over every wake at that count.
check("ten 50 ms firings in an hour pass, at the cap",
      not file_gate(budget_file(wake=[wake("a", 400)])).failed)

g = file_gate(budget_file(wake=[wake("a", 360)]))
check("eleven fail: a 6-minute wake at the cap fires eleven times in a closed hour",
      g.failed and len(g.blocking) == 1 and "above the 500 ms" in g.blocking[0])
check("...and the failure states the sum and each wake's share of it",
      "550 ms" in g.blocking[0] and "50 ms x 11" in g.blocking[0])

check("the sum is over every wake: two inside the per-wake cap fail together",
      file_gate(budget_file(wake=[wake("a", 600), wake("b", 900)])).failed)

check("...and two that sum to the cap pass",
      not file_gate(budget_file(wake=[wake("a", 600), wake("b", 1800)])).failed)

check("a wake well under the per-wake cap still counts toward the window",
      file_gate(budget_file(wake=[wake("a", 60, 10)])).failed)

g = file_gate(budget_file(wake=[{**WAKE, "period": 0}, wake("b", 400)]))
check("a wake the schema cannot read is reported and left out of the sum",
      g.failed and len(g.blocking) == 1 and "period" in g.blocking[0])

# --- the repository's own budget file ---------------------------------------
# The file as committed reads under the schema and fails only on what it says
# it fails on: two runners awaiting procurement. The gate compares it against
# the closed list, requires a decision on every ratified entry and a margin on
# both SC-004 entries, so those hold by the gate passing. What is proved here
# beyond that is Q-E9's split of the entries, the wording of the conditions,
# and the enumeration's arithmetic, none of which a gate clause states.

real = budgets.load_budgets(budgets.REPO / "budgets.toml")
g = file_gate(real)
check("the committed budget file fails only on its two unpinned runners",
      len(g.blocking) == 2 and all(budgets.UNPINNED in m for m in g.blocking))
budgets.defer_unpinned_runners(g)
check("...and passes once that is deferred", not g.failed)
check("every committed entry states its unit",
      all(e.get("unit") == budgets.UNIT_OF[e["criterion"]] for e in real["entry"]))

# The closed list, against this file's independent statement of it.
committed = {identity(e) for e in real["entry"]}
check("the committed budget file carries exactly the eighteen entries the "
      "preamble closes over",
      len(real["entry"]) == 18 and committed == set(CLOSED_LIST))

check("every committed entry states its measurement condition",
      all(budgets.is_text(e.get("condition")) for e in real["entry"]))

# Q-E9's split: thirteen ratified, five provisional -- SC-002's four and SC-004
# on tier 2 -- and every ratified entry names the decision that set it.
ratified = [e for e in real["entry"] if e["status"] == "ratified"]
provisional = [e for e in real["entry"] if e["status"] == "provisional"]
check("thirteen committed entries are ratified and five provisional",
      len(ratified) == 13 and len(provisional) == 5)
check("the provisional entries are SC-002's four and SC-004 on macos",
      {(e["criterion"], e["platform"]) for e in provisional}
      == {("SC-002", "windows"), ("SC-002", "macos"), ("SC-004", "macos")})
check("every ratified entry names decisions/0001",
      all(e.get("founder_decision") == "decisions/0001" for e in ratified))
check("no provisional entry names a founder decision, since one that set its "
      "figure would ratify it",
      all("founder_decision" not in e for e in provisional))

# SC-004's two entries declare the cross-check margin, and only they do.
check("both SC-004 entries declare a cross_check_margin",
      all("cross_check_margin" in e
          for e in real["entry"] if e["criterion"] == "SC-004"))
check("no other committed entry declares one",
      all("cross_check_margin" not in e
          for e in real["entry"] if e["criterion"] != "SC-004"))

# SC-005's conditions state what 0.5% of one core is in processor time at each
# condition's own scale, since that is the quantity a harness reports.
by_key = {identity(e): e for e in real["entry"]}
check("the 60-minute window condition names 18 s of processor time",
      all("18 s" in by_key[("SC-005", "60-minute window", p)]["condition"]
          for p in PLATFORMS))
check("the wake-free sample condition names 5 ms of processor time",
      all("5 ms" in by_key[("SC-005", "wake-free 1-second sample", p)]["condition"]
          for p in PLATFORMS))

# A tolerance and a margin are justified by measured variation, so an entry no
# measurement has written a baseline for cannot have either.
check("an unmeasured committed entry declares no tolerance",
      all(e["tolerance_pct"] == 0.0 for e in real["entry"] if e["baseline"] == 0.0))
check("an unmeasured committed SC-004 entry declares a margin of zero",
      all(e["cross_check_margin"] == 0.0 for e in real["entry"]
          if e["criterion"] == "SC-004" and e["baseline"] == 0.0))

# SC-005's enumeration: the two wakes it names, each inside the per-wake cap,
# and together inside the hourly cap however their periods fall in an hour. A
# closed 60-minute window holds at most floor(3600 / period) + 1 firings of a
# wake, so that is the count each bound is multiplied by.
wakes = real.get("wake")
check("the committed budget file enumerates the update check and the "
      "blocking-list refresh",
      isinstance(wakes, list)
      and {w["name"] for w in wakes} == {"update check", "blocking-list refresh"})
check("each committed wake names the requirement SC-005 justifies it by",
      {(w["name"], w["justifying_requirement"]) for w in wakes}
      == {("update check", "FR-014"), ("blocking-list refresh", "FR-008")})
check("each committed wake is bounded at or below 50 ms of processor time",
      all(0 < w["processor_time_bound"] <= 50 for w in wakes))
hourly = sum(w["processor_time_bound"] * (3600 // w["period"] + 1) for w in wakes)
check("the committed wakes sum inside 500 ms of processor time in any 60-minute "
      "window",
      hourly <= 500)
check("...at the count the gate multiplies each bound by",
      hourly == sum(w["processor_time_bound"] * budgets.firings_in_window(w["period"])
                    for w in wakes))
check("the committed budget file records no unretired spike exemption",
      budgets.unretired_exemptions(real) == [])

# --- absolute and regression gates -------------------------------------------
# These read entries one at a time and report each, so they are exercised on a
# one-entry file; the budget-file gate, which would fail that file on the
# seventeen it lacks, is not run on it.

b = single_entry()
absolute, regression, unmeasured = budgets.run_gates(
    b, {("SC-001", "download size", "windows"): 25.0}
)
check("exceeding a non-hardware figure blocks the absolute gate", absolute.failed)
check("...and the verdict is stated in the entry's unit",
      "25.000 MB exceeds 20 MB" in absolute.blocking[0])

b = single_entry()
absolute, regression, _ = budgets.run_gates(
    b, {("SC-001", "download size", "windows"): 1.0}
)
check("meeting the figure passes the absolute gate", not absolute.failed)

# An undeclared tolerance is zero, not unbounded: the opposite reading lets an
# entry disable its own regression gate by omitting a field.
b = single_entry()
del b["entry"][0]["tolerance_pct"]
absolute, regression, _ = budgets.run_gates(
    b, {("SC-001", "download size", "windows"): 1.001}
)
check("an undeclared tolerance is zero, not unbounded", regression.failed)

b = single_entry(entry={"tolerance_pct": 5.0})
absolute, regression, _ = budgets.run_gates(
    b, {("SC-001", "download size", "windows"): 1.04}
)
check("a regression inside the declared tolerance passes", not regression.failed)

b = single_entry(entry={"tolerance_pct": 5.0})
absolute, regression, _ = budgets.run_gates(
    b, {("SC-001", "download size", "windows"): 1.06}
)
check("a regression outside the declared tolerance blocks", regression.failed)
check("...stated in the entry's unit",
      "1.060 MB is worse than baseline 1.0 MB" in regression.blocking[0])

# A millisecond entry is compared and reported in milliseconds.
b = single_entry(
    entry={"criterion": "SC-002", "name": "warm start", "figure": 800, "unit": "ms",
           "baseline": 700.0, "tolerance_pct": 0.0},
)
absolute, regression, _ = budgets.run_gates(
    b, {("SC-002", "warm start", "windows"): 900.0}
)
check("a millisecond breach blocks on a pinned runner",
      absolute.failed and regression.failed)
check("...and is stated in ms", "900.000 ms exceeds 800 ms" in absolute.blocking[0])

# An entry the budget-file gate has already refused is not compared: a
# measurement in the criterion's unit against a figure in another is two
# quantities, and a verdict over them would be a verdict on nothing.
over = {("SC-001", "download size", "windows"): 25.0}
b = single_entry(entry={"unit": "ms"})
absolute, regression, unmeasured = budgets.run_gates(b, over)
check("a figure in a unit its criterion does not state gets no verdict",
      not absolute.failed and not absolute.advisory and not regression.failed
      and not unmeasured)

b = single_entry(entry={"figure": "20"})
absolute, regression, unmeasured = budgets.run_gates(b, over)
check("a figure that is not a number gets no verdict rather than a crash",
      not absolute.failed and not unmeasured)

# A hardware-dependent entry's absolute gate is advisory until its runner is
# pinned; its regression gate is not, because it compares a machine to itself.
b = single_entry(
    entry={"criterion": "SC-004", "name": "ten-tab memory", "figure": 150},
    runner={"identity": ""},
)
absolute, regression, _ = budgets.run_gates(
    b, {("SC-004", "ten-tab memory", "windows"): 200.0}
)
check("an unpinned hardware entry's absolute breach is advisory", not absolute.failed)
check("...and is still reported", len(absolute.advisory) == 1)

b = single_entry(
    entry={"criterion": "SC-004", "name": "ten-tab memory", "figure": 150},
    runner={"runner_label": ""},
)
absolute, regression, _ = budgets.run_gates(
    b, {("SC-004", "ten-tab memory", "windows"): 200.0}
)
check("a runner with no label is unpinned for the absolute gate too",
      not absolute.failed)

# The other half of that rule, and the one the deferral turns on. Only a
# HARDWARE-dependent entry waits on a pinned runner. SC-001 is measured from the
# release artefact and means the same on any machine, so an unpinned tier is no
# reason to soften its breach -- and dropping the `hardware and` qualifier from
# that branch is a loosening the suite could not see, on the one figure this
# script actually measures.
b = single_entry(entry={"figure": 10}, runner={"identity": ""})
absolute, regression, _ = budgets.run_gates(
    b, {("SC-001", "download size", "windows"): 500.0}
)
check("a non-hardware breach blocks even on an unpinned runner", absolute.failed)
check("...and is not filed as advisory", not absolute.advisory)

# Every criterion the deferral covers, not just the one that was fixtured. Each
# could be dropped from HARDWARE_DEPENDENT and the entry's breach would go from
# advisory to blocking -- a stricter verdict, so no assertion about failing
# caught it.
for criterion, name, figure, measured in (
    ("SC-002", "warm start", 800, 5000.0),
    ("SC-005", "60-minute window", 2, 50.0),
    ("SC-006", "tab switch", 50, 900.0),
):
    b = single_entry(
        entry={"criterion": criterion, "name": name, "figure": figure,
               "unit": budgets.UNIT_OF[criterion]},
        runner={"identity": ""},
    )
    absolute, _, _ = budgets.run_gates(b, {(criterion, name, "windows"): measured})
    check(f"{criterion} is hardware-dependent, so its unpinned breach is advisory",
          not absolute.failed and len(absolute.advisory) == 1)

# Both gates compare with a strict `>`, so a figure met exactly is met. Relaxing
# either to `>=` blocks a release that is inside its budget, and nothing sat on
# the boundary to notice.
b = single_entry(entry={"figure": 20})
absolute, _, _ = budgets.run_gates(b, {("SC-001", "download size", "windows"): 20.0})
check("a measurement equal to the figure passes the absolute gate",
      not absolute.failed and not absolute.advisory)

b = single_entry(entry={"tolerance_pct": 5.0})
_, regression, _ = budgets.run_gates(b, {("SC-001", "download size", "windows"): 1.05})
check("a measurement exactly at the tolerance passes the regression gate",
      not regression.failed)

# `baseline > 0` is what makes an unmeasured entry silent rather than a ceiling
# at zero. Every entry in the committed file carries baseline 0.0, so without
# this guard the regression gate would block the first measurement of all
# eighteen -- the gate exists to compare a machine against itself, and there is
# nothing yet to compare against.
b = single_entry(entry={"baseline": 0.0, "tolerance_pct": 0.0})
_, regression, _ = budgets.run_gates(b, {("SC-001", "download size", "windows"): 5.0})
check("a zero baseline is no baseline yet, not a ceiling", not regression.failed)

b = single_entry(
    entry={"criterion": "SC-004", "name": "ten-tab memory", "figure": 150},
    runner={"identity": "pinned-1"},
)
absolute, regression, _ = budgets.run_gates(
    b, {("SC-004", "ten-tab memory", "windows"): 200.0}
)
check("a pinned hardware entry's absolute breach blocks", absolute.failed)

b = single_entry(
    entry={
        "criterion": "SC-004",
        "name": "ten-tab memory",
        "figure": 150,
        "baseline": 100.0,
        "tolerance_pct": 0.0,
    },
    runner={"identity": ""},
)
absolute, regression, _ = budgets.run_gates(
    b, {("SC-004", "ten-tab memory", "windows"): 120.0}
)
check("regression blocks even on an unpinned runner", regression.failed)

# An unmeasured entry is not a pass.
b = single_entry()
absolute, regression, unmeasured = budgets.run_gates(b, {})
check("an unmeasured entry is reported, not silently passed", len(unmeasured) == 1)

# --- unmeasured entries ------------------------------------------------------
# The docstring promises "an unmeasured entry is not a pass". These cover the
# case where getting it wrong is silently permissive.

b = single_entry()
absolute, regression, unmeasured = budgets.run_gates(b, {}, host="windows")
check("a non-hardware entry with no measurement is blocking, with the reason",
      unmeasured == [("SC-001 download size (windows)",
                      "no measurement was produced", True)])

b = single_entry(
    entry={"criterion": "SC-004", "name": "ten-tab memory", "figure": 150},
    runner={"identity": ""},
)
absolute, regression, unmeasured = budgets.run_gates(b, {})
check("an unmeasured hardware entry with no pinned runner is not blocking",
      unmeasured and not unmeasured[0][2]
      and unmeasured[0][1] == "no pinned runner for this tier")

b = single_entry(
    entry={"criterion": "SC-004", "name": "ten-tab memory", "figure": 150},
    runner={"identity": "pinned-1"},
)
absolute, regression, unmeasured = budgets.run_gates(b, {})
check("an unmeasured hardware entry WITH a pinned runner is blocking",
      unmeasured and unmeasured[0][2])

b = single_entry()
absolute, regression, unmeasured = budgets.run_gates(
    b, {("SC-001", "download size", "windows"): 1.0}
)
check("a measured entry is not reported unmeasured", unmeasured == [])

# --- the measurement key -----------------------------------------------------
# A measurement is keyed on (criterion, name, platform), an entry's whole
# identity. What these prove against is the defect the merged gate had: one
# Linux binary, keyed on (criterion, name) alone, was compared against both
# download-size entries, and neither entry's condition -- the installer
# artefact CI publishes -- was met by it. A host now measures the artefact it
# builds, declares the platform, and satisfies that platform's entry alone.

WIN = ("SC-001", "download size", "windows")
MAC = ("SC-001", "download size", "macos")
MiB = 1024 * 1024
TREES = []  # every packaging_tree() made, removed once the run is over


def both_platforms():
    """The download-size entry on both platforms, baseline 1.0, both runners
    pinned: the measuring gates' two-entry fixture."""
    b = single_entry()
    b["entry"].append({**stated_entry(*MAC), "baseline": 1.0})
    b["runners"]["tier2"] = pinned_runner("tier2", "macos")
    return b


def packaging_tree(**artefacts):
    """A repository root carrying the named installer artefacts, each of the
    size in bytes given, where its platform's build publishes it."""
    root = Path(tempfile.mkdtemp())
    TREES.append(root)
    for name, size in artefacts.items():
        platform = "windows" if name.endswith(".msi") else "macos"
        directory = root / budgets.INSTALLER_ARTEFACT[platform][0]
        directory.mkdir(parents=True, exist_ok=True)
        (directory / name).write_bytes(b"\0" * size)
    return root


def labels(unmeasured):
    return [label for label, _, _ in unmeasured]


# The host declares the tier it builds for, and no other.
check("a Windows host builds the tier-1 artefact",
      budgets.host_platform("win32") == "windows")
check("a macOS host builds the tier-2 artefact",
      budgets.host_platform("darwin") == "macos")
check("a Linux host builds no tier's artefact, Linux being the deferred platform",
      budgets.host_platform("linux") is None)
check("each tier's artefact is read where its own build publishes it: WiX's .msi "
      "on tier 1, productbuild's .pkg on tier 2",
      set(budgets.INSTALLER_ARTEFACT) == set(PLATFORMS)
      and budgets.INSTALLER_ARTEFACT["windows"][1] == ".msi"
      and budgets.INSTALLER_ARTEFACT["macos"][1] == ".pkg"
      and len({d for d, _ in budgets.INSTALLER_ARTEFACT.values()}) == 2)

# measure_download_size() reads the host's own artefact and declares the host.
tree = packaging_tree(**{"a.msi": 12 * MiB, "b.pkg": 15 * MiB})

measured = budgets.measure_download_size("windows", tree)
check("a Windows host measures the .msi, in MB, and declares windows",
      measured.platform == "windows" and measured.megabytes == 12.0
      and measured.reason is None)

measured = budgets.measure_download_size("macos", tree)
check("a macOS host measures the .pkg and declares macos, not the .msi beside it",
      measured.platform == "macos" and measured.megabytes == 15.0)

measured = budgets.measure_download_size(None, tree)
check("a host of no tier measures nothing, though both artefacts stand on its disk",
      measured.platform is None and measured.megabytes is None
      and "builds no tier's installer artefact" in measured.reason)

measured = budgets.measure_download_size("windows", packaging_tree())
check("a tier's host with no artefact measures nothing and says the installer is "
      "not built yet",
      measured.platform == "windows" and measured.megabytes is None
      and ".msi" in measured.reason and "not built yet" in measured.reason)

measured = budgets.measure_download_size(
    "windows", packaging_tree(**{"a.msi": MiB, "b.msi": MiB})
)
check("two artefacts where exactly one is served to everyone are reported, not "
      "picked between",
      measured.megabytes is None and "exactly one" in measured.reason
      and "a.msi" in measured.reason and "b.msi" in measured.reason)

tree = packaging_tree()
(tree / "target" / "release").mkdir(parents=True)
(tree / "target" / "release" / "evreos-shell").write_bytes(b"\0" * MiB)
check("the release binary is not read: a host with only that on its disk "
      "measures nothing, whatever tier it is",
      all(budgets.measure_download_size(host, tree).megabytes is None
          for host in ("windows", "macos", None)))

# One host's artefact no longer satisfies both platform entries.
measured = budgets.measure_download_size("windows", packaging_tree(**{"a.msi": MiB}))
absolute, regression, unmeasured = budgets.run_gates(
    both_platforms(),
    {("SC-001", "download size", measured.platform): measured.megabytes},
    host=measured.platform,
)
check("one host's artefact satisfies its own platform's entry...",
      not absolute.failed and not regression.failed and len(unmeasured) == 1)
check("...and leaves the other platform's entry unmeasured, with the reason",
      labels(unmeasured) == ["SC-001 download size (macos)"]
      and "a macos figure is measured on a macos host" in unmeasured[0][1]
      and "this is the windows host" in unmeasured[0][1])
check("...which blocks: an unmeasured entry is not a pass, and another platform's "
      "artefact is no measurement of it",
      unmeasured[0][2] is True)

absolute, regression, unmeasured = budgets.run_gates(
    both_platforms(), {WIN: 25.0}, host="windows"
)
check("a windows breach fails the windows entry alone",
      absolute.failed and len(absolute.blocking) == 1
      and "(windows)" in absolute.blocking[0]
      and labels(unmeasured) == ["SC-001 download size (macos)"])

absolute, regression, unmeasured = budgets.run_gates(
    both_platforms(), {MAC: 25.0}, host="macos"
)
check("...and a macos breach the macos entry alone",
      absolute.failed and len(absolute.blocking) == 1
      and "(macos)" in absolute.blocking[0]
      and labels(unmeasured) == ["SC-001 download size (windows)"])

# A measurement for one platform never satisfies another platform's entry.
absolute, regression, unmeasured = budgets.run_gates(single_entry(), {MAC: 1.0})
check("a macos measurement inside the figure does not pass the windows entry",
      not absolute.failed and labels(unmeasured) == ["SC-001 download size (windows)"])

absolute, regression, unmeasured = budgets.run_gates(single_entry(), {MAC: 25.0})
check("...and one over the figure does not fail it either, being no measurement "
      "of it",
      not absolute.failed and not absolute.advisory and not regression.failed
      and len(unmeasured) == 1)

absolute, regression, unmeasured = budgets.run_gates(
    single_entry(), {("SC-001", "download size"): 1.0}
)
check("a measurement keyed with no platform, the old key, satisfies nothing",
      not absolute.failed and len(unmeasured) == 1)

# A host of no tier, which is what the build job runs on today.
absolute, regression, unmeasured = budgets.run_gates(both_platforms(), {}, host=None)
check("on a host of no tier both download-size entries are unmeasured, each with "
      "the reason",
      labels(unmeasured) == ["SC-001 download size (windows)",
                             "SC-001 download size (macos)"]
      and all("this host builds no tier's artefact" in reason
              for _, reason, _ in unmeasured)
      and all(f"a {p} figure is measured on a {p} host" in reason
              for (_, reason, _), p in zip(unmeasured, PLATFORMS)))
check("...and both block", all(blocking for _, _, blocking in unmeasured))

# A harness figure is the pinned runner's, not the host's.
b = single_entry(entry={"criterion": "SC-004", "name": "ten-tab memory", "figure": 150})
absolute, regression, unmeasured = budgets.run_gates(b, {}, host="macos")
check("an unmeasured hardware entry on a pinned runner is blocking whatever host "
      "runs the gate, its figure being the runner's",
      unmeasured == [("SC-004 ten-tab memory (windows)",
                      "no measurement was produced", True)])

# --- the spike exemption under the measuring gates ---------------------------
# It lifts that one entry's absolute gate and nothing else: never the
# regression gate, never the budget-file gate, never another entry.

EXEMPT = {"spike_exemption": dict(EXEMPTION)}

b = single_entry(entry=EXEMPT)
absolute, regression, unmeasured = budgets.run_gates(b, over)
check("a spike exemption lifts the entry's absolute gate", not absolute.failed)
check("...and the breach is still reported, naming the pull request and the "
      "consequence",
      len(absolute.advisory) == 1 and "#57" in absolute.advisory[0]
      and "not released or tagged" in absolute.advisory[0])
check("...and never the regression gate", regression.failed)

b = single_entry(entry=EXEMPT)
b["entry"].append({**stated_entry("SC-001", "installed footprint", "windows"),
                   "baseline": 1.0})
absolute, regression, _ = budgets.run_gates(
    b, {("SC-001", "download size", "windows"): 25.0,
        ("SC-001", "installed footprint", "windows"): 70.0}
)
check("...and never another entry: the breach beside it blocks",
      absolute.failed and len(absolute.blocking) == 1
      and "installed footprint" in absolute.blocking[0]
      and len(absolute.advisory) == 1)

b = single_entry(entry={"spike_exemption": {"pull_request": 57}})
absolute, regression, _ = budgets.run_gates(b, over)
check("a malformed spike exemption is not one, and lifts nothing", absolute.failed)

b = single_entry(entry={"spike_exemption": {**EXEMPTION, "figure": "cold start"}})
absolute, regression, _ = budgets.run_gates(b, over)
check("a spike exemption naming another entry's figure lifts nothing",
      absolute.failed)

b = single_entry(entry=EXEMPT)
absolute, regression, unmeasured = budgets.run_gates(b, {}, host="windows")
check("an exempt entry with no measurement is still unmeasured and blocking, the "
      "unmeasured clause being the budget-file gate's",
      unmeasured == [("SC-001 download size (windows)",
                      "no measurement was produced", True)])

b = single_entry(
    entry={"criterion": "SC-004", "name": "ten-tab memory", "figure": 150,
           "spike_exemption": {"pull_request": 57, "figure": "ten-tab memory"}},
    runner={"identity": ""},
)
absolute, regression, _ = budgets.run_gates(
    b, {("SC-004", "ten-tab memory", "windows"): 200.0}
)
check("an exempt hardware entry on an unpinned runner is advised once, as exempt",
      not absolute.failed and len(absolute.advisory) == 1
      and "exempt" in absolute.advisory[0])

# --- the release refusal ----------------------------------------------------
# A build produced while an exemption is unretired is not released or tagged.
# The release job runs --refuse-exemptions, which fails on any recorded
# exemption, well formed or not, and runs nothing else; the build job it
# depends on has already run the gates.

check("a file recording no exemption has none unretired",
      budgets.unretired_exemptions(budget_file()) == [])

found = budgets.unretired_exemptions(budget_file(entry=EXEMPT))
check("a recorded exemption is an unretired one, named by its entry",
      len(found) == 1 and found[0][0] == "SC-001 download size (windows)")

check("a malformed exemption is still a recorded one, and refused",
      len(budgets.unretired_exemptions(budget_file(entry={"spike_exemption": 57})))
      == 1)

SCRIPT = Path(__file__).resolve().parent / "check-budgets.py"
EXEMPT_TOML = """\
[[entry]]
criterion = "SC-002"
name = "cold start"
platform = "windows"
figure = 2000
unit = "ms"
status = "provisional"
baseline = 0.0
tolerance_pct = 0.0
spike_exemption = { pull_request = 57, figure = "cold start" }
"""


def refuse(toml_text):
    """Run the script as the release job runs it, over this file."""
    with tempfile.NamedTemporaryFile("w", suffix=".toml", delete=False) as handle:
        handle.write(toml_text)
        path = handle.name
    try:
        return subprocess.run(
            [sys.executable, str(SCRIPT), "--budgets", path, "--refuse-exemptions"],
            capture_output=True, text=True,
        )
    finally:
        os.unlink(path)


result = refuse(EXEMPT_TOML)
check("--refuse-exemptions exits non-zero on a recorded exemption",
      result.returncode == 1)
check("...naming the entry and the pull request",
      "SC-002 cold start (windows)" in result.stderr and "#57" in result.stderr)

retired = EXEMPT_TOML.replace(
    'spike_exemption = { pull_request = 57, figure = "cold start" }\n', ""
)
check("--refuse-exemptions exits zero once the exemption is retired",
      "spike_exemption" not in retired and refuse(retired).returncode == 0)

result = subprocess.run(
    [sys.executable, str(SCRIPT), "--refuse-exemptions"], capture_output=True, text=True
)
check("...and on the committed budget file", result.returncode == 0)

# --- the committed state, end to end ----------------------------------------
# The script as the workflow's blocking step runs it, on this host. Neither
# installer exists, so no download-size entry is measured on any host: an entry
# of this host's own platform reports that no measurement was produced, and one
# of the other platform that it is measured where its artefact is built. Both
# stand unmeasured with that reason until the installer each condition names is
# built, deferred by --allow-unmeasured and by nothing else.

host = budgets.host_platform()
here = budgets.measure_download_size(host)
result = subprocess.run(
    [sys.executable, str(SCRIPT), "--allow-unpinned-runners", "--allow-unmeasured"],
    capture_output=True, text=True,
)
check("the workflow's blocking step passes on the committed file",
      result.returncode == 0)
if here.megabytes is None:
    check("...measuring nothing, no installer artefact existing on this host",
          "measured: nothing on this" in result.stdout
          and here.reason in result.stdout)
for platform in PLATFORMS:
    line = next((each for each in result.stdout.splitlines()
                 if f"SC-001 download size ({platform})" in each), "")
    if platform == host and here.megabytes is not None:
        continue
    if platform == host:
        reason = "no measurement was produced"
    else:
        reason = f"a {platform} figure is measured on a {platform} host"
    check(f"...and the {platform} download-size entry is unmeasured with its "
          "reason, deferred",
          reason in line and "deferred by --allow-unmeasured" in line)

result = subprocess.run(
    [sys.executable, str(SCRIPT), "--allow-unpinned-runners"],
    capture_output=True, text=True,
)
check("without --allow-unmeasured the same file fails on them, an unmeasured "
      "entry not being a pass",
      result.returncode == 1
      and all(f"SC-001 download size ({p})" in result.stderr
              for p in PLATFORMS if p != host or here.megabytes is None))

for tree in TREES:
    shutil.rmtree(tree, ignore_errors=True)


# --- the tier set is closed --------------------------------------------------
# An absent [runners.*] table was indistinguishable from an unpinned one, so
# every hardware-dependent entry on that tier went advisory and a measurement
# many times its figure passed the build. The tiers are hardcoded for the same
# reason the entry list is: a file missing one cannot report its own omission.
b = budget_file()
del b["runners"]["tier2"]
g = file_gate(b)
check("an absent tier is refused", g.failed)
check("...and the failure names it",
      any("tier2 is not declared" in message for message in g.blocking))

# `X or True` is unconditionally true. This is the assertion that guarded the
# headline blocker, and it could not fail -- while the behaviour it named was
# still wrong: run_gates resolved an absent tier through the same
# `pinned.get(tier, False)` as an unpinned one and advised the breach.
b = budget_file()
del b["runners"]["tier2"]
absolute, _, _ = budgets.run_gates(
    b, {("SC-004", "ten-tab memory", "macos"): 9999.0}, host=None
)
check("a breach on an undeclared tier blocks", absolute.failed)
check("...and says the tier is undeclared, not unpinned",
      any("is not declared" in message for message in absolute.blocking))
check("...and is not merely advisory", not absolute.advisory)

# The contrast that gives the case its meaning: a tier that IS declared and is
# merely unpinned still defers, because that is a machine not yet bought.
b = budget_file(runner={"identity": "", "runner_label": "", "os_version": "",
                        "memory": "", "display_refresh": 0})
absolute, _, _ = budgets.run_gates(
    b, {("SC-004", "ten-tab memory", "windows"): 9999.0}, host=None
)
check("a breach on a declared-but-unpinned tier stays advisory",
      not absolute.failed and bool(absolute.advisory))

# Gate.tags must stay the same length as Gate.blocking on every path that
# mutates either, or defer_unpinned_runners' zip truncates to the shorter and
# the surplus lands in neither list.
g = budgets.Gate("x")
g.block("plain")
g.block("unpinned", tag=budgets.UNPINNED_TAG)
check("tags stay in step with blocking", len(g.blocking) == len(g.tags))
budgets.defer_unpinned_runners(g)
check("...and stay in step after a deferral", len(g.blocking) == len(g.tags))
check("...with the untagged failure kept", g.blocking == ["plain"])

# The invariant above holds of Gate whether or not main() honours it, so it
# cannot report the defect it was written for: main() appended to `blocking`
# directly, past Gate.block, and the surplus was dropped by the zip. This runs
# main() end to end and asserts the pair is still in step afterwards, which is
# the only place that appending shows.
def main_gate_lengths(path):
    """Run main() over a budget file and report each gate's final lengths.

    The gate objects are captured and read AFTER main() returns, not at the
    deferral. The append this exists to catch happens after the deferral, so
    measuring at deferral time reports the pair in step and proves nothing --
    which is what the first version of this test did.
    """
    captured = {}
    original = budgets.defer_unpinned_runners

    def spy(gate):
        captured[gate.name] = gate
        original(gate)

    budgets.defer_unpinned_runners = spy
    argv = sys.argv
    # WITHOUT --allow-unmeasured: the append this test exists to catch sits
    # inside `if blocking_unmeasured and not args.allow_unmeasured`, so passing
    # the flag skips the very branch under test. --allow-unpinned-runners stays,
    # because the deferral is what the zip runs over.
    sys.argv = ["check-budgets.py", "--budgets", str(path),
                "--allow-unpinned-runners"]
    try:
        budgets.main()
    except SystemExit:
        pass
    finally:
        sys.argv = argv
        budgets.defer_unpinned_runners = original
    return {name: (len(gate.blocking), len(gate.tags))
            for name, gate in captured.items()}


# Over the repository's own budget file, which is unpinned and unmeasured and
# so drives every branch that appends: the unpinned-runner failures that get
# deferred, and the unmeasured entries appended afterwards.
lengths = main_gate_lengths(Path(__file__).resolve().parent.parent / "budgets.toml")
check("main() leaves blocking and tags in step",
      bool(lengths) and all(b == t for b, t in lengths.values()))

# --- main()'s own decisions --------------------------------------------------
# Three of them, and each is a place where a gate can report a breach and not
# block on it. The gates themselves are exercised directly above; what is
# exercised here is main()'s assembly of them, which is a separate thing and
# was pinned only for the budget-file gate.

def as_toml(b):
    """The fixture dict as the file the script reads. There is no TOML writer
    in the standard library and the shapes here are three levels deep at most,
    so this writes exactly those shapes and nothing more."""

    def value(v):
        if isinstance(v, bool):
            return "true" if v else "false"
        if isinstance(v, (int, float)):
            return repr(v)
        if isinstance(v, dict):
            inner = ", ".join(f"{k} = {value(x)}" for k, x in v.items())
            return "{ " + inner + " }"
        return '"' + str(v).replace('"', '\\"') + '"'

    out = []
    # An empty enumeration is written as `wake = []`, which the gate requires:
    # a file with none must say so rather than omit the key. A writer that
    # emitted nothing for an empty list produced a file the gate refused, which
    # is the gate working.
    if not b.get("wake"):
        out.append("wake = []")
        out.append("")
    for tier, runner in b.get("runners", {}).items():
        out.append(f"[runners.{tier}]")
        out += [f"{k} = {value(v)}" for k, v in runner.items()]
        out.append("")
    for entry in b.get("entry", []):
        out.append("[[entry]]")
        out += [f"{k} = {value(v)}" for k, v in entry.items()]
        out.append("")
    for wake in b.get("wake", []):
        out.append("[[wake]]")
        out += [f"{k} = {value(v)}" for k, v in wake.items()]
        out.append("")
    return "\n".join(out) + "\n"


def run_main(b, *flags, host=None, megabytes=None):
    """main() over this fixture, with the host and its one measurement supplied.

    main() calls `host_platform()` and `measure_download_size()` with their
    defaults, so a test cannot reach the measuring path without standing in for
    both -- which is why nothing did, and why main()'s keying and its failing
    set were unpinned while `run_gates`' were not.
    """
    with tempfile.NamedTemporaryFile("w", suffix=".toml", delete=False) as handle:
        handle.write(as_toml(b))
        path = handle.name
    saved = (sys.argv, budgets.host_platform, budgets.measure_download_size,
             sys.stdout, sys.stderr)
    out, err = io.StringIO(), io.StringIO()
    budgets.host_platform = lambda *a, **k: host
    budgets.measure_download_size = lambda *a, **k: budgets.DownloadSize(
        host, megabytes, None if megabytes is not None else "no artefact")
    sys.argv = ["check-budgets.py", "--budgets", path, *flags]
    sys.stdout, sys.stderr = out, err
    try:
        code = budgets.main()
    except SystemExit as exit:
        code = exit.code
    finally:
        (sys.argv, budgets.host_platform, budgets.measure_download_size,
         sys.stdout, sys.stderr) = saved
        os.unlink(path)
    return code, out.getvalue(), err.getvalue()


# The file gate alone was pinned in main()'s failing set, so `absolute` or
# `regression` could be dropped from it and a gate would print its FAIL line and
# return zero -- a gate that reports and does not block, which is the defect
# class T012 exists for.
b = budget_file(entry={"figure": 20})
code, out, err = run_main(b, "--allow-unmeasured", host="windows", megabytes=400.0)
check("main() fails when only the absolute gate fails", code == 1)
check("...naming that gate", "FAILED: absolute" in err)
check("...having reported the breach", "400.000 MB exceeds 20 MB" in err)

b = budget_file(entry={"figure": 500, "baseline": 100.0, "tolerance_pct": 5.0})
code, out, err = run_main(b, "--allow-unmeasured", host="windows", megabytes=200.0)
check("main() fails when only the regression gate fails", code == 1)
check("...naming that gate", "FAILED: regression" in err)

b = budget_file(entry={"figure": 500})
code, out, err = run_main(b, "--allow-unmeasured", host="windows", megabytes=5.0)
check("main() passes when no gate fails", code == 0)

# main()'s measurement key. `run_gates` keying on the platform and
# measure_download_size declaring one are both pinned; main()'s assembly of the
# two into a key was not, and hardcoding a platform there is the T012 defect
# exactly -- one host's artefact judged against another platform's entry.
b = budget_file()
for entry in b["entry"]:
    if entry["criterion"] == "SC-001" and entry["name"] == "download size":
        entry["figure"] = 20 if entry["platform"] == "macos" else 9999
code, out, err = run_main(b, "--allow-unmeasured", host="macos", megabytes=400.0)
check("a macos measurement is judged against the macos entry",
      code == 1 and "download size (macos)" in err)
check("...and not against the windows entry",
      "download size (windows): measured" not in err)

# The flag's conditionality. Every other invocation passes
# --allow-unpinned-runners, so only the deferral's ON state was pinned: making
# it unconditional left the suite green, and that flag's conditionality is what
# bounds the advisory period the constitution's Principle II entry cites.
b = budget_file(runner={"identity": "", "runner_label": ""})
code, out, err = run_main(b, "--allow-unmeasured", host=None)
check("without the flag an unpinned runner blocks", code == 1 and "is not pinned" in err)
code, out, err = run_main(b, "--allow-unmeasured", "--allow-unpinned-runners", host=None)
check("...and with it the same file passes, the failure filed as advisory",
      code == 0 and "is not pinned" in out)

# A budget file the script cannot read is exit 2, not a traceback. `run_gates`
# already guards a misread ENTRY on the ground that a verdict over a misread file
# is a verdict on nothing; that guard sits far below `load_budgets` and cannot
# help when the file itself does not parse, so the traceback replaced the message
# one level up from where it was fixed.
for label, data in (
    ("malformed TOML", b"[not toml\n"),
    # Real undecodable bytes: 0xE9 is a valid latin-1 'e-acute' and an invalid
    # UTF-8 continuation byte. Encoding with errors="replace" produces plain
    # ASCII and proves nothing, which is what the first version of this did.
    ("bytes that are not UTF-8", b'name = "caf\xe9"\n'),
):
    with tempfile.NamedTemporaryFile("wb", suffix=".toml", delete=False) as handle:
        handle.write(data)
        path = handle.name
    result = subprocess.run(
        [sys.executable, str(SCRIPT), "--budgets", path],
        capture_output=True, text=True,
    )
    os.unlink(path)
    check(f"a budget file of {label} exits 2, no verdict having been reached",
          result.returncode == 2)
    check(f"...saying so rather than raising: {label}",
          "Cannot read the budget file" in result.stderr and "Traceback" not in result.stderr)

result = subprocess.run(
    [sys.executable, str(SCRIPT), "--budgets", "/nonexistent/budgets.toml"],
    capture_output=True, text=True,
)
check("a budget file that does not exist exits 2",
      result.returncode == 2 and "no such file" in result.stderr)

# The unmeasured branch's own undeclared-tier case, which had no test at all:
# reverting the branch left the whole suite green. An entry on a tier the file
# does not declare is BLOCKING-unmeasured, where one on a declared-but-unpinned
# tier is deferrable.
b = budget_file()
del b["runners"]["tier2"]
_, _, unmeasured = budgets.run_gates(b, {}, host=None)
macos = [row for row in unmeasured
         if "SC-004" in row[0] and "macos" in row[0]]
check("an unmeasured entry on an undeclared tier is blocking",
      bool(macos) and macos[0][2] is True)
check("...and says the tier is not declared",
      bool(macos) and "not declared" in macos[0][0])

b = budget_file(runner={"identity": "", "runner_label": "", "os_version": "",
                        "memory": "", "display_refresh": 0})
_, _, unmeasured = budgets.run_gates(b, {}, host=None)
windows = [row for row in unmeasured
           if "SC-004" in row[0] and "windows" in row[0]]
check("an unmeasured entry on a declared-but-unpinned tier is deferrable",
      bool(windows) and windows[0][2] is False)

# A platform no tier maps must not produce a message naming a tier called None.
b = budget_file()
b["entry"].append({**stated_entry(*DEFAULT), "criterion": "SC-004",
                   "name": "ten-tab memory", "platform": "linux"})
_, _, unmeasured = budgets.run_gates(b, {}, host=None)
linux = [row for row in unmeasured if "linux" in row[0]]
check("an unmapped platform names no tier called None",
      bool(linux) and "None" not in linux[0][0])
# The message was asserted and the verdict was not, so flipping the branch's
# blocking flag left the suite green -- and that flag is what main() reads to
# decide whether the run fails.
check("...and the entry still blocks", bool(linux) and linux[0][2] is True)

# The measured branch mirrors the unmeasured one, including on the message.
b = budget_file()
b["entry"].append({**stated_entry(*DEFAULT), "criterion": "SC-004",
                   "name": "ten-tab memory", "platform": "linux", "figure": 150})
absolute, _, _ = budgets.run_gates(
    b, {("SC-004", "ten-tab memory", "linux"): 999.0}, host=None
)
check("a breach on an unmapped platform blocks", absolute.failed)
check("...and names no tier called None",
      not any("None" in message for message in absolute.blocking))

# The isinstance guards in run_gates: a misread file must reach a verdict
# rather than a traceback. Nothing exercised run_gates with a non-table.
b = budget_file()
b["runners"] = "not a table"
try:
    budgets.run_gates(b, {}, host=None)
    check("run_gates survives a non-table runners", True)
except AttributeError:
    check("run_gates survives a non-table runners", False)
b = budget_file()
b["runners"]["tier2"] = "no machine yet"
try:
    absolute, _, _ = budgets.run_gates(
        b, {("SC-004", "ten-tab memory", "macos"): 9999.0}, host=None
    )
    check("run_gates survives a non-table runner", True)
    check("...and a breach on it blocks rather than advising", absolute.failed)
except AttributeError:
    check("run_gates survives a non-table runner", False)
    check("...and a breach on it blocks rather than advising", False)

# --- a negative baseline is a disabled regression gate, not a tight one -------
b = budget_file(entry={"criterion": "SC-004", "name": "ten-tab memory",
                       "platform": "windows", "baseline": -1.0})
g = file_gate(b)
check("a negative baseline is refused", g.failed)
check("...and the failure says why",
      any("is negative" in message for message in g.blocking))

# A figure of zero with a baseline of zero isolates the figure clause: the
# negative-baseline clause does not fire, and `baseline > figure` is false, so
# only the new clause can reject it. A figure of -5 with baseline 0.0 -- the
# case first written here -- tripped the pre-existing "baseline above its
# stated figure" clause instead, and so proved nothing about the new one.
b = budget_file(entry={"figure": 0, "baseline": 0.0})
g = file_gate(b)
check("a figure of zero is refused", g.failed)
check("...by the figure clause specifically",
      any("is not positive" in message for message in g.blocking))
check("a negative figure is refused",
      file_gate(budget_file(entry={"figure": -5, "baseline": -5})).failed)

# --- a deferral flag selects on a tag, never on the file's own text -----------
# --allow-unpinned-runners once moved every failure whose MESSAGE contained
# "is not pinned". Entry names are interpolated into failure messages, so an
# entry named for the phrase deferred its own unrelated failure -- and the
# closed-list clause, the one this script does not read off the file precisely
# so the file cannot weaken it, was turned off by a string inside the file.
unprocured = {"identity": "", "runner_label": "", "os_version": "",
              "memory": "", "display_refresh": 0}
b = budget_file(runner=unprocured)
b["entry"].append({**stated_entry(*DEFAULT), "name": "telemetry blob is not pinned"})
g = file_gate(b)
real = [m for m in g.blocking if budgets.UNPINNED in m and m.startswith("runner")]
smuggled = [m for m in g.blocking if "telemetry blob" in m]
check("the fixture has both a real unpinned failure and a smuggled name",
      len(real) == 1 and len(smuggled) == 1)
budgets.defer_unpinned_runners(g)
check("a deferral flag does not defer an entry named for the phrase",
      any("telemetry blob" in message for message in g.blocking))
check("...while the real unpinned failure is deferred",
      not any(m.startswith("runner") and budgets.UNPINNED in m
              for m in g.blocking))

print(f"\n{PASSED}/{PASSED + FAILED} passed")
sys.exit(1 if FAILED else 0)
