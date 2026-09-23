# Measurement N6: Chrome & Content Accessibility Tree Composition

- **Task**: T015 (`specs/001-evreos-v1/tasks.md`) / CAR-137
- **Date**: 2026-09-23
- **Status**: Measured on Tier 1 Interim Instrument; Tier 2 Unmeasured

## Provenance & Instrument Tuple

This measurement record is governed by `decisions/0004`:

- **Decision Record**: `decisions/0004` (Interim hardware for chrome spikes)
- **Tier 1 Instrument Tuple**:
  - **Machine**: Acer Predator PH315-53 (notebook)
  - **CPU**: Intel Core i7-10750H (6 cores, 12 threads, 10th Gen)
  - **Memory**: 8 GB DDR4
  - **Storage**: NVMe SSD
  - **Display**: Internal 1920x1080 panel (driven at 60 Hz)
  - **Operating System**: Windows 11 Pro, version 25H2, build 26200.9168
  - **Web Runtime**: Microsoft Edge WebView2 Runtime, evergreen channel, 152.0.4191.62
  - **AccessKit / Adapters**: `accesskit` 0.25.0, `accesskit_winit` 0.34.0, `winit` 0.30.13, `wry` 0.53.5
  - **Screen Readers**: Narrator (Windows 11 25H2 built-in), NVDA 2024.4.1
- **Tier 2 Status**:
  - **State**: Unmeasured.
  - **Reason**: No physical interim Mac at or near the macOS 13 floor was available during this run. Tier 2 N6 measurement remains owed on the Tier 2 pinned runner or interim Mac when procured.

---

## Claim Scope

Under `decisions/0004`:
1. **Software Determination**: The cross-tree composition and labelling results measured here are determined by software architecture (UI Automation hierarchy, WebView2 native UIA tree publishing, AccessKit 0.25.0 tree scoping), not by underlying CPU or GPU hardware performance.
2. **Tier 1 Candidate Elimination**: The negative result regarding cross-tree `labelled-by` references stands for Tier 1 under AccessKit 0.25.0 and WebView2. An accessibility property structurally inexpressible in AccessKit 0.25.0 will not become expressible on Tier 1 reference hardware running the same runtime.
3. **Tier 2 Open Condition**: This measurement makes no claims regarding Tier 2 (WKWebView / NSAccessibility). Tier 2 remains unmeasured and owed.

---

## Spike Architecture & Test Setup

The measurement was conducted using the spike host at `tests/spikes/n6-chrome-accessibility/`:
- **Scaffold** (`src/host.rs`): A `winit` 0.30 window embedding a `wry` 0.53 child webview rendering an inline HTML document with a heading (`#content-heading`), a text input (`#content-input`), and a button (`#content-button`).
- **Chrome Front** (`src/fronts/accesskit_min.rs`): An `accesskit_winit` 0.34 front publishing a 3-node AccessKit tree (`TreeId::ROOT`): a window root node (`NodeId(0)`), an address text input (`NodeId(1)`), and a "Go" button (`NodeId(2)`).

---

## Measurement Results

### 1. Can a chrome node be labelled by a content node across the host and web-view trees?

* **Result**: **NO** (Structurally Inexpressible).
* **Structural Rationale**:
  - `accesskit::Node::set_labelled_by` takes `accesskit::NodeId` parameters.
  - An `accesskit::NodeId` is a 64-bit integer whose resolution is strictly scoped to the local AccessKit tree instance (`TreeId::ROOT`) managed by `accesskit_winit::Adapter`.
  - The embedded WebView2 content view publishes its own platform native UI Automation (UIA) tree directly to the operating system. It is not an AccessKit tree and is not registered with the AccessKit adapter.
  - AccessKit 0.25.0 has no API or node property that can reference, embed, or cross-link a node from an external native platform tree (like WebView2's HWND/UIA subtree).
  - Consequently, referencing `#content-heading` inside the webview as the `labelled-by` node for the chrome address input cannot be expressed in AccessKit's schema. The host falls back to providing a local text label (`"Address"`).

### 2. Does focus cross the boundary in reading order?

* **Result**: **YES** (Handled via Window Keyboard Traversal).
* **Details**:
  - Tabbing through the chrome nodes moves AccessKit focus from `ADDRESS_ID` (`NodeId(1)`) to `GO_ID` (`NodeId(2)`).
  - Tabbing past the last chrome control (`GO_ID`) triggers host-side window event processing (`WindowEvent::KeyboardInput` for Tab), which explicitly calls `webview.focus()`.
  - Focus is successfully transferred to the WebView2 child HWND. Within the webview, focus moves sequentially through content controls (`#content-input` -> `#content-button`) governed by the browser engine's internal focus loop.
  - Reverse navigation (Shift+Tab) from the webview back into the host chrome requires host-level keyboard accelerators / focus-movement hooks (such as `MoveFocusRequested`).

### 3. Screen Reader Announcements (Narrator & NVDA)

#### Narrator (Windows 11 built-in UIA client):
- **Chrome Navigation**: When focused on the address input, Narrator announces: *"Address, edit, about:spike"*. When focused on the Go button, Narrator announces: *"Go, button"*.
- **Boundary Crossing**: Pressing Tab on the Go button moves system focus to the webview. Narrator announces the transition into the webview content document: *"N6 content page, web document"*, followed by the focused web control *"Content input, edit"*.
- **Cross-Tree Labelling**: Narrator reads local AccessKit labels (`"Address"`). It cannot read or associate the in-page HTML heading as a label for the chrome address field across the tree boundary.

#### NVDA (2024.4.1):
- **Chrome Navigation**: NVDA announces *"Address edit about:spike"* and *"Go button"*.
- **Boundary Crossing**: Pressing Tab moves NVDA into browse/focus mode inside the WebView2 document, announcing *"Content input edit"*.
- **Cross-Tree Labelling**: NVDA reads the local node label `"Address"`. No cross-tree label relationship is established or reported.

---

## Architectural Consequences & Reopen Conditions

1. **Principle X / Drawn Chrome Admissibility**:
   - Drawn-chrome candidates (such as a custom Rust GPU UI with AccessKit) cannot establish cross-tree semantic associations (like `labelled-by`) with web content nodes.
   - Drawn chrome depends on explicit host-level keyboard focus trapping and focus handoff hooks (`webview.focus()`, `MoveFocusRequested`) to maintain reading-order focus traversal across the chrome/content boundary.
   - Re-verification of the shipped chrome focus boundary is required at T162 / T163 (`docs/measurements/n6-shipped-chrome-verification.md`).

2. **Reopen Conditions (from `decisions/0004`)**:
   - A Tier 1 pinned-runner re-run contradicting this interim result reopens `decisions/0004` and `ADR-0002`.
   - Tier 2 N6 measurement on macOS 13 floor (WKWebView / NSAccessibility) remains owed when Tier 2 reference hardware is procured.
