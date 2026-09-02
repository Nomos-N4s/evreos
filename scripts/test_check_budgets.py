#!/usr/bin/env python3
"""Tests for the budget gate. The gate is CI's authority to fail a build, so
its own behaviour is checked rather than assumed."""
import datetime
import importlib.util
import io
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


def budget_file(**overrides):
    entry = {
        "criterion": "SC-001",
        "name": "download size",
        "platform": "windows",
        "figure": 20,
        "unit": "MB",
        "status": "ratified",
        "baseline": 1.0,
        "tolerance_pct": 0.0,
    }
    entry.update(overrides.pop("entry", {}))
    runner = {
        "platform": "windows",
        "model": "a laptop",
        "display_refresh": 60,
        "runner_label": "evreos-tier1",
        "identity": "abc123",
    }
    runner.update(overrides.pop("runner", {}))
    return {
        "runners": {"tier1": runner},
        "entry": [entry],
        "wake": overrides.pop("wake", []),
    }


def file_gate(b):
    g = budgets.Gate("f")
    budgets.check_budget_file(b, g)
    return g


# Well-formed sub-tables, for the cases that vary one field of each.
EXEMPTION = {"pull_request": 57, "figure": "cold start"}
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
          "baseline": 0.0, "unit": "percent-of-core"}
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
# Pinned means identity, runner_label and display_refresh all recorded. They
# are written together when the machine is procured, so any of them missing
# is the same unpinned state, reported once and deferred by one flag.

check("a runner with no runner_label is not pinned",
      file_gate(budget_file(runner={"runner_label": ""})).failed)

check("a runner with display_refresh 0 is not pinned",
      file_gate(budget_file(runner={"display_refresh": 0})).failed)

b = budget_file()
del b["runners"]["tier1"]["runner_label"]
del b["runners"]["tier1"]["display_refresh"]
check("a runner block without the two fields is not pinned", file_gate(b).failed)

unprocured = {"identity": "", "runner_label": "", "display_refresh": 0}
g = file_gate(budget_file(runner=unprocured))
check("a runner missing all three is reported once, not three times",
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

# --- founder_decision and cross_check_margin ---------------------------------

check("a founder decision citing a record passes",
      not file_gate(budget_file(entry={"founder_decision": "decisions/0001"})).failed)

check("an empty founder decision fails",
      file_gate(budget_file(entry={"founder_decision": ""})).failed)

check("a founder decision that is not a string fails",
      file_gate(budget_file(entry={"founder_decision": 1})).failed)

TEN_TAB = {"criterion": "SC-004", "name": "ten-tab memory", "figure": 150}

check("a cross-check margin on SC-004 passes",
      not file_gate(budget_file(entry={**TEN_TAB, "cross_check_margin": 2.0})).failed)

check("a cross-check margin on a criterion other than SC-004 fails",
      file_gate(budget_file(entry={"cross_check_margin": 2.0})).failed)

check("a cross-check margin that is not a number fails",
      file_gate(budget_file(entry={**TEN_TAB, "cross_check_margin": "2"})).failed)

# --- spike_exemption --------------------------------------------------------

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

# --- baseline_reset ---------------------------------------------------------

check("a complete baseline reset passes",
      not file_gate(budget_file(entry={"baseline_reset": dict(RESET)})).failed)

for field in RESET:
    record = dict(RESET)
    del record[field]
    check(f"a baseline reset lacking {field} fails",
          file_gate(budget_file(entry={"baseline_reset": record})).failed)


def reset(**fields):
    return budget_file(entry={"baseline_reset": {**RESET, **fields}})


check("a baseline reset whose date is a string fails",
      file_gate(reset(date="2026-09-01")).failed)

check("a baseline reset naming an empty decision fails",
      file_gate(reset(founder_decision="")).failed)

check("a baseline reset with a cost that is not a number fails",
      file_gate(reset(measured_cost="1.5 MB")).failed)

# --- the wake enumeration ---------------------------------------------------

check("an empty enumeration passes", not file_gate(budget_file(wake=[])).failed)

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

# An absent enumeration is not yet a failure. The clause that fails on it lands
# with the enumeration's semantics -- the per-wake cap and the hourly sum --
# and is recorded here as not enforced rather than left to be assumed.
b = budget_file()
del b["wake"]
check("an absent enumeration is not yet a failure", not file_gate(b).failed)

# --- the repository's own budget file ---------------------------------------
# The file as committed reads under the schema and fails only on what it says
# it fails on: two runners awaiting procurement. Beyond that it carries the
# eighteen entries the preamble closes over, Q-E9's split of them, and SC-005's
# enumeration. The gate clauses that would fail on their absence land
# separately, so until they do the file's completeness is proved here.

real = budgets.load_budgets(budgets.REPO / "budgets.toml")
g = file_gate(real)
check("the committed budget file fails only on its two unpinned runners",
      len(g.blocking) == 2 and all(budgets.UNPINNED in m for m in g.blocking))
budgets.defer_unpinned_runners(g)
check("...and passes once that is deferred", not g.failed)
check("every committed entry states its unit",
      all(e.get("unit") == budgets.UNIT_OF[e["criterion"]] for e in real["entry"]))

# The closed list: nine entries per platform, eighteen in all, and no other.
NAMES = {
    "SC-001": ("download size", "installed footprint"),
    "SC-002": ("warm start", "cold start"),
    "SC-004": ("ten-tab memory",),
    "SC-005": ("60-minute window", "wake-free 1-second sample"),
    "SC-006": ("tab switch", "address-field keystroke"),
}
CLOSED_LIST = {
    (criterion, name, platform)
    for criterion, names in NAMES.items()
    for name in names
    for platform in ("windows", "macos")
}
committed = {(e["criterion"], e["name"], e["platform"]) for e in real["entry"]}
check("the committed budget file carries exactly the eighteen entries the "
      "preamble closes over",
      len(real["entry"]) == 18 and committed == CLOSED_LIST)

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
by_key = {(e["criterion"], e["name"], e["platform"]): e for e in real["entry"]}
check("the 60-minute window condition names 18 s of processor time",
      all("18 s" in by_key[("SC-005", "60-minute window", p)]["condition"]
          for p in ("windows", "macos")))
check("the wake-free sample condition names 5 ms of processor time",
      all("5 ms" in by_key[("SC-005", "wake-free 1-second sample", p)]["condition"]
          for p in ("windows", "macos")))

# A tolerance is justified by measured variation, so an entry no measurement
# has written a baseline for cannot have one.
check("an unmeasured committed entry declares no tolerance",
      all(e["tolerance_pct"] == 0.0 for e in real["entry"] if e["baseline"] == 0.0))

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

# --- absolute and regression gates -------------------------------------------

b = budget_file()
absolute, regression, unmeasured = budgets.run_gates(
    b, {("SC-001", "download size"): 25.0}
)
check("exceeding a non-hardware figure blocks the absolute gate", absolute.failed)
check("...and the verdict is stated in the entry's unit",
      "25.000 MB exceeds 20 MB" in absolute.blocking[0])

b = budget_file()
absolute, regression, _ = budgets.run_gates(b, {("SC-001", "download size"): 1.0})
check("meeting the figure passes the absolute gate", not absolute.failed)

# An undeclared tolerance is zero, not unbounded: the opposite reading lets an
# entry disable its own regression gate by omitting a field.
b = budget_file()
del b["entry"][0]["tolerance_pct"]
absolute, regression, _ = budgets.run_gates(b, {("SC-001", "download size"): 1.001})
check("an undeclared tolerance is zero, not unbounded", regression.failed)

b = budget_file(entry={"tolerance_pct": 5.0})
absolute, regression, _ = budgets.run_gates(b, {("SC-001", "download size"): 1.04})
check("a regression inside the declared tolerance passes", not regression.failed)

b = budget_file(entry={"tolerance_pct": 5.0})
absolute, regression, _ = budgets.run_gates(b, {("SC-001", "download size"): 1.06})
check("a regression outside the declared tolerance blocks", regression.failed)
check("...stated in the entry's unit",
      "1.060 MB is worse than baseline 1.0 MB" in regression.blocking[0])

# A millisecond entry is compared and reported in milliseconds.
b = budget_file(
    entry={"criterion": "SC-002", "name": "warm start", "figure": 800, "unit": "ms",
           "baseline": 700.0, "tolerance_pct": 0.0},
)
absolute, regression, _ = budgets.run_gates(b, {("SC-002", "warm start"): 900.0})
check("a millisecond breach blocks on a pinned runner",
      absolute.failed and regression.failed)
check("...and is stated in ms", "900.000 ms exceeds 800 ms" in absolute.blocking[0])

# An entry the budget-file gate has already refused is not compared: a
# measurement in the criterion's unit against a figure in another is two
# quantities, and a verdict over them would be a verdict on nothing.
over = {("SC-001", "download size"): 25.0}
b = budget_file(entry={"unit": "ms"})
absolute, regression, unmeasured = budgets.run_gates(b, over)
check("a figure in a unit its criterion does not state gets no verdict",
      not absolute.failed and not absolute.advisory and not regression.failed
      and not unmeasured)

b = budget_file(entry={"figure": "20"})
absolute, regression, unmeasured = budgets.run_gates(b, over)
check("a figure that is not a number gets no verdict rather than a crash",
      not absolute.failed and not unmeasured)

# A hardware-dependent entry's absolute gate is advisory until its runner is
# pinned; its regression gate is not, because it compares a machine to itself.
b = budget_file(
    entry={"criterion": "SC-004", "name": "ten-tab memory", "figure": 150},
    runner={"identity": ""},
)
absolute, regression, _ = budgets.run_gates(b, {("SC-004", "ten-tab memory"): 200.0})
check("an unpinned hardware entry's absolute breach is advisory", not absolute.failed)
check("...and is still reported", len(absolute.advisory) == 1)

b = budget_file(
    entry={"criterion": "SC-004", "name": "ten-tab memory", "figure": 150},
    runner={"runner_label": ""},
)
absolute, regression, _ = budgets.run_gates(b, {("SC-004", "ten-tab memory"): 200.0})
check("a runner with no label is unpinned for the absolute gate too",
      not absolute.failed)

b = budget_file(
    entry={"criterion": "SC-004", "name": "ten-tab memory", "figure": 150},
    runner={"identity": "pinned-1"},
)
absolute, regression, _ = budgets.run_gates(b, {("SC-004", "ten-tab memory"): 200.0})
check("a pinned hardware entry's absolute breach blocks", absolute.failed)

b = budget_file(
    entry={
        "criterion": "SC-004",
        "name": "ten-tab memory",
        "figure": 150,
        "baseline": 100.0,
        "tolerance_pct": 0.0,
    },
    runner={"identity": ""},
)
absolute, regression, _ = budgets.run_gates(b, {("SC-004", "ten-tab memory"): 120.0})
check("regression blocks even on an unpinned runner", regression.failed)

# An unmeasured entry is not a pass.
b = budget_file()
absolute, regression, unmeasured = budgets.run_gates(b, {})
check("an unmeasured entry is reported, not silently passed", len(unmeasured) == 1)

# --- unmeasured entries ------------------------------------------------------
# The docstring promises "an unmeasured entry is not a pass". These cover the
# case where getting it wrong is silently permissive.

b = budget_file()
absolute, regression, unmeasured = budgets.run_gates(b, {})
check("a non-hardware entry with no measurement is marked BLOCKING",
      unmeasured == [("SC-001 download size (windows)", "BLOCKING")])

b = budget_file(
    entry={"criterion": "SC-004", "name": "ten-tab memory", "figure": 150},
    runner={"identity": ""},
)
absolute, regression, unmeasured = budgets.run_gates(b, {})
check("an unmeasured hardware entry with no pinned runner is not blocking",
      unmeasured and unmeasured[0][1] != "BLOCKING")

b = budget_file(
    entry={"criterion": "SC-004", "name": "ten-tab memory", "figure": 150},
    runner={"identity": "pinned-1"},
)
absolute, regression, unmeasured = budgets.run_gates(b, {})
check("an unmeasured hardware entry WITH a pinned runner is blocking",
      unmeasured and unmeasured[0][1] == "BLOCKING")

b = budget_file()
absolute, regression, unmeasured = budgets.run_gates(
    b, {("SC-001", "download size"): 1.0}
)
check("a measured entry is not reported unmeasured", unmeasured == [])

print(f"\n{PASSED}/{PASSED + FAILED} passed")
sys.exit(1 if FAILED else 0)
