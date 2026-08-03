# Changelog - RaceEngineer (AC Pro Engineer)

All notable changes to this project will be documented in this file.

## [Unreleased]

## [v0.3.1] - 2026-08-03

### 🚀 New Features & Enhancements
- **Linux & Proton Path Discovery**: Assetto Corsa is now found on Linux. The install root was probed as four hardcoded Windows drive letters, so `content/cars` was never located and every car-spec lookup returned nothing; setups were looked for in `~/Documents`, while under Proton the game writes to `Documents` inside its own prefix. Steam's `libraryfolders.vdf` is read as well, so a library on any drive is found rather than guessed at. `ac_install_path` and `ac_documents_path` in the config override both.
- **Setup Cloud Browser Is Reachable**: The Setup tab handled only Up, Down and B, so the browser opened onto a permanently empty setup list with no way to install anything — while its own hint line, the help overlay and the README all documented `D` to download. Arrows navigate, Enter reloads, `D` installs, PgUp/PgDn scroll. `load_browser_car`, `download_setup`, `get_browser_selected_setup` and `scroll_details` previously had no callers anywhere in the workspace.
- **Measured Fuel Consumption**: Fuel estimates no longer depend solely on AC's `fuel_x_lap`, which reads zero for the whole of lap one and sits in the part of the graphics page not yet confirmed against a live capture. Consumption measured across completed laps fills in, so the strategy tab works from lap two regardless.
- **Terminal Size Guard**: The app shows its current and required size instead of drawing into an area too small for its layout.
- **Real Connection Status**: The footer distinguishes `LIVE`, `AC RUNNING - NO DATA` and `AC NOT RUNNING`, rather than collapsing all three into ONLINE/OFFLINE. Panels with no telemetry say so instead of rendering nothing.
- **Torn-Read Detection**: Physics and graphics pages are re-read when AC's `packet_id` changes mid-copy, so a frame spliced from two different game ticks no longer reaches the analyzer.

### 🛡️ Bug Fixes & Stability
- **Shared-Memory Graphics Layout**: `AcGraphics` was using Assetto Corsa Competizione's `SPageFileGraphic` layout, which carries `activeCars`, `carCoordinates[60][3]`, `carID[60]`, `playerCarID` and `penalty` — 964 bytes that plain AC never writes. Every field from `car_coordinates` onward was therefore read from the wrong offset, past the end of the 360-byte page AC actually publishes. Track position was plotted from the car's altitude, and `surface_grip`, `fuel_x_lap`, `wind_speed`, `tc`, `abs`, `engine_map`, `flag` and the driver-stint timers all read a constant zero. Reported in [#2](https://github.com/Rgosh/ac-pro-engineer/issues/2).
- **Version Carousel Arrows Did Nothing**: `check_for_updates` dropped every release older than the running one, so on the newest build the list held a single entry and Left/Right had nothing to move between — while the launcher rendered a "you won't be able to switch back" warning for legacy versions that could never appear. Reported by users unable to roll back.
- **Settings Were Never Saved**: `handle_input` mutated the config and nothing wrote it back, so every unit, alert threshold, target pressure and update rate was discarded on exit — on a tab whose own item 3 reads "Automatically save settings on exit". `apply_config` had no callers either, so changes did not take effect until a restart. The `auto_save` and `show_ghost_delta` toggles now do something.
- **Crashes on a Narrow Terminal**: Four `Rect` fields in the Setup tab subtracted constants from `area.width`/`area.height` as u16, wrapping to ~65530 below 20 columns and indexing out of the render buffer. The updater's download bar did the same with `"░".repeat(20 - filled)` on an unclamped percentage — a panic mid-update.
- **NaN Crashes from Stale Shared Memory**: `Gauge::ratio` asserts its input is in 0.0..=1.0, and `clamp` returns NaN unchanged, so one garbage float from a zeroed `/dev/shm` page took the app down. All nine gauge call sites now reject non-finite input first.
- **100% CPU From a Bad Config**: `AppConfig::validate` had no caller outside its own unit test, so `update_rate: 0` in the config file reached `event::poll` and `thread::sleep` and spun two cores. Validation now runs on load and covers the pressure targets, alert bands, temperature limits and shift point that had no bounds at all.
- **Aggression Measured the Wrong Axis**: `acc_g` is `[lateral, vertical, longitudinal]` and the driving-style metric combined indices 0 and 1 — so it included the ~1 g the car carries standing still (a stationary car scored 40% aggression) and ignored braking and acceleration entirely.
- **Personal Bests Were Never Saved**: Laps were compared against the world record rather than the driver's own history, so `records.json` only ever gained an entry from someone who had beaten it. The whole block was also nested inside a car-specs lookup that always failed on Linux, so no record was created, compared or saved there at all.
- **Final Sector Split Raced the Lap Counter**: The last sector was captured on the transition that coincides with the lap-count increment, so depending on which AC published first it could land in the following lap. It is derived from the lap time now. `AcStatic::sector_count` is also honoured, so two- and four-sector mod tracks produce a theoretical best.
- **Out-Laps Scored Perfect Tyre Management**: With no sample above the speed gate, pressure deviation computed to 0.0 and the tyre score to a perfect 100 — an out-lap rated better than a hot lap, and the advice recommended inflating by 27.5 psi against a 0.0 psi reading.
- **Mistake Counts Scaled With Update Rate**: Oversteer, understeer, lockup and scrubbing counts were divided by a fixed sample count, so changing Update Rate in Settings halved every score and made laps recorded at different rates incomparable.
- **Fuel Targets Under-Fuelled**: A timed race ends when the leader *completes* the lap the clock ran out on, and the lap in progress still has to be finished; the fuel target used a fraction that accounted for neither.
- **Stale `/dev/shm` Mappings**: shm-bridge's cleanup loop returned on the first failure, leaving the remaining mappings behind as zero-filled pages that the app maps without complaint — reporting a healthy connection to a dead feed. It is best-effort now and reports what it could not remove.
- **Quitting Could Hang Forever**: The Linux bridge shutdown blocked on its join handle with no timeout, so a bridge that never acknowledged the exit request left the app unable to finish quitting. Errors inside that task were also discarded entirely.
- **App Refused to Start Without Protontricks**: A missing `protontricks-launch` was propagated as a fatal error before the TUI was drawn, so anyone running AC natively — or just wanting to review saved laps offline — could not start the app.
- **Crash Reports and Logs Went Nowhere**: Both were written relative to the working directory, which is unwritable when the app is launched from a shortcut or installed under Program Files. The crash report was then dropped in silence. A logging failure also aborted startup.
- **Durability of Saved Data**: The records file, config and CSV export renamed a temp file into place without flushing it first, so a power loss could publish a correctly-named empty file. Two instances saving at once also shared a temp path.
- **Setup Auto-Detection Was Unreachable**: `match_score` can only produce 0/20/25/30/45/50/55/75 and the threshold was `> 60`, so only a perfect three-way match qualified — one lap of burnt fuel silently blanked the "(NOW: x%)" hints.
- **INI Injection From Remote Setups**: A newline in a downloaded setup's notes field opened a new line in the file AC parses as a car setup, letting a `[SECTION]` be smuggled in past everything the downloader validates.
- **Keys That Did Nothing**: The first-run prompt could not be exited with Ctrl+C, q or Esc. F1 did not close the help modal that says "PRESS ESC, ?, Q, OR F1 TO CLOSE". Esc in the analysis load menu quit the whole session instead of closing the menu. Held keys were dropped on Windows. `S` saved the fastest lap rather than the selected one.
- **Wrong Units Displayed**: Target pressures printed a hardcoded "PSI" and ambient temperatures a hardcoded "C" regardless of the configured unit; alert thresholds printed no unit at all. Tyre temperature *spreads* were converted as if they were absolute temperatures, which adds 32 °F that does not belong to a difference.
- **Status Messages Never Cleared**: `status_timer` was set and never decremented, so "Exported CSV: ..." stayed pinned to the footer for the rest of the session and a stale message was indistinguishable from a fresh one.
- **Unreachable Roll-Asymmetry Warning**: The suspension check compared `avg_ride_height[0]` against itself, so the difference was always exactly zero. AC publishes ride height per axle, not per corner, so the check cannot be written against this data and has been removed.
- **Locale Files Out of Sync**: Twelve keys existed only in Russian; a test now enforces parity. A malformed locale override also produced an empty dictionary in silence, degrading the whole UI to raw key names.
- **Linux Simulator Undetectable**: `is_process_running` matched only `simulator.exe`, but the Linux build produces `simulator`, so the launcher waited forever on the platform the bridge exists for.

### ⚡ Performance
- **Process Scanning**: `is_process_running` reads every process on the system and was called twice per frame from the launcher — roughly 124 full scans per second while sitting in a menu. Cached for one second.
- **Setup Fetching**: Loading a car's cloud setups no longer blocks the render thread on a five-second HTTP call.

### 🧹 Internal
- **Shared-Memory Regression Tests**: Added layout tests that parse graphics, physics and static pages captured verbatim from a live AC 1.16.4 session through the same zerocopy call the app uses. Previously every test built an `Ac*` value in Rust and read it back, so no test could detect a mismatch with the game.
- Test suite now compiles under the workspace edition and lints; `unwrap_used` and `panic` were silently unenforced across it. Removed two modules that asserted nothing about this project — one never imported the crate under test, the other spawned `sh` and checked its exit status.
- CI builds with `--locked`. Release scripts and generated screenshots read the version from the workspace manifest instead of a hardcoded string two releases old.

### ⚠️ Changed Behaviour
- **Cold Tyre Pressure Targets Will Shift**: The pressure calculator scales its recommendation by `surface_grip`, which was previously stuck at `0.0` and clamped to a floor of `0.80` — so every recommendation carried the same fixed compensation. Now that real grip is read, a well-rubbered track (≈0.94) produces roughly a third of the previous adjustment. Recommendations will differ from v0.3.0 on the same car and track; this is the fix working, not a regression.
- **Fuel Strategy Becomes Active**: `fuel_x_lap` was a constant `0.0`, and both the fuel-remaining estimate and the race-fuel target are gated on it being positive, so they never ran. They now do. That field still sits in the part of the graphics page not confirmed against a live capture — but the estimate no longer depends on it alone, since consumption measured across completed laps fills in when it reads zero.
- **Fuel Warnings Reset Between Stints**: `fuel_laps_remaining` was never cleared once set, so after a pit stop or a session change the BOX BOX BOX warning could fire on a value measured before the stop.

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
