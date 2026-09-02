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
    return {
        "platform": platform,
        "model": "a laptop",
        "display_refresh": 60,
        "runner_label": f"evreos-{tier}",
        "identity": f"{tier}-abc123",
    }


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

# --- absolute and regression gates -------------------------------------------
# These read entries one at a time and report each, so they are exercised on a
# one-entry file; the budget-file gate, which would fail that file on the
# seventeen it lacks, is not run on it.

b = single_entry()
absolute, regression, unmeasured = budgets.run_gates(
    b, {("SC-001", "download size"): 25.0}
)
check("exceeding a non-hardware figure blocks the absolute gate", absolute.failed)
check("...and the verdict is stated in the entry's unit",
      "25.000 MB exceeds 20 MB" in absolute.blocking[0])

b = single_entry()
absolute, regression, _ = budgets.run_gates(b, {("SC-001", "download size"): 1.0})
check("meeting the figure passes the absolute gate", not absolute.failed)

# An undeclared tolerance is zero, not unbounded: the opposite reading lets an
# entry disable its own regression gate by omitting a field.
b = single_entry()
del b["entry"][0]["tolerance_pct"]
absolute, regression, _ = budgets.run_gates(b, {("SC-001", "download size"): 1.001})
check("an undeclared tolerance is zero, not unbounded", regression.failed)

b = single_entry(entry={"tolerance_pct": 5.0})
absolute, regression, _ = budgets.run_gates(b, {("SC-001", "download size"): 1.04})
check("a regression inside the declared tolerance passes", not regression.failed)

b = single_entry(entry={"tolerance_pct": 5.0})
absolute, regression, _ = budgets.run_gates(b, {("SC-001", "download size"): 1.06})
check("a regression outside the declared tolerance blocks", regression.failed)
check("...stated in the entry's unit",
      "1.060 MB is worse than baseline 1.0 MB" in regression.blocking[0])

# A millisecond entry is compared and reported in milliseconds.
b = single_entry(
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
absolute, regression, _ = budgets.run_gates(b, {("SC-004", "ten-tab memory"): 200.0})
check("an unpinned hardware entry's absolute breach is advisory", not absolute.failed)
check("...and is still reported", len(absolute.advisory) == 1)

b = single_entry(
    entry={"criterion": "SC-004", "name": "ten-tab memory", "figure": 150},
    runner={"runner_label": ""},
)
absolute, regression, _ = budgets.run_gates(b, {("SC-004", "ten-tab memory"): 200.0})
check("a runner with no label is unpinned for the absolute gate too",
      not absolute.failed)

b = single_entry(
    entry={"criterion": "SC-004", "name": "ten-tab memory", "figure": 150},
    runner={"identity": "pinned-1"},
)
absolute, regression, _ = budgets.run_gates(b, {("SC-004", "ten-tab memory"): 200.0})
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
absolute, regression, _ = budgets.run_gates(b, {("SC-004", "ten-tab memory"): 120.0})
check("regression blocks even on an unpinned runner", regression.failed)

# An unmeasured entry is not a pass.
b = single_entry()
absolute, regression, unmeasured = budgets.run_gates(b, {})
check("an unmeasured entry is reported, not silently passed", len(unmeasured) == 1)

# --- unmeasured entries ------------------------------------------------------
# The docstring promises "an unmeasured entry is not a pass". These cover the
# case where getting it wrong is silently permissive.

b = single_entry()
absolute, regression, unmeasured = budgets.run_gates(b, {})
check("a non-hardware entry with no measurement is marked BLOCKING",
      unmeasured == [("SC-001 download size (windows)", "BLOCKING")])

b = single_entry(
    entry={"criterion": "SC-004", "name": "ten-tab memory", "figure": 150},
    runner={"identity": ""},
)
absolute, regression, unmeasured = budgets.run_gates(b, {})
check("an unmeasured hardware entry with no pinned runner is not blocking",
      unmeasured and unmeasured[0][1] != "BLOCKING")

b = single_entry(
    entry={"criterion": "SC-004", "name": "ten-tab memory", "figure": 150},
    runner={"identity": "pinned-1"},
)
absolute, regression, unmeasured = budgets.run_gates(b, {})
check("an unmeasured hardware entry WITH a pinned runner is blocking",
      unmeasured and unmeasured[0][1] == "BLOCKING")

b = single_entry()
absolute, regression, unmeasured = budgets.run_gates(
    b, {("SC-001", "download size"): 1.0}
)
check("a measured entry is not reported unmeasured", unmeasured == [])

print(f"\n{PASSED}/{PASSED + FAILED} passed")
sys.exit(1 if FAILED else 0)
