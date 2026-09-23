# Instructions for Autonomous Agents

This file documents the mandatory conventions, rules, and invariants governing all changes in this repository. Autonomous agents (Google Jules, Claude Code, Antigravity, Cursor, etc.) must follow these instructions.

---

## 1. Commit Authorship & Hygiene (Principle I)

The repository's commit hygiene checker (`scripts/check-commit-hygiene.py`) gates every pull request and push to `main`. Every commit MUST satisfy:

1. **Author & Committer Identity**:
   - Author: `xcoder-es <capintobe@gmail.com>` or `xcoder-es <291264330+xcoder-es@users.noreply.github.com>`.
   - Never commit under an unapproved identity or third-party address.
2. **Conventional Commits**:
   - Commit subjects must be formatted as: `type(scope): lowercase imperative subject`
   - Allowed types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`.
3. **Mandatory Issue Reference**:
   - Every commit must link to its issue on its own line in the commit body:
     - For Linear tasks: `Refs CAR-xxx` or `Closes CAR-xxx` (e.g. `Refs CAR-143`)
     - For GitHub issues: `Refs #xxx` or `Closes #xxx` (e.g. `Refs #62`)
4. **Strict Prohibition on AI Attribution & Footers**:
   - **No** `Co-Authored-By:` trailers of any kind (human or AI).
   - **No** generator footers or tool links (e.g., `Generated with ...`, `Co-authored-by ...`, robot emoji `🤖`).
   - **No** session, run, or conversation identifiers anywhere in commit messages, PR titles, or PR bodies.
5. **Atomic Commits & Branch History**:
   - When fixing a commit on a branch, **amend the existing commit** (`git commit --amend`) or rebase/squash rather than stacking new commits on top of an invalid commit. CI checks **every commit** in `origin/main..HEAD`.

---

## 2. Code Quality & Pre-Push Verification

Before pushing to any branch:

```sh
# 1. Format check
cargo fmt --all --check

# 2. Workspace compilation and clippy
cargo check --workspace
cargo clippy --all-targets --all-features -- -D warnings

# 3. Unit and integration test suite
cargo test --all

# 4. Commit hygiene verification
python3 scripts/check-commit-hygiene.py --range origin/main..HEAD
```

---

## 3. Core Architectural Invariants

- **Unsafe Code Forbidden**: `#![forbid(unsafe_code)]` is strictly enforced across the entire workspace.
- **Engine Seam**: The shell drives rendering through the `Engine` trait defined in `crates/evreos-engine`. No bundled Chromium, CEF, or Electron runtimes are permitted.
- **Privacy & Money**: All financial and ledger operations are server-side; browsing history must never leave the local machine.
