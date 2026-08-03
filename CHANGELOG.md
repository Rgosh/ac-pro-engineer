# Changelog - RaceEngineer (AC Pro Engineer)

All notable changes to this project will be documented in this file.

## [Unreleased]

## [v0.3.1] - 2026-08-03

A bug-fix release. The launcher's version carousel, the Settings tab and the
Setup Cloud browser now do what they have always claimed to; several crashes
are gone; and Assetto Corsa is finally found on Linux.

### ⚠️ Read This First

- **Cold tyre pressure targets will shift.** The calculator scales by
  `surface_grip`, which used to read a constant `0.0` and clamp to a floor of
  `0.80`, so every recommendation carried the same fixed compensation. With
  real grip, a well-rubbered track (≈0.94) gives roughly a third of the old
  adjustment. Your numbers will differ from v0.3.0 on the same car and track —
  that is the fix working.
- **Settings you saved before this release were never written to disk.** The
  Settings tab did not persist anything, so it will come up with defaults one
  last time.

### 🚀 New

- **Assetto Corsa is found on Linux.** The install root was probed as four
  hardcoded Windows drive letters, and setups were looked for in
  `~/Documents` — but under Proton the game writes inside its own prefix.
  Steam's `libraryfolders.vdf` is read too, so a library on any drive works.
  `ac_install_path` and `ac_documents_path` in the config override both.
- **The Setup Cloud browser works.** Arrows navigate, `D` installs,
  PgUp/PgDn scroll. Previously the tab handled only Up/Down/B, so the browser
  opened onto a permanently empty list — while its own hint line, the help
  overlay and the README all documented `D`.
- **Fuel strategy no longer waits on AC.** Consumption measured across
  completed laps fills in when `fuel_x_lap` reads zero, which it does for the
  whole of lap one.
- **Honest connection status.** The footer distinguishes `LIVE`,
  `AC RUNNING - NO DATA` and `AC NOT RUNNING` instead of ONLINE/OFFLINE.
- **CSV export carries RPM, both G axes and slip**, and names files after the
  car, track and lap instead of colliding on `lap_3_export.csv`.
- **Terminal-too-small screen** instead of drawing into an area that cannot
  hold the layout.

### 🛡️ Fixed

**Crashes**
- Narrow terminals: four `Rect` fields in the Setup tab subtracted constants
  from a `u16` width, wrapping to ~65530 below 20 columns.
- Updater: `"░".repeat(20 - filled)` panicked mid-download on any percentage
  over 100.
- Stale shared memory: `Gauge::ratio` asserts on its input and `clamp` passes
  NaN through, so one garbage float from a zeroed `/dev/shm` page took the app
  down.
- A config with `update_rate: 0` spun two cores at 100% — `validate()` had no
  caller outside its own unit test.

**Things that silently did nothing**
- Version carousel arrows: releases older than the running one were filtered
  out, leaving a one-entry list with nowhere to move.
- Settings were never saved, and never re-applied without a restart. The
  `auto_save` and `show_ghost_delta` toggles were read by nothing.
- Personal bests were compared against the *world record*, so `records.json`
  only ever gained an entry from someone who had beaten it.
- Setup auto-detection required a perfect three-way match, so one lap of burnt
  fuel blanked the "(NOW: x%)" hints.
- The suspension roll-asymmetry warning compared a value against itself.
- On Linux the launcher waited forever for `simulator.exe`; the Linux build is
  called `simulator`.

**Wrong numbers**
- Driving-style aggression combined the lateral and *vertical* G axes, so a
  stationary car scored 40% and braking was invisible.
- Out-laps scored perfect tyre management: with no sample above the speed gate
  the deviation computed to 0.0 and the score to 100.
- Mistake counts scaled with Update Rate, making laps recorded at different
  rates incomparable.
- The final sector split raced the lap counter and could land in the next lap.
  `AcStatic::sector_count` is honoured now, so 2- and 4-sector mod tracks work.
- Fuel targets under-fuelled: a timed race ends when the leader *completes* the
  lap the clock ran out on, and the lap in progress still has to be finished.
- `fuel_laps_remaining` was never cleared, so BOX BOX BOX could fire after a
  refuel on a value measured before the stop.
- Physics and graphics pages are re-read when AC's `packet_id` moves mid-copy,
  so a frame spliced from two game ticks no longer reaches the analyzer.
- Target pressures printed a hardcoded "PSI" and temperatures a hardcoded "C";
  tyre temperature *spreads* were converted as absolute temperatures, adding a
  32°F offset that does not belong to a difference.

**Keys and text**
- The first-run prompt could not be exited with Ctrl+C, q or Esc.
- F1 did not close the help modal that says "PRESS ESC, ?, Q, OR F1 TO CLOSE".
- Esc in the analysis load menu quit the whole session.
- Held keys were dropped on Windows.
- `S` saved the fastest lap rather than the selected one.
- Tabs were documented as F1–F9 throughout; they are 1–9.
- Status messages never cleared, so a stale one looked fresh.
- Twelve locale keys existed only in Russian; a test now enforces parity.

**Data and shutdown**
- Records, config and CSV export renamed a temp file into place without
  flushing first, so a power loss could publish a correctly-named empty file.
  Two instances saving at once also shared a temp path.
- Crash reports and logs were written relative to the working directory, which
  is unwritable from a shortcut or under Program Files — the crash report was
  then dropped in silence. A logging failure also aborted startup.
- shm-bridge's cleanup returned on the first failure, leaving zero-filled pages
  the app maps without complaint, reporting a healthy connection to a dead feed.
- Quitting could hang forever waiting on a bridge that never acknowledged the
  exit request.
- A missing `protontricks-launch` was fatal, so anyone running AC natively —
  or reviewing saved laps offline — could not start the app.
- A newline in a downloaded setup's notes could inject an INI section into a
  file AC parses as a car setup.

### ⚡ Performance

- `is_process_running` reads every process on the system and was called twice
  per frame from the launcher — roughly 124 full scans a second while sitting
  in a menu. Cached for one second.
- Loading a car's cloud setups no longer blocks the render thread on a
  five-second HTTP call.

### 🧹 Internal

- Shared-memory layout tests parse graphics, physics and static pages captured
  verbatim from a live AC 1.16.4 session. Previously every test built an `Ac*`
  value in Rust and read it back, so none could detect a mismatch with the game.
- The test suite now compiles under the workspace edition and lints;
  `unwrap_used` and `panic` were silently unenforced across it. Removed two
  modules that asserted nothing about this project.
- CI builds with `--locked`. Release scripts and generated screenshots read the
  version from the workspace manifest instead of a hardcoded string two
  releases old.
- 171 tests, up from 130.

## [v0.3.0] - 2026-08-02

### 🚀 New Features & Enhancements
- **Automated Release Pipeline**: Added `cargo-dist` configuration and a GitHub Actions workflow that builds and publishes Linux and Windows binaries, with shell and PowerShell installers.
- **Continuous Integration**: Added a CI workflow running `cargo fmt --check`, `cargo clippy --workspace --all-targets` and `cargo test --workspace` on Linux and Windows.

### 🛡️ Bug Fixes & Stability
- **In-App Updater Platform Selection**: The updater looked for a `-linux` asset suffix that no release has ever published, so on Linux no update was ever offered. Asset selection is now based on the running OS, rejects artifacts that are not the application (`shm-bridge`, installers, checksums), and refuses to install a build for a foreign platform.
- **In-App Updater Archive Support**: The updater now unpacks the application binary out of release archives (`.tar.gz` on Linux, `.zip` on Windows) instead of only handling bare binaries.

---

## [v0.2.3] - 2026-07-30

### 🚀 New Features & Enhancements
- **Live Micro-Sector & Predictive Lap Time Engine**: Real-time sector split analytics (S1, S2, S3) and predictive delta estimation built into `ac_core::analyzer` and UI tabs.
- **Crash Diagnostic Logging & Panic Hook**: Added custom `std::panic::set_hook` diagnostic logger that captures unhandled exceptions and exports detailed crash trace dumps (`crash_report_<timestamp>.log`) to the logs directory.
- **External JSON Localization System**: Moved all UI translations to external `data/locales/en.json` and `data/locales/ru.json` files with embedded compile-time fallbacks.
- **Linux Bash Build Script (`build_release.sh`)**: Added executable Linux release packaging script for native Linux TUI binaries and Wine/Proton `shm-bridge.exe`.
- **Pixel-Perfect PNG Text Glyph Renderer**: Enhanced `tui_tester` tool with bitmap text glyph rendering for readable English PNG screenshots.

### 🛡️ Bug Fixes & Stability
- **Safe Lock Protection**: Converted `SafeLock` mutex primitives to handle poisoned locks without crashing or panicking.
- **Cross-Platform Compatibility**: Gated Win32 file mapping APIs cleanly under Linux target stubs so `cargo check --workspace` passes without errors on all platforms.
- **Clippy Cleanliness**: Resolved all Clippy warnings and enforced strict workspace linting rules (`unwrap_used = "deny"`, `panic = "deny"`).

---

## [v0.2.2] - 2026-07-30

### 🌟 Features
- Added ratatui TUI dashboard, telemetry analyzer, setup manager, and overlay manager.
- Added cross-platform shared memory reader for Assetto Corsa.
