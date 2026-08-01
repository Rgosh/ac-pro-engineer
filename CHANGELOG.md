# Changelog - RaceEngineer (AC Pro Engineer)

All notable changes to this project will be documented in this file.

## [Unreleased]

### 🛡️ Bug Fixes & Stability
- **Shared-Memory Graphics Layout**: `AcGraphics` was using Assetto Corsa Competizione's `SPageFileGraphic` layout, which carries `activeCars`, `carCoordinates[60][3]`, `carID[60]`, `playerCarID` and `penalty` — 964 bytes that plain AC never writes. Every field from `car_coordinates` onward was therefore read from the wrong offset, past the end of the 360-byte page AC actually publishes. Track position was plotted from the car's altitude, and `surface_grip`, `fuel_x_lap`, `wind_speed`, `tc`, `abs`, `engine_map`, `flag` and the driver-stint timers all read a constant zero. Reported in [#2](https://github.com/Rgosh/ac-pro-engineer/issues/2).
- **Shared-Memory Regression Tests**: Added layout tests that parse graphics, physics and static pages captured verbatim from a live AC 1.16.4 session through the same zerocopy call the app uses. Previously every test built an `Ac*` value in Rust and read it back, so no test could detect a mismatch with the game.

### ⚠️ Changed Behaviour
- **Cold Tyre Pressure Targets Will Shift**: The pressure calculator scales its recommendation by `surface_grip`, which was previously stuck at `0.0` and clamped to a floor of `0.80` — so every recommendation carried the same fixed compensation. Now that real grip is read, a well-rubbered track (≈0.94) produces roughly a third of the previous adjustment. Recommendations will differ from v0.3.0 on the same car and track; this is the fix working, not a regression.
- **Fuel Strategy Becomes Active**: `fuel_x_lap` was a constant `0.0`, and both the fuel-remaining estimate and the race-fuel target are gated on it being positive, so they never ran. They now do. Note that this field sits in the part of the graphics page not yet confirmed against a live capture — see the note on the tail fields in `core/src/ac_structs.rs`.

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
