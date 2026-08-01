# Changelog - RaceEngineer (AC Pro Engineer)

All notable changes to this project will be documented in this file.

## [v0.3.0] - Unreleased

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
