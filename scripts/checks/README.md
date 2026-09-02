# Repository checks

This directory holds the checks that read the repository and need nothing
else: no release build, no pull request context, no pinned machine. Each is
one Python script that inspects the checkout — source, manifests, the graph
`cargo metadata` resolves, a committed list beside it — and exits non-zero
when the tree breaks a rule the specification or the constitution states.
`.github/workflows/checks.yml` runs every one of them on every pull request
and on every push to `main`, and adding a check never edits that workflow:
the workflow globs this directory, so a new check is one new file pair and
nothing else.

## What belongs here, and what does not

A check belongs here when it can reach its verdict on any hosted runner from
a checkout alone. The job runs on Linux, so a check reasons about the tree
and never about the machine: a rule over a file that matters only on one
platform is checked by reading the file, not by running on that platform.

Two kinds of check cannot meet that test, and each has a home already:

- **A measurement.** Anything that produces a figure — start time, memory,
  idle processor time, input latency — is meaningless on a hosted runner,
  because the machine is not the one the budget names and the figure is not
  reproducible under SC-013. Those jobs belong to the benchmark harness and
  run on the tier's pinned runner.
- **A check bound to another workflow's context.** `scripts/check-budgets.py`
  reads the release artefact `build.yml` builds, and
  `scripts/check-commit-hygiene.py` reads the pull request's full history,
  its real head rather than the merge ref, and its title and body, which
  `commit-hygiene.yml` writes to files for it. Each needs what its workflow
  supplies, so each stays in `scripts/` beside that workflow's job and is not
  a check in this directory's sense.

## The convention

One check is two files, and the names are fixed by the workflow's globs:

| File | Role |
| --- | --- |
| `check_<name>.py` | the check; exits non-zero when the tree breaks its rule |
| `test_check_<name>.py` | its tests; exit non-zero when the check misbehaves |
| `<name>.py` | a module two or more checks share, with `test_<name>.py` beside it |

The third row is `rustlex.py`, the one Rust scanner, and it is here because two
of them was the defect: a second, weaker copy grew up beside the first and made
two checks wrong at once. A shared module is production logic that reaches every
check importing it, so it carries tests on the same terms a check does. The
workflow enforces that pairing over every module here, not over the `check_`
prefix — while it keyed on the prefix, the one file that had already broken two
checks was the one file nothing required tests for.

- **Underscores, not hyphens.** The workflow runs
  `python3 scripts/checks/test_check_<name>.py`, which puts this directory
  first on `sys.path`, so a test imports its check with
  `import check_<name>`. The two scripts in `scripts/` are hyphenated and pay
  for it with an `importlib` preamble in each test; nothing here repeats that.
- **A check runs with no arguments.** The workflow passes none, so the
  default invocation is the whole check over the tree the file sits in.
  Resolve the repository root from the file's own path,
  `Path(__file__).resolve().parents[2]`, rather than from the working
  directory, so the check means the same thing run by hand from anywhere.
  Flags for local use are fine. A flag that defers part of the check is not,
  because the workflow would never pass it, and a deferral CI cannot see is
  an exemption. The deferrals `build.yml` carries are each stated beside the
  condition that retires them; a check here has no such channel, and that is
  deliberate.
- **Exit codes.** `0` is a pass, and a pass ends in one line saying so. `1`
  is a failure: the tree breaks the rule, and every breach is printed to
  standard error naming the file or entry that caused it, so the failure is
  locatable from the log alone, followed by one summary line. `2` means the
  check could not reach a verdict — an input it reads is missing, a tool it
  shells out to is absent. An unrun check is not a pass, so `2` fails the
  workflow exactly as `1` does; the distinction is for whoever reads the log.
- **Standard library only.** The runner installs nothing, so a check imports
  only what the Python the hosted image provides — 3.11 or later, which is
  what `tomllib` needs — and no third-party package. It may shell out to
  `git` and to `cargo`, both of which the workflow provides.
- **Tests are a plain script**, in the shape of `scripts/test_check_budgets.py`:
  a `check(name, condition)` helper that counts passes and prints each
  failure, one `N/M passed` line at the end, exit `1` when anything failed.
  No framework, because the two existing test scripts have this shape and one
  shape across the directory is easier to read than two. Every rule the check
  enforces has a failing case and a passing case, and the failing cases are
  the ones that matter: a check whose tests only prove it passes good input
  has not been shown to fail bad input.
- **A shared module is a last resort, not a layer.** Extract one only when a
  second check needs the identical logic and a second copy would be a defect
  rather than a duplication — which is what happened here. It is not a home for
  helpers one check finds convenient: a check reads as one file for a reason,
  and the further its logic sits from its rule, the harder the rule is to audit.
- **A committed list lives beside its check.** A check that needs an
  allowlist or a deny-list keeps it in this directory as a plain-text file —
  one entry per line, `#` starting a comment, blank lines ignored — read by
  the check and reviewed with it. Such a file may be empty, so that the first
  entry lands as a visible diff rather than as an edit to code. The workflow
  does not run it; only `check_*.py` and `test_*.py` match its globs.

## What the workflow does

`.github/workflows/checks.yml` has one job, `Repository checks`, on
`ubuntu-latest`, triggered by every pull request and every push to `main`.
Its steps, in order:

1. **Install the stable toolchain**, minimal profile, so a check that reads
   `cargo metadata` reads the graph one named toolchain resolves rather than
   whatever the runner image happens to carry. Nothing is compiled.
2. **Every module has its tests.** For each `<name>.py` that is not itself a
   `test_` file, `test_<name>.py` must exist. A check with no tests is a gate
   whose own behaviour nobody has proved, and a shared module with none is
   worse, because it is wrong in every check at once. The pair is enforced here
   rather than asked for in review. The rule is one-way: a test file with no
   module is not an error.
3. **Every check's own tests**, `python3 scripts/checks/test_*.py`, each run
   to completion; then
4. **Every check**, `python3 scripts/checks/check_*.py`, each run to
   completion.

Steps 3 and 4 run every file even after one fails and fail the step at the
end, so one red file does not hide another. Step 4 is not reached if step 3
failed: a check whose tests fail has no verdict worth reading. An empty
directory passes: steps 2 to 4 print there is nothing to run, which is what let
harness land before its first check. Two have since landed beside it, the crate
policy and the engine prohibition, so those steps now run them.

The job name is load-bearing. A required-status-check setting on `main`
refers to a job by name, so renaming it would un-require every check here at
once, and silently. It changes only together with that setting.

## Running a check by hand

From the repository root:

```sh
python3 scripts/checks/test_check_<name>.py
python3 scripts/checks/check_<name>.py
```

or everything the workflow would run:

```sh
shopt -s nullglob
for f in scripts/checks/test_*.py scripts/checks/check_*.py; do
    python3 "$f" || echo "FAILED  $f" >&2
done
```

## What a green run means

That every rule someone thought to automate held on this tree. It is not a
review and does not stand in for one: the constitution's Development Workflow
section says green automated checks test what someone thought to automate,
and the review exists for what nobody did. A check here narrows what review
has to carry; it does not shorten the review.
