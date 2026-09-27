# Default-browser registration: byte cost of the tier-1 bindings

- **Task**: T059 (`specs/001-evreos-v1/tasks.md`) / CAR-116
- **Date**: 2026-09-27
- **Status**: measured on a cross-compiled tier-1 build; the tier-1 figure on the
  pinned runner is owed

## The question

T059 reaches every platform service through a safe binding whose byte cost is
stated against `budgets.toml`. FR-013 on tier 1 needs two: the registry, to
register the browser, and a launcher, to open the system's default-apps page.
This records what each adds to a release build.

## The bindings

- **`windows-registry` 0.6.1**, Microsoft's safe binding to the registry, for
  `WindowsRegistry` in `crates/evreos-platform/src/default_browser/windows.rs`.
  It depends on `windows-link` 0.2.1, `windows-result` 0.4.1 and
  `windows-strings` 0.5.1, all already in the workspace's graph at those
  versions. It is the one crate the lockfile gains.
- **`windows` 0.62.2**, with its `System` and `Foundation` features, for
  `Windows.System.Launcher`, which `open_settings` hands
  `ms-settings:defaultapps` to. The shell already links this crate on Windows,
  through `evreos-chrome`'s `accesskit_windows`, so the lockfile gains no
  crate for it.

Both are Windows-only dependencies of `evreos-platform`. Nothing is added to a
build for any other target.

## Method

The workspace's release profile (`opt-level = "z"`, fat LTO, one codegen unit,
`panic = "abort"`, symbols stripped), on `x86_64-pc-windows-gnu`, cross-built on
Linux with rustc 1.94.1 and MinGW-w64 GCC 13. The tier-1 target is
`x86_64-pc-windows-msvc`, which cannot be linked on this host, so these figures
are indicative of it and not a substitute for it.

Five small executables in a scratch crate outside the workspace, each reading
its arguments so that nothing is folded away, depend on `evreos-platform` by
path and use the workspace's `Cargo.lock`:

| Executable | What it reaches |
| --- | --- |
| `baseline` | its arguments and one line of output |
| `io_error` | the baseline, plus an unwrapped `io::Result` from `std::fs` |
| `registry_only` | the baseline, plus `register` and `unregister` through `WindowsRegistry::current_user` |
| `settings_only` | the baseline, plus `open_settings` |
| `reached` | the baseline, plus registration, its removal and `open_settings` |

Sizes are `st_size` of each `.exe`. Two builds from clean sources gave the same
bytes.

## Results

| Executable | Bytes | Over `baseline` |
| --- | ---: | ---: |
| `baseline` | 251,904 | — |
| `io_error` | 262,144 | 10,240 |
| `registry_only` | 267,776 | 15,872 |
| `settings_only` | 263,168 | 11,264 |
| `reached` | 271,360 | 19,456 |

`io_error` shows how much of each figure is only the standard library's error
formatting, which the shell already carries: 10,240 bytes. Against it,
registration adds 5,632 bytes, the settings page 1,024, and the two together
9,216.

**The option not adopted.** `open_settings` first handed the page to
`explorer.exe` through `std::process::Command`. Measured the same way,
`settings_only` was then 320,512 bytes, 68,608 over `baseline`, against 11,264
through the launcher. Spawning a process on Windows brings in the standard
library's command-line quoting and environment handling, which nothing else in
the build uses.

## Against `budgets.toml`

SC-001's download-size and installed-footprint entries for Windows are
unmeasured: no installer exists yet. Nothing in the shipped binary reaches
`evreos-platform` at this change; the shell does not depend on it until a
surface offers FR-013. So the cost as shipped is 0 bytes, and once a surface
reaches registration and the settings page it is at most 19,456 bytes on this
build, 0.019 MB. That is an upper bound: the shell already links `windows`
0.62 and the standard library's error formatting, which the probes count.

No other entry moves. Registration writes 19 string values once, when the
member asks, and the launch returns without waiting, so there is no idle wake,
no resident memory beyond the call and nothing on the chrome's input path.

## Owed

- The same five executables on the tier-1 pinned runner, for
  `x86_64-pc-windows-msvc`.
- The marginal cost inside the shell itself, measured when a surface first
  reaches `evreos-platform`.

## Reproducing

Make a crate outside the workspace with the release profile above, a
`[workspace]` table of its own, `evreos-platform` as a path dependency and a
copy of the workspace's `Cargo.lock`. Put each executable under `src/bin/`:

```rust
// baseline.rs
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let name = args.get(1).map_or("", String::as_str);
    println!("{}", name.len());
}
```

`io_error` adds `if args.len() > 2 { std::fs::metadata(name).unwrap(); }`.
`settings_only` adds `if args.len() > 2 { open_settings().unwrap(); }`.
`registry_only` adds:

```rust
let app = Application { name, description: name, executable: name };
let mut registry = WindowsRegistry::current_user().unwrap();
if args.len() > 2 {
    register(&mut registry, &app).unwrap();
} else {
    unregister(&mut registry, &app).unwrap();
}
```

and `reached` does the same, calling `open_settings().unwrap()` after
`register`. Then run
`cargo build --release --target x86_64-pc-windows-gnu` and compare the
executables' sizes.
