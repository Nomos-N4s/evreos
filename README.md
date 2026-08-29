# evreos

This repository is set up for [Spec-Driven Development](https://github.com/github/spec-kit) using GitHub's **Spec Kit** with the **Claude Code** integration.

## How it works

Instead of writing code first, features start from a specification. Spec Kit installs a set of Claude Code skills that walk each feature through a structured pipeline:

| Step | Command | What it does |
| --- | --- | --- |
| 1 | `/speckit-constitution` | Establish the project's governing principles (edit `.specify/memory/constitution.md`) |
| 2 | `/speckit-specify` | Create a baseline specification for a feature |
| 3 | `/speckit-plan` | Turn the spec into an implementation plan |
| 4 | `/speckit-tasks` | Generate actionable, ordered tasks from the plan |
| 5 | `/speckit-implement` | Execute the tasks |
| 6 | `/speckit-converge` | Assess the codebase and append remaining work as tasks |

Optional quality gates:

- `/speckit-clarify` — structured questions to de-risk ambiguity (run before `/speckit-plan`)
- `/speckit-checklist` — quality checklists for requirements (run after `/speckit-plan`)
- `/speckit-analyze` — cross-artifact consistency report (run after `/speckit-tasks`, before `/speckit-implement`)
- `/speckit-taskstoissues` — turn generated tasks into GitHub issues

## Repository layout

```
.claude/skills/       Claude Code skills for the spec-kit pipeline
.specify/
  memory/             Project constitution
  scripts/bash/       Helper scripts used by the skills
  templates/          Spec / plan / tasks / checklist templates
  workflows/          Spec Kit workflow definition
specs/                Created per-feature by /speckit-specify (one dir per feature)
```

## Contributing checks

Every commit must be authored by `xcoder-es <capintobe@gmail.com>`, and nothing in the
repository may attribute authorship or assistance to an AI or generator tool. Naming a tool
the project integrates with is description, not attribution, and is allowed.

A GitHub Actions job (`.github/workflows/commit-hygiene.yml`) is the enforcement. It checks
every commit in a pull request — author, committer, AI attribution trailers, generator
footers, Conventional Commits subject, and an issue reference — plus the pull request's own
title and body, which is where generator footers land and where no git hook can reach.

The automated check covers what is mechanical and unambiguous: fixed footers and git
trailers that tooling appends. It deliberately does **not** try to detect attribution
written as free English. That was attempted and removed: in a browser project `cursor` and
`AI panel` are everyday words, so the patterns rejected legitimate commits more often than
they caught anything. Prose attribution stays forbidden by the rules above and is caught in
review, not by this script.

The local hook checks a commit message only: attribution, subject format and issue
reference. It cannot check the pull request title or body, so it is faster feedback rather
than the same coverage. Enable it once per clone:

```sh
git config core.hooksPath .githooks
```

Run the check by hand against a branch, and run the checker's own tests, with:

```sh
python3 scripts/check-commit-hygiene.py --range main..HEAD
python3 scripts/test_check_commit_hygiene.py
```

## Getting started

1. Open this repository in [Claude Code](https://claude.ai/code).
2. Run `/speckit-constitution` to set the project principles.
3. Run `/speckit-specify <describe your feature>` to start the first feature.

The Spec Kit CLI itself is not required day-to-day, but you can refresh or manage the installation with:

```sh
uvx --from git+https://github.com/github/spec-kit.git specify --help
```
