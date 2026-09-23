# ADR-0002: Windowing crate and browser chrome rendering architecture

- **Status**: Accepted
- **Date**: 2026-09-23
- **Deciders**: Founder

## Context

ADR-0001 selected `wry` for hosting web content in operating-system webviews (WebView2 on Windows, WKWebView on macOS) but left the windowing crate and chrome rendering mechanism as a free variable to be determined by spike S4 (task T016).

The browser's own chrome — omnibox address bar, tab strip, window controls, navigation buttons, and money surface triggers — requires a windowing API and a rendering approach that satisfy strict architectural constraints:
1. **SC-006 Latency Budget**: Address-field keystroke and tab-switch input-to-repaint latency must not exceed 16 ms at the 99th percentile (p99).
2. **SC-004 Memory Budget**: Shell memory overhead must fit within the 150 MB baseline budget at ten open tabs on Tier 1.
3. **Accessibility (SC-008, Principle X)**: Chrome controls must be fully accessible to platform screen readers (Narrator/NVDA on Windows, VoiceOver on macOS).
4. **International Text & IME (FR-003, T164)**: Must support composed international text input, German dead keys, and Greek text entry without losing composition state.
5. **Maintainability**: Manageable maintenance footprint for a solo founder across Tier 1 (Windows) and Tier 2 (macOS).

Research (`specs/001-evreos-v1/research.md` §5.2) identified three primary candidates:
- **Candidate 1: Platform-native widgets per tier** (Win32/UIA on Windows, Cocoa/AppKit on macOS).
- **Candidate 2: A second web view rendering chrome as HTML** (using `wry`).
- **Candidate 3: A Rust GPU toolkit with AccessKit** (Drawn chrome in Rust using `winit`).

## Decision

**Select `winit` 0.30.13 (with `accesskit_winit` 0.34.0 / `accesskit` 0.25.0) as the windowing and drawn-chrome renderer crate.**

The browser's own chrome surfaces will be rendered in a single Rust host process drawn on GPU surfaces created via `winit` windows, publishing accessibility subtrees through `accesskit_winit`.

All downstream tasks (such as application shell creation in T038 and text entry in T164) read their windowing API and IME composition paths strictly from this ADR.

---

## Rationale & Candidate Evaluation

The candidates were evaluated on the interim Tier 1 instrument under `decisions/0004` and recorded at `docs/measurements/s4-chrome-candidates.md`:

### 1. Second Webview HTML Chrome (Eliminated)
- **Memory Overhead**: Spawns a second evergreen WebView2 renderer process tree for the chrome, adding ~147 MB of Resident Set Size (RSS) overhead. Total memory at 10 open views reached 285 MB, violating SC-004.
- **Latency**: Inter-process communication (IPC) between the main shell host process and the chrome webview, plus webview layout and composition cycles, resulted in coarse p99 input-to-repaint latency of 18.4 ms (exceeding the 16 ms cap).
- **Engine Coupling**: Couples the chrome's availability and stability directly to the web engine runtime.
- **Result**: Grossly disqualified and eliminated.

### 2. Platform-Native Widgets (Eliminated)
- **Maintainability Burden**: Requires writing and maintaining two distinct UI codebases (Win32 on Windows, Cocoa/AppKit on macOS).
- **Seam Friction**: Complicates brand configuration (FR-035) and theme customization (FR-042) across two native widget toolkits.
- **Result**: Eliminated due to unacceptable maintainability overhead for a solo founder.

### 3. Drawn Chrome in Rust via `winit` + `AccessKit` (Selected)
- **Latency Performance**: Direct event handling in `winit` yields p99 input-to-repaint latency of ~2.1 ms, comfortably inside SC-006's 16 ms cap.
- **Memory Efficiency**: Runs inside the main shell process. Total working set memory across 10 open views sits at ~138 MB, satisfying SC-004.
- **Cross-Platform Parity**: A single Rust-native implementation serves all tiers cleanly.
- **Accessibility Traversal (Measurement N6)**: Evaluated in `docs/measurements/n6-chrome-accessibility.md`. While cross-tree `labelled-by` references between AccessKit and WebView2 trees are structurally inexpressible, reading order and focus traversal function correctly across the chrome/content boundary via host window keyboard trapping.
- **Result**: Provisionally admitted as the winning candidate.

---

## Windowing & IME API Specification for Downstream Tasks

This section defines the canonical API contract that downstream tasks (including T038 `crates/evreos-chrome` / `crates/evreos-shell` and T164 text entry) MUST consume:

### 1. Crate Versions
- `winit` = `=0.30.13`
- `accesskit_winit` = `=0.34.0`
- `accesskit` = `=0.25.0`
- `wry` = `=0.53.5`

### 2. Windowing & UI Event Loop Contract
- Application windows are created using `winit::window::WindowBuilder` / `ActiveEventLoop::create_window`.
- The main event loop is managed by implementing `winit::application::ApplicationHandler`.
- All window event handling and webview delegate callbacks MUST execute on the platform's main UI thread.

### 3. IME Composed Text Entry Path (FR-003, T164)
- Text inputs in chrome controls (omnibox, find-in-page) MUST consume composed text events via `winit::event::WindowEvent::Ime`.
- Chrome text input fields MUST enable IME on focus by calling `window.set_ime_allowed(true)` and disable it on blur (`set_ime_allowed(false)`).
- Composed characters (such as German dead-key combinations like `´` + `a` -> `á`, and Greek accented characters) MUST be read from `winit::event::Ime::Commit(text)` or `winit::event::KeyEvent::text`, and NEVER from raw virtual key codes (`PhysicalKey` / `VirtualKeyCode`).

---

## Consequences & Reopen Conditions

This decision is **provisional** under `decisions/0004`. As required by `decisions/0004`, this record carries the following verbatim reopen conditions:

1. **Pinned-Runner Re-run**: A pinned-runner re-run contradicting an interim result recorded under `decisions/0004` reopens this decision.
2. **Tier-2 macOS N6 Traversal**: A Tier 2 N6 result at the macOS floor failing the join for the selected candidate reopens this decision.
3. **Full Harness Measurement**: Failure of the built chrome to satisfy SC-006 (16 ms p99 latency) or SC-004 (150 MB memory floor) when measured by the ratified Phase 4 benchmark harnesses reopens this decision.
