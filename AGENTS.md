# RaceEngineer Codebase Context & Agent Reference Guide

Welcome! This document (`AGENTS.md`) provides a concise reference for AI agents and developer tools working on the **RaceEngineer** (AC Pro Engineer) codebase.

---

## 1. Project Overview & Sitemap

RaceEngineer is a high-performance, cross-platform telemetry analysis and real-time race engineering suite for **Assetto Corsa** (AC / ACC), written in Rust.

```
RaceEngineer/
├── Cargo.toml               # Workspace manifest (Edition 2024, resolver = 3, workspace lints)
├── core/ (ac_core)          # Telemetry processing, setup manager, engineer rules, telemetry analyzer
│   └── src/
│       ├── ac_structs.rs    # Memory layout structs matching Assetto Corsa SHM
│       ├── analyzer.rs      # Telemetry analyzer and telemetry recording
│       ├── config.rs        # AppConfig (JSON configuration, theme, language, paths)
│       ├── content_manager.rs# Car and track content reader
│       ├── discord.rs       # Discord Rich Presence integration
│       ├── engineer.rs      # Real-time engineer recommendations and advice engine
│       ├── memory.rs        # Cross-platform shared memory reader (Win32 / dev/shm)
│       ├── overlay/         # In-game / VR / desktop overlay manager
│       ├── process.rs       # Process checker for acs.exe / simulator.exe
│       ├── records.rs       # Track and car lap record manager
│       ├── session_info.rs  # Active session metadata
│       ├── setup_manager.rs # Car setup reader and cloud setup synchronization
│       └── updater.rs       # GitHub auto-updater
├── tui/ (ac_tui)            # Interactive Ratatui TUI dashboard & control center
│   └── src/
│       ├── lib.rs           # Shared TUI library exports (AppState, AppTab, AppStage, SafeLock)
│       ├── main.rs          # ac_pro_engineer binary entry point and main event loop
│       ├── bin/
│       │   ├── simulator.rs # Mock telemetry simulator binary
│       │   └── tui_tester.rs# Automated headless TUI menu & action test runner (generates SVG/txt screenshots)
│       └── ui/              # TUI components, layouts, theme engine, and tab widgets
├── shm-bridge/              # Shared Memory Bridge binary for Wine/Proton to Linux /dev/shm
└── tests_suite/             # Integration tests for core logic, Linux distro mocks, & overlay
```

---

## 2. Key Commands Quick Reference

All commands must pass cleanly without warnings or errors.

- **Check Compilation**:
  ```bash
  cargo check --workspace
  ```
- **Lint with Clippy** (strict workspace lints, no `unwrap` or `panic` allowed):
  ```bash
  cargo clippy --workspace --all-targets
  ```
- **Run Unit & Integration Test Suite**:
  ```bash
  cargo test --workspace
  ```
- **Run Automated TUI Menu & Action Visual Test Runner**:
  ```bash
  cargo run --bin tui_tester
  ```
- **Run Main Application**:
  ```bash
  cargo run --bin ac_pro_engineer
  ```

---

## 3. Cross-Platform Guidelines (Linux & Windows)

1. **Shared Memory (`shm-bridge`)**:
   - `shm-bridge` runs under Wine/Proton to map Windows named shared memory objects (`acpmf_physics`, `acpmf_graphics`, `acpmf_static`) directly to Linux `/dev/shm`.
   - Always gate Win32 API calls (`windows` crate, `std::os::windows`) with `#[cfg(target_os = "windows")]` or `#[cfg(windows)]`.
   - Provide non-Windows compilation stubs so `cargo check --workspace` succeeds on Linux native targets.

2. **Overlay Manager**:
   - `OverlayMode::NativeDesktop` and `OverlayMode::StandaloneTest` rely on Win32 overlay windows on Windows. On non-Windows targets, match arms must fallback cleanly to `None`.

---

## 4. Code Quality Standards

- **No `unwrap()` Calls**: Never call `.unwrap()` in production or library code. Use `anyhow::Context`, `?` operator, `expect()`, or safe fallbacks (`unwrap_or_default`, `unwrap_or_else`).
- **Mutex Safety**: Use the `SafeLock` extension trait (`mutex.safe_lock()`) instead of raw `.lock().unwrap()` to avoid panicking on poisoned mutexes.
- **TUI Visual Testing**: Always run `cargo run --bin tui_tester` after modifying UI tabs or widgets. Inspect generated visual outputs in `screenshots/` to verify layout rendering across English and Russian languages.
- **Git Branching**: Development work must take place on a dedicated branch with incremental commits after each verified build. History uses `fix/...` and `feature/...` prefixes.

---

## 5. Commit Messages

Conventional Commits, all in **English**:

```
type(scope): lowercase imperative summary
```

- **Types in use**: `feat`, `fix`, `test`, `docs`, `style`, `chore`, `ci`, `perf`, plus a bare `release:` for version bumps.
- **Scopes in use**: the module touched — `shm`, `updater`, `setup`, `engineer`, `analyzer`, `config`, `records`, `memory`, `keys`, `ui`, `i18n`, `paths`, `io`, `process`, `platform`, `dist`, `ci`, `changelog`.
- **One commit per change.** A bug fix, a feature and a version bump are three commits, not one.

Bodies are wrapped at ~72 characters and answer three questions:

1. **What was wrong** — the specific mechanism, not "fixed a bug".
2. **Why it mattered** — what the user saw, or what a reader would wrongly
   conclude from the old code.
3. **How the change was verified to bite** — ideally the assertion that fails
   without it. See `4662354` and `ca8b1bb` for the house style.

Note that comments and commit messages are written in English throughout.
User-facing strings are bilingual (English/Russian) via `data/locales/`, and
`KeyCode::Char('й')`-style aliases exist so Russian keyboard layouts reach the
same shortcuts — neither is a reason to write Russian in code or history.
