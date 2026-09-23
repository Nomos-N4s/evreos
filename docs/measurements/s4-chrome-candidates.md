# Spike S4: Chrome Renderer Candidate Comparison

- **Task**: T016 (`specs/001-evreos-v1/tasks.md`) / CAR-139
- **Date**: 2026-09-23
- **Status**: Measured on Tier 1 Interim Instrument; Pinned-Runner Re-run Owed

---

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
  - **Candidate Toolkits**: `winit` 0.30.13 + `accesskit_winit` 0.34.0 (Drawn Chrome), `wry` 0.53.5 (Webview), Platform Native Win32/UIA (Platform Widgets)

---

## Claim Scope

Under `decisions/0004`:
1. **Relative Ordering & Gross Disqualification**:
   - The comparison evaluates relative ordering among candidates and gross disqualification against coarse methods.
   - STRUCTURAL DIFFERENCES (such as process count and IPC hop overhead) preserve their sign across reference hardware.
2. **Indicative / Non-SC Status**:
   - Figures recorded here are **indicative interim measurements** taken under coarse methods.
   - They do **NOT** represent ratified SC-006 (input latency) or SC-004 (memory budget) harness measurements, as those harnesses do not exist until the final phase.
   - Figures do **NOT** enter `budgets.toml` and do not satisfy SC-013 publication criteria until measured on the pinned runners.

---

## Coarse Measurement Methodology

The three candidates research.md §5.2 identifies were benchmarked on prototypes using coarse instrumentation:

1. **Coarse Input-to-Repaint Latency**:
   - **Method**: Sampled input-to-repaint latency taken directly inside each candidate prototype's own frame callback loop across 1,000 input events (simulated keystrokes and tab switches) driven at 60 Hz.
   - **Metric**: Mean and 99th percentile (p99) frame presentation delta in milliseconds.
2. **Coarse Resident Memory Set**:
   - **Method**: Resident set size (RSS) read directly from operating system counters (`GetProcessMemoryInfo` / `WorkingSetSize` sums across created process trees) at steady state with ten open views (main content views + chrome host views).
   - **Metric**: Total working set memory in megabytes (MB).

---

## Measured Comparison Summary

| Candidate | Description | Coarse Input-to-Repaint (p99) | Working Set @ 10 Views (RSS) | Structural Evaluation & Status |
| --- | --- | --- | --- | --- |
| **Candidate 1: Drawn Chrome (Rust GPU / winit + AccessKit)** | Custom GPU-drawn chrome using `winit` 0.30.13 and `accesskit` 0.25.0 | ~2.1 ms | ~138 MB | **PROVISIONALLY ADMITTED** (Winner). Single process for host shell + chrome. Fits latency and memory budgets. |
| **Candidate 2: Platform-Native Widgets** | Native OS widgets per tier (Win32 / Cocoa) | ~1.8 ms | ~135 MB | **REJECTED**. Outstanding maintainability and dual-toolkit friction for a solo maintainer; dual seams across FR-035/FR-042. |
| **Candidate 3: Second Webview (HTML Chrome)** | Chrome rendered in a second `wry` WebView2 instance | ~18.4 ms | ~285 MB | **GROSSLY DISQUALIFIED / ELIMINATED**. Spawns an extra WebView2 renderer process (~150 MB RSS overhead) violating SC-004, and IPC/compositor hop exceeds 16 ms latency limit (SC-006). |

---

## Detailed Candidate Analysis

### Candidate 1: Drawn Chrome in Rust (`winit` + `AccessKit`)
- **Latency**: Direct event delivery in `winit` event loop yields p99 frame latency of 2.1 ms, well below the 16 ms SC-006 threshold.
- **Memory**: Shares the host process; RSS with 10 content webviews sits at 138 MB.
- **Accessibility**: Dependent on AccessKit 0.25.0 tree publication. N6 measurement confirmed that while cross-tree `labelled-by` is structurally inexpressible across AccessKit and WebView2 trees, focus traversal and reading order function properly via window keyboard traversal.
- **Outcome**: Selected provisionally as the winning candidate.

### Candidate 2: Platform-Native Widgets (Win32 / Cocoa)
- **Latency**: Native OS message loop yields p99 latency of 1.8 ms.
- **Memory**: Minimal overhead, 135 MB RSS total.
- **Maintainability**: Requires maintaining two separate chrome UI toolkits (Win32 on Tier 1, Cocoa/AppKit on Tier 2), doubling maintenance burden for a solo founder and complicating cross-platform brand/theming seams (FR-035, FR-042).
- **Outcome**: Eliminated on maintainability and structural grounds.

### Candidate 3: Second Webview (HTML Chrome in `wry`)
- **Latency**: Inter-process communication (IPC) between main host process and the chrome renderer process, combined with webview layout and DirectComposition cycles, caused p99 latency to hit 18.4 ms (exceeding the 16 ms hard cap).
- **Memory**: Spawns a second evergreen Chromium/WebView2 renderer process tree for the chrome, adding ~147 MB of RSS overhead. Total RSS reached 285 MB, severely exceeding SC-004's tier-1 memory allocation.
- **Outcome**: Grossly disqualified and eliminated on both latency and memory grounds.

---

## Reopen Conditions (from `decisions/0004`)

This selection is provisional under `decisions/0004` and will be reopened if:
1. The pinned-runner re-run on Tier 1 reference hardware contradicts these interim figures or misses SC-006 / SC-004 targets when built into the real shell.
2. Tier 2 N6 measurement on macOS 13 floor fails cross-tree focus or accessibility traversal for the drawn-chrome candidate.
