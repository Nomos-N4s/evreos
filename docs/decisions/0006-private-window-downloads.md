# Decision 0006: Private-window downloads record retention

- **Status**: Decided
- **Date**: 2026-09-25
- **Recorded**: 2026-09-25
- **Deciders**: Founder
- **Cite as**: `decisions/0006`

## Question

FR-007 requires that a private window leave no browsing trace on the local
machine after closure. FR-004 requires that the download list survive browser
restart and close/reopen, with each entry showing its destination path on disk.

When a member initiates and saves a download within a private browsing window,
two requirements pull in opposite directions:

1. FR-007 strictly forbids a browsing trace from persisting after a private
   window closes. Under FR-007a, the `source_address` of any transfer constitutes
   browsing history.
2. FR-004 requires downloads to be reviewed and to survive restart.

The specification does not settle whether a download started in a private window
leaves a record after that window closes, and data-model §1.9 identifies this as
an open gap requiring founder resolution.

## Decision

**A download started in a private window leaves NO persistent record in the
download store and produces no trace on disk after the private window closes or
the browser restarts. The file the member chose to save on disk remains intact
and is never deleted.**

Specifically:

1. **Transient Lifecycle**: While a private window is active, downloads started
   within that window may be tracked in memory (for transfer progress,
   cancellation, and completion feedback).
2. **Persistence Exclusion**: Private-window download records MUST NOT be
   written to the persistent `downloads.toml` file under the profile root at any
   time. Serialization filters out private entries as a strict write-path exclusion.
3. **Closure Purge**: When a private window closes, all transient download
   records associated with it are discarded from memory.
4. **Member File Preservation**: The saved file on disk at `destination_path` is
   the member's property, not browser internal state. Evreos does not delete,
   truncate, or alter the file when the private window closes or when private
   records are purged.

## Evidence

- `specs/001-evreos-v1/spec.md`:
  - **FR-007**: Private browsing windows MUST leave no browsing trace on the
    local machine after closure.
  - **FR-004**: Download review, progress tracking, cancellation, and restart
    survival.
  - **FR-007a**: Closed and exhaustive network egress enumeration; source addresses
    are classified as sensitive browsing history.
  - **SC-001**: Downloaded files are member data, excluded from the installed
    footprint.
- `specs/001-evreos-v1/data-model.md` §1.9 (Download):
  "This design takes the reading that the **record** does not persist while the
  **file the member chose to save** does, on the ground that a file the member
  directed Evreos to write is not a browsing trace."
- `specs/001-evreos-v1/tasks.md`: Task T044.

## Serves

- FR-007 (Private browsing window isolation and trace elimination)
- FR-004 (Download store and list management)
- FR-007a (Local data privacy and browsing history protection)
- SC-001 (Installed footprint accounting)
- Tasks T044, T052, and T081

## Consequences

- `crates/evreos-shell/src/store/downloads.rs` implements window-kind awareness,
  ensuring private download entries are never serialized to `downloads.toml` and
  can be purged on window close.
- `crates/evreos-shell/tests/download_store.rs` asserts that private window
  download records do not survive restart and do not persist in `downloads.toml`,
  while the file saved on disk is preserved intact.
- Preserves the core invariant that local disk files saved by explicit member
  direction belong to the member, while the browser retains zero browsing
  metadata.

## Corrections

None yet.
