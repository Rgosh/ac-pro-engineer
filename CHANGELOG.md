# Changelog - RaceEngineer (AC Pro Engineer)

All notable changes to this project will be documented in this file.

## [Unreleased]

## [v0.3.4] - 2026-08-05

> ### ⚠️ The in-game overlay in this release is a DEMO
>
> **This is a preview, not the finished feature.** It is published so it can be
> tested on real machines — which is the one thing that cannot be done from
> here. Expect rough edges, expect to report them, and do not assume a lap is
> safe because the panel says something.
>
> Known limits of this preview, stated plainly:
>
> - **Nothing has ever been run on Windows.** The whole suite passes
>   cross-compiled and clippy is clean for the Windows target, but no line of
>   this has executed on a Windows machine. The Windows path is the simpler one
>   — no bridge is involved, the application creates the named mapping itself —
>   but "compiles and passes tests" is not "works".
> - **Nothing has been run inside the game by the author of these changes.**
>   The panel is exercised under LuaJIT and under LÖVE, against a real published
>   frame, and every `ui.*` call it makes is checked against the installed CSP.
>   That is not the same as a session.
> - Some of the settings console's labels are still English in Russian, as are
>   the `Wear:`, `T:` and `B:` prefixes in the panel.
>
> The official, supported release follows once this has been through real
> sessions on both systems. Please report what breaks — the panel's status
> window now names every version involved, which is what a useful report needs.

Getting the in-game Lua overlay to the point where it can be handed to someone
else. Three components have to agree about a frame — the application, the panel
and `shm-bridge.exe` — and until now only two of them could be checked. Checking
the third found that no published release contains a bridge that can serve the
overlay at all, which is the reason this release exists.

### ⚠️ Breaking

- **The overlay frame is version 4.** It gained the application's release string
  so the panel can tell whether it is the copy the running application ships.
  The field is last in the struct, so nothing before it moves — but the panel,
  the application and `shm-bridge.exe` all have to come from this release
  together. Updating the application installs the matching panel by itself; the
  bridge is the one to replace by hand, or with **[B]** on the overlay card.

### 🐞 Bug Fixes

- **Both developer switches took the panel down.** `applyDemo` and
  `DEMO_ADVICE` sat below `script.update` and the advice block that use them,
  which makes them globals to their callers — that is, `nil`. Turning on "Demo
  numbers" called nil; turning on "Sample advice" indexed it. Neither is on by
  default, which is why every harness passed for as long as this was there, and
  why driving the windows could never find it. This is the fourth time a local
  declared after its callers has cost something here, so the harness now
  compiles the panel and fails on any name read from the global table that is
  not CSP's API or the standard library — verified by putting the bug back and
  watching it get caught.
- **No published `shm-bridge.exe` can serve the overlay.** v0.3.3 was tagged
  eleven minutes before the commit that added the overlay's mapping to the
  bridge's list, so the released binary maps AC's four `acpmf_*` pages and
  nothing else. It starts, reports no error, and the overlay mapping is never
  created — which on Linux is indistinguishable, from the driver's seat, from
  the application not running. Confirmed by scanning the published artifact: it
  does not contain `AcTools.CSP.Limited.ACPE.v1` anywhere. **This needs a
  release cut from a commit at or after `187b914`**; no code change can fix a
  binary that is already published. What is fixed is that the application now
  detects it, says so, and refuses to install one.
- **The panel's version was `1.0` through eleven releases.** `manifest.ini`
  carried a number that had never been updated, and the panel had no version of
  its own at all — only the frame-layout version, which most releases leave
  alone and which therefore says nothing about how old an installed panel is.
  Both now track the crate version, and two tests fail the build if they drift.
- **The LuaJIT harness reported OK for a panel that drew nothing.** With no
  application publishing, `readMemoryMappedFile` threw, every window took its
  "waiting for AC Pro Engineer" branch, and the check documented as the thing to
  run after every panel edit exercised none of the drawing — 27 strings rendered
  where a live panel renders 140. It now synthesises a frame when none is
  published and fails if the speed never reaches the screen.
- **The overlay card clipped whatever it had to say.** It was a fixed 66×15 with
  no wrapping, sized when it had five rows; anything longer than 64 columns lost
  its second half, which for a diagnostic is the half naming the remedy. It is
  now measured against its content and wraps.

### 🚀 New Features

- **The bridge says who it is.** `shm-bridge.exe` writes
  `/dev/shm/acpe-bridge.info` naming its version, the bridge protocol, the bytes
  it mapped and under what name, and removes it on a clean exit. It also
  compiles `ACPE-SHM-BRIDGE-VERSION=<version>;` into its own binary, so a bridge
  on disk and not running can still be identified — there is no running a
  Windows binary from Linux to ask it. A test asserts the marker survives a real
  release cross-build, because `#[used]` surviving LTO is not something to
  assume.
- **The launcher card judges all three pieces.** Frame version, release, and the
  bridge's version, protocol and mapped size, each against what this build
  needs. A bridge from another release that still maps enough bytes is reported
  as working, in yellow, rather than as a fault — a check that cries wolf stops
  being read. A bridge running without an announcement gets its own case and its
  own remedy: telling that driver to "start the bridge" sends them to start the
  same broken one again.
- **[B] fetches the published bridge.** It finds the asset dist actually
  publishes — `shm-bridge-x86_64-pc-windows-gnu.zip`, not the bare `.exe` that
  only v0.2.2 ever had — unpacks it, and verifies it before it replaces
  anything: a PE header, the overlay mapping's name in its bytes, and a version
  marker that agrees with the release tag. The previous bridge is kept as
  `shm-bridge.exe.previous`. Matching only `shm-bridge.exe` would have found
  nothing newer than v0.2.2 and offered that as an update.
- **`cargo run -p ac_core --example bridge_probe`.** Which bridge is on disk,
  which is running, and whether the overlay can work at all — the first thing to
  run when the panel waits with the mapping right there.
- **The panel reports its own version.** In the status window, the developer
  tab, the version-mismatch screen and the waiting screen, which is where a beta
  tester's screenshot is taken from. The waiting screen also names the bridge,
  because on Linux that is the missing piece as often as the application is.
- **The panel says when the game is drawing an old copy of it.** The application
  rewrites the panel's files at startup, but a game that was already running
  keeps the copy it loaded — and nothing on either side could see that: the
  files on disk are current, the frame version still matches, and the panel
  carries on. Every frame now carries the application's release, so the panel
  compares it against its own and says "restart Assetto Corsa to load it". It
  is one line, and it can be turned off in Panel → Blocks for anyone who cannot
  restart mid-session.
- **The application checks for a newer bridge at startup, and asks.** One
  background look at the release page; if there is a bridge worth taking, the
  card says so and **[B]** is what fetches it. Nothing is downloaded and no
  binary is replaced without a keystroke — and the application's *own* version
  is never touched by this, only the bridge. A bridge that cannot serve the
  overlay also forces the card up even for someone who turned it off with [D]:
  that preference means "stop telling me things are fine", not "stay quiet
  while the panel is broken".
- **`proton-setup.sh` ships in the Linux archive.** The four `protontricks`
  commands CSP needs — `vcrun2019`, `corefonts`, `d3dcompiler_47`, `dwrite` —
  in the order they have to run, with the checks that catch the two ways it
  goes wrong: protontricks missing, and the prefix not created because the game
  has never been launched. Without these CSP does not load at all, which reads
  as "the overlay broke my game".
- **`ac_pro_engineer --export-overlay <dir>`** writes the panel out for a manual
  install, for when the automatic one cannot work: an unwritable game folder, an
  install in a place the path search does not find, a second copy of AC. The
  files come out of the binary, so what lands is exactly the panel this build's
  frame is shaped for.

  A flag rather than a folder in the release archive, and not by choice: the
  panel's folder must be named `ac_pro_engineer` for CSP to find its entry
  point, and that is also the name of the Linux binary. Shipping both in one
  archive is a collision, and it failed the first v0.3.4 build outright with
  `File exists (os error 17)` — on Linux only, because the Windows binary has an
  `.exe` on the end.

### 📝 Note on fonts

No font files are shipped, and none can be. The desktop side is a terminal
application and draws with the terminal's own font; the in-game panel draws
through CSP's DirectWrite. The font step for Linux is `corefonts` **inside the
Proton prefix**, which is what `proton-setup.sh` runs — a font in a release
archive would not be where either of them looks.

## [v0.3.2] - 2026-08-04

A small follow-up to v0.3.1. Four pieces of functionality that were fully
implemented but had no way to reach the user are now wired up, one wrong
number in the analysis tab is corrected, and three things that ran far more
often than they needed to no longer do.

### 🚀 New Features

- **Screenshot the interface with Ctrl+S.** A complete SVG renderer for a
  drawn terminal buffer already existed inside `tui_tester`, where it
  generates the images in the README; the application itself had no way to
  capture what it was showing. Frames are written to
  `<data>/screenshots/<timestamp>.svg` and the path is reported in the status
  line. SVG keeps the text selectable and needs no image encoder.
- **Tyre pressure targets are on screen.** `ColdPressureCalculator` and
  `TyrePressureOptimizer` were both fully implemented in `ac_core` and called
  only by the test suite. A third Engineer sub-tab shows what to set the tyres
  to cold so they reach the configured hot target at the current air
  temperature and track grip, and what each corner's inner-versus-outer
  temperature spread says to change.
- **Frame and tick timing in the footer.** The render loop and the background
  tick thread contend for the same state mutex, so when one stalls it is
  usually because the other holds the lock — and from the outside both look
  identical, because the numbers stop moving either way. The footer now shows
  frames per second and how long ago the tick completed, in red past 500ms.

### 🛡️ Fixed

- **A missing sector split no longer zeroes the best sector.** The analysis tab
  computed each best sector as a plain minimum over the raw values, which
  includes the zeroes left by a lap whose split was never captured and by the
  unused third slot of a two-sector track. One such lap pinned that sector to
  0.000 and made the "Optimal" row a lap time no car could set. The analyzer's
  own `theoretical_best_lap_ms` — which filters those out and had no callers
  outside its unit test — is used instead, and a sector with nothing recorded
  renders as a dash rather than as a time.
- **The config is no longer rewritten on every launch.** The decision to save
  compared the file's text against a re-serialisation, so different
  indentation, a different key order, or a serialisation failure all triggered
  a write. The comparison is now between values, and formatting stops
  mattering. Migration and validation still write, which they must.
- **The mouse is no longer captured.** Capture was enabled at startup and no
  mouse event was ever handled, so the only effect was taking selection and
  copy away from the terminal — which is how anyone gets a lap time or an
  error message out of a TUI and into a bug report.
- **The timing readout stays blank until a frame is measured**, rather than
  reporting a fabricated "0fps" before anything has been drawn.

### ⚡ Performance

- **The delta-versus-best series is cached.** It was recomputed every frame,
  and computing it resamples two telemetry traces — cloning and fully sorting
  up to 7200 points each — to arrive at an answer that cannot change, since
  both laps are finished.
- **Setup folders are rescanned on a ten second heartbeat** instead of twice a
  second. The scan walks three directory trees and parses every setup ini in
  them, for a directory that changes only when the user saves a setup from
  inside the game.

### 🧹 Internal

185 tests, up from 171. The SVG renderer moved out of `tui_tester` into
`ui::screenshot` so the binary and the application share one implementation;
the README screenshots regenerate byte-identical from it.

## [v0.3.1] - 2026-08-03

A bug-fix release, and a large one. Three features that the interface has
always advertised — the version carousel, saving your settings, and the Setup
Cloud browser — did not work at all and now do. Four reachable crashes are
gone. Assetto Corsa is finally found on Linux.

47 commits, 171 tests (up from 130), green on Linux and Windows.

### ⚠️ Read This First

- **Your cold tyre pressure targets will change.** The calculator scales its
  recommendation by `surface_grip`, which used to read a constant `0.0` and
  clamp to a floor of `0.80` — so every recommendation carried the same fixed
  compensation regardless of track state. With real grip being read, a
  well-rubbered track (≈0.94) produces roughly a third of the previous
  adjustment. Numbers will differ from v0.3.0 for the same car and track.
  This is the fix working, not a regression.
- **Any settings you saved before this release were never written to disk.**
  The Settings tab did not persist anything, so it comes up with defaults one
  last time. From now on it saves as you edit.
- **Lap records saved before this release may be missing.** Personal bests
  were compared against the world record rather than your own history, so
  `records.json` only ever gained an entry from someone who had beaten it.

### 🚀 New Features

- **Assetto Corsa is found on Linux.** The install root was probed as four
  hardcoded Windows drive letters, so `content/cars` was never located and
  every car-spec lookup returned nothing. Setups were looked for in
  `~/Documents`, but under Proton the game is a Windows process writing inside
  its own prefix. The new `ac_paths` module walks the real Steam roots
  (`~/.steam/steam`, `~/.local/share/Steam`, Flatpak and Snap homes, Program
  Files on Windows), reads Steam's `libraryfolders.vdf` so a library on any
  drive is found rather than guessed at, and locates the Proton prefix by app
  id. `ac_install_path` and `ac_documents_path` in the config override both.
- **The Setup Cloud browser works.** The Setup tab handled only Up, Down and
  B, so pressing B opened a browser onto a permanently empty setup list with
  no way to install anything — while the tab's own hint line, the help overlay
  and the README all documented `D` to download. Arrows navigate, Enter
  reloads a car, `D` installs, PgUp/PgDn scroll the details. Fetching runs off
  the render thread, so the UI no longer freezes on a five-second HTTP call.
- **Fuel strategy no longer waits on AC.** Every fuel figure was gated on
  `gfx.fuel_x_lap`, which reads zero for the whole of lap one and sits in the
  part of the graphics page not yet confirmed against a live capture.
  Consumption measured across completed laps now fills in, so the strategy tab
  works from lap two regardless of that field.
- **Honest connection status.** The footer distinguishes `LIVE`,
  `AC RUNNING - NO DATA` and `AC NOT RUNNING` rather than collapsing three
  tracked states into ONLINE/OFFLINE. Panels with no telemetry say which it is
  instead of drawing nothing.
- **Richer CSV export.** RPM, lateral G, longitudinal G and average slip were
  being dropped even though the trace carries them — the three things an
  external tool is most often opened for. Files are named after the car, track
  and lap instead of colliding on `lap_3_export.csv`, and a failed export now
  reports itself instead of failing silently.
- **Terminal-too-small screen.** Below 80x20 the app shows its current and
  required size instead of drawing into an area that cannot hold the layout.
  The startup resize is now grow-only, so it stops shrinking the window of
  anyone running maximised.
- **Ghost delta.** The `show_ghost_delta` toggle now selects the delta source:
  with it on, the readout compares against your own recorded best lap through
  `calculate_ghost_delta`, which was fully implemented and had no caller.

### 🛡️ Crashes Fixed

- **Narrow terminals.** Four `Rect` fields in the Setup tab subtracted
  constants from a `u16` width and height. Below 20 columns they wrapped to
  around 65530 and indexed out of the render buffer.
- **Mid-download panic.** The updater's progress bar built its trailing
  segment with `"░".repeat(20 - filled)` on an unclamped percentage, so a
  response body longer than its Content-Length aborted the app while the user
  watched it update.
- **NaN from stale shared memory.** `Gauge::ratio` asserts its input is within
  0.0..=1.0 and `clamp` returns NaN unchanged, so a single garbage float from
  a zeroed `/dev/shm` page took the app down. All nine gauge call sites reject
  non-finite input first.
- **100% CPU from a config file.** `AppConfig::validate` had no caller outside
  its own unit test, so `update_rate: 0` reached `event::poll` and
  `thread::sleep` and spun two cores. Validation now runs on load, and covers
  the pressure targets, alert bands, temperature limits and shift point that
  previously had no bounds at all.

### 🛡️ Things That Silently Did Nothing

- **Version carousel arrows.** `check_for_updates` dropped every release older
  than the running one, so on the newest build the list held a single entry
  and Left/Right had nowhere to move — while the launcher rendered a "you
  won't be able to switch back" warning for versions that could never appear.
- **Update checks after being offline.** The check ran once at startup, so a
  machine behind a captive portal kept an empty carousel for the whole session
  with no way to retry. Selecting the UPDATE item now re-checks, debounced to
  once a minute.
- **Saving settings.** `handle_input` mutated the config and nothing wrote it
  back; `apply_config` had no callers, so changes did not take effect until a
  restart. The `auto_save` and `show_ghost_delta` toggles were read by nothing.
- **Personal bests.** Compared against the world record, and the whole block
  was nested inside a car-specs lookup that always failed on Linux — so no
  record was created, compared or saved there at all, which also left
  `world_record` as None and disabled the off-pace advice.
- **Setup auto-detection.** `match_score` can only produce 0/20/25/30/45/50/
  55/75 and the threshold was `> 60`, so only a perfect three-way match ever
  qualified. One lap of burnt fuel dropped it to 55 and silently blanked the
  "(NOW: x%)" hints in the brake-bias and camber advice.
- **Suspension roll-asymmetry warning.** It compared `avg_ride_height[0]`
  against itself, so the difference was always exactly zero. AC publishes ride
  height per axle, not per corner, so the check cannot be written against this
  data and has been removed rather than left looking functional.
- **Simulator detection on Linux.** `is_process_running` matched only
  `simulator.exe`, but the Linux build is called `simulator`, so the launcher
  waited forever on the platform the bridge exists for.

### 🛡️ Wrong Numbers

- **Driving-style aggression** combined the lateral and *vertical* G axes, so
  a stationary car scored 40% and braking or acceleration was invisible to it.
- **Out-laps scored perfect tyre management.** With no sample above the speed
  gate, pressure deviation computed to 0.0 and the score to a perfect 100 — an
  out-lap rated better than a hot lap, and the advice recommended inflating by
  27.5 psi against a 0.0 psi reading.
- **Mistake counts scaled with Update Rate.** Oversteer, understeer, lockup
  and scrubbing counters were divided by a fixed sample count, so changing the
  rate in Settings halved every score and made laps recorded at different
  rates incomparable.
- **The final sector split raced the lap counter** and could land in the
  following lap. It is derived from the lap time now. `AcStatic::sector_count`
  is honoured too, so 2- and 4-sector mod tracks produce a theoretical best.
- **Fuel targets under-fuelled.** A timed race ends when the leader
  *completes* the lap the clock ran out on, and the lap already in progress
  still has to be finished; the target accounted for neither.
- **Stale fuel warnings.** `fuel_laps_remaining` was never cleared, so
  BOX BOX BOX could fire after a refuel on a value measured before the stop.
- **Torn shared-memory reads.** The physics page is rewritten at 333 Hz while
  ~600 bytes are copied out of it. Pages are re-read when AC's `packet_id`
  moves mid-copy, so a frame spliced from two game ticks no longer reaches the
  jerk accumulators and peak-G tracking as a phantom lockup.
- **Track-map bounds** were serialised as `f32::MAX`/`f32::MIN` sentinels when
  a lap had no usable coordinates, so anything computing `max - min` from a
  saved lap got -6.8e38.
- **Units were ignored.** Target pressures printed a hardcoded "PSI" and
  ambient temperatures a hardcoded "C" whatever the Display settings said;
  alert thresholds printed no unit at all. Tyre temperature *spreads* were
  converted as absolute temperatures, adding a 32°F offset that does not
  belong to a difference. Min Speed was folded from a seed of 999.0, so an
  empty trace displayed "999.0 km/h" as if it were a measurement.

### 🛡️ Keys, Text and Alerts

- The first-run prompt could not be exited with Ctrl+C, q or Esc — the first
  screen every new user sees, and Enter was the only way out.
- F1 did not close the help modal that says "PRESS ESC, ?, Q, OR F1 TO CLOSE"
  in nine places.
- Esc in the analysis load menu quit the whole session back to the launcher,
  while the menu's own footer promised "ESC: Close".
- Held keys were dropped on Windows, which reports them as `Repeat` rather
  than `Press`.
- `S` in the analysis tab saved the fastest lap rather than the selected one.
- Tabs were documented as F1–F9 in nine screen titles, the navigation summary
  and the README; they are 1–9. The footer advertised "[H: Help]" for a key
  that is not handled, and F10 was described as a compact UI mode when it
  toggles the game overlay. Keys documented nowhere — Tab/Shift+Tab, F11,
  Ctrl+L, E, PgUp/PgDn, the A/S/D category switches — are now listed.
- Brake and tyre-temperature alerts pushed a fresh recommendation on every
  frame the condition held — roughly sixty a second per corner, burying every
  other message. They now use the same hysteresis as every comparable alert.
- Status messages never cleared, so "Exported CSV: ..." stayed pinned to the
  footer for the session and a stale message looked like a fresh one.
- Twelve locale keys existed only in Russian; a test now enforces parity. A
  malformed locale override produced an empty dictionary in silence,
  degrading the whole UI to raw key names.

### 🛡️ Data, Shutdown and Security

- **Durability.** The records file, config and CSV export renamed a temp file
  into place without flushing it first, so a power loss could publish a
  correctly-named empty file. Two instances saving at once also shared a temp
  path, which is the one way that pattern corrupts rather than merely loses.
- **Records validation.** A zero or negative lap time was accepted, written to
  disk, then dropped by the read path on next load — which reads to the driver
  as a personal best vanishing between sessions.
- **Crash reports and logs** were written relative to the working directory,
  unwritable when launched from a shortcut or installed under Program Files.
  The crash report was then dropped in silence. A logging failure also aborted
  startup before the TUI was drawn.
- **Stale `/dev/shm` mappings.** shm-bridge's cleanup returned on the first
  failure, leaving the remaining pages behind zero-filled — and the app maps
  those without complaint, reporting a healthy connection to a dead feed.
- **Quitting could hang forever** waiting on a bridge that never acknowledged
  the exit request. Bounded to five seconds, and errors inside that task are
  no longer discarded.
- **A missing `protontricks-launch` was fatal**, so anyone running AC natively,
  through another launcher, or simply reviewing saved laps offline could not
  start the app at all.
- **INI injection.** A newline in a downloaded setup's notes field opened a new
  line in the file AC parses as a car setup, letting a `[SECTION]` be smuggled
  past everything the downloader validates.

### ⚡ Performance

- `is_process_running` reads every process on the system and was called twice
  per frame from the launcher — roughly 124 full process-table scans a second
  while sitting in a menu. Cached for one second.
- Loading a car's cloud setups no longer blocks the render thread.

### 🧹 Internal

- **Shared-memory layout tests** parse graphics, physics and static pages
  captured verbatim from a live AC 1.16.4 session through the same zerocopy
  call the app uses. Previously every test built an `Ac*` value in Rust and
  read it back, so none could detect a mismatch with the game.
- **The test suite now compiles under the workspace edition and lints.** It
  was pinned to edition 2021 against the workspace's 2024 and omitted
  `[lints] workspace = true`, so `unwrap_used` and `panic` were silently
  unenforced across it. Two modules that asserted nothing about this project
  were removed — one never imported the crate under test, the other spawned
  `sh` and checked its exit status.
- **CI builds with `--locked`** and runs on `actions/checkout@v6`, matching the
  release workflow.
- **Version numbers come from the manifest.** The release scripts and the
  generated screenshots hardcoded `v0.2.3`, two releases behind.
- **Screenshots regenerated**, including `Help_Modal.svg`, which was
  byte-identical to `Analysis_Radar.svg` because the tester set a field the
  renderer does not read.
- **The commit convention is written down** in AGENTS.md.

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
