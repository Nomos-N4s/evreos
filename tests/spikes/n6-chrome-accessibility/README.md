# N6 chrome accessibility spike

The spike host for T015 (`specs/001-evreos-v1/tasks.md`): one winit window
holding one embedded platform webview — wry, binding the web runtime the
operating system provides, per ADR-0001 — beside a minimal AccessKit chrome
tree, so the join between the chrome and content accessibility trees can be
measured rather than argued about.

## What this is

- `src/host.rs` — the reusable scaffold: a winit 0.30 `ApplicationHandler`
  that creates one window, embeds one wry webview as a child with managed
  bounds (resized with the window), and exposes `ChromeFront`, the seam a
  later chrome renderer plugs into. Debug builds assert on every callback
  that UI calls happen on the event-loop thread. This split is deliberate
  groundwork for the later real window work: the host is the half a renderer
  candidate would keep.
- `src/fronts/accesskit_min.rs` — the minimal AccessKit front: a three-node
  chrome tree (window root, a labelled address text input, a button)
  published through `accesskit_winit` beside the content webview, with Tab
  traversal wired so focus order can be exercised. It logs to stderr, with a
  `[n6]` prefix, the cross-tree labelled-by attempt — including the
  structural reason the reference cannot be expressed: an `accesskit::NodeId`
  resolves inside the adapter's own tree, the webview's content tree is the
  platform tree the system webview publishes itself, and accesskit 0.25.0
  has no property that references a foreign platform tree — and every focus
  traversal as it happens.
- The webview renders a small inline page (a heading, a labelled text input,
  a button) so the content tree has nodes to join against. Nothing is
  fetched from the network.

Dependency versions are pinned exactly in `Cargo.toml` — winit 0.30.13,
wry 0.53.5, accesskit 0.25.0, accesskit_winit 0.34.0 — because a measurement
against a drifting dependency set is a measurement of nothing.

## Running it on tier 1

```
cargo run -p n6-chrome-accessibility
```

A window opens with an unpainted chrome strip across the top and the content
page below it. The chrome strip has no drawn widgets — its UI exists as the
AccessKit tree, which is the point of the spike. Tab moves chrome focus from
the address input to the button; Tab again hands focus across the tree
boundary into the webview, where traversal belongs to the system webview.
Narrator (Win+Ctrl+Enter) or NVDA read the chrome tree; everything the front
attempts is on stderr.

On platforms outside tiers 1 and 2 the binary prints why it is gated and
exits 0: the platform dependencies are declared only for Windows and macOS
targets, so the Linux CI runner builds the stub and none of them.

## What T015 measures with it

T015 (`specs/001-evreos-v1/tasks.md`) drives this host with Narrator and NVDA
on tier 1 and VoiceOver on tier 2, and records three things: whether a chrome
node can be labelled by a content node across the host and web-view trees,
whether focus crosses the boundary in reading order, and what each screen
reader announces there. A negative result eliminates the drawn-chrome
candidate under Principle X before ADR-0002 (T016) chooses what renders the
chrome.

## Where the record lands

Not here, and not in this pull request. The measurement record lands at
`docs/measurements/n6-chrome-accessibility.md` under the terms of
`docs/decisions/0004-interim-spike-hardware.md`: measured on the named
interim instrument with the provenance block that decision requires, and
with the pinned-runner re-runs owed rather than replaced.

## What this costs the workspace

`Cargo.lock` grows from three local path packages to a resolved third-party
graph. The dependencies are this test-tree workspace member's: nothing on
the release path depends on this crate, it feeds no release artefact, and
the shell's release binary is unchanged. Quickstart A0's statement that the
workspace pulls no third-party crate at all is superseded for the workspace
by this change while remaining true of the shell's release binary; amending
that wording belongs to the doc-sync that follows the merged spike, not to
this directory.
