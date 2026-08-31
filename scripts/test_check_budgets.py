#!/usr/bin/env python3
"""Tests for the budget gate. The gate is CI's authority to fail a build, so
its own behaviour is checked rather than assumed."""
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
        "figure_mb": 20,
        "status": "ratified",
        "baseline_mb": 1.0,
        "tolerance_pct": 0.0,
    }
    entry.update(overrides.pop("entry", {}))
    runner = {"platform": "windows", "model": "a laptop", "identity": "abc123"}
    runner.update(overrides.pop("runner", {}))
    return {"runners": {"tier1": runner}, "entry": [entry]}


# --- budget-file gate ---------------------------------------------------------

g = budgets.Gate("f")
budgets.check_budget_file(budget_file(), g)
check("a complete file passes the budget-file gate", not g.failed)

g = budgets.Gate("f")
budgets.check_budget_file(budget_file(runner={"identity": ""}), g)
check("an unpinned runner fails the budget-file gate", g.failed)

g = budgets.Gate("f")
budgets.check_budget_file(budget_file(runner={"identity": "   "}), g)
check("whitespace is not an identity", g.failed)

g = budgets.Gate("f")
budgets.check_budget_file(budget_file(entry={"baseline_mb": 25.0}), g)
check("a baseline above the stated figure fails", g.failed)

g = budgets.Gate("f")
budgets.check_budget_file(budget_file(entry={"baseline_mb": 20.0}), g)
check("a baseline equal to the figure is permitted", not g.failed)

g = budgets.Gate("f")
budgets.check_budget_file(budget_file(entry={"tolerance_pct": 5.1}), g)
check("a tolerance above the cap fails", g.failed)

g = budgets.Gate("f")
budgets.check_budget_file(budget_file(entry={"tolerance_pct": 5.0}), g)
check("a tolerance at the cap is permitted", not g.failed)

g = budgets.Gate("f")
budgets.check_budget_file(budget_file(entry={"status": "ratified-ish"}), g)
check("an unknown status fails", g.failed)

g = budgets.Gate("f")
budgets.check_budget_file({"runners": {}, "entry": []}, g)
check("an empty file fails rather than vacuously passing", g.failed)

g = budgets.Gate("f")
b = budget_file()
b["entry"].append(dict(b["entry"][0]))
budgets.check_budget_file(b, g)
check("a duplicated entry fails", g.failed)

# --- absolute and regression gates -------------------------------------------

b = budget_file()
absolute, regression, unmeasured = budgets.run_gates(
    b, {("SC-001", "download size"): 25.0}
)
check("exceeding a non-hardware figure blocks the absolute gate", absolute.failed)

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

# A hardware-dependent entry's absolute gate is advisory until its runner is
# pinned; its regression gate is not, because it compares a machine to itself.
b = budget_file(
    entry={"criterion": "SC-004", "name": "ten-tab memory", "figure_mb": 150},
    runner={"identity": ""},
)
absolute, regression, _ = budgets.run_gates(b, {("SC-004", "ten-tab memory"): 200.0})
check("an unpinned hardware entry's absolute breach is advisory", not absolute.failed)
check("...and is still reported", len(absolute.advisory) == 1)

b = budget_file(
    entry={"criterion": "SC-004", "name": "ten-tab memory", "figure_mb": 150},
    runner={"identity": "pinned-1"},
)
absolute, regression, _ = budgets.run_gates(b, {("SC-004", "ten-tab memory"): 200.0})
check("a pinned hardware entry's absolute breach blocks", absolute.failed)

b = budget_file(
    entry={
        "criterion": "SC-004",
        "name": "ten-tab memory",
        "figure_mb": 150,
        "baseline_mb": 100.0,
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

print(f"\n{PASSED}/{PASSED + FAILED} passed")
sys.exit(1 if FAILED else 0)
