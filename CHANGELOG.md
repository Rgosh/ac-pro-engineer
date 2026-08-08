# Changelog - RaceEngineer (AC Pro Engineer)

All notable changes to this project will be documented in this file.

## [v0.3.5] - 2026-08-08

**The point:** the overlay stopped being "the panel that sometimes works". It
opens before the race and in the pits, keeps its settings when the window is
closed, and shows up to eight lines of advice instead of four. There is now one
overlay rather than two — the desktop window on F10 is gone. In the terminal,
the keys finally do what the bottom of the tab says they do, and every one of
them can be rebound.

> The panel still has not been checked in game by the author of these changes:
> it is driven under LuaJIT and LÖVE, and every `ui.*` call it makes is checked
> against the installed CSP, but that is not the same as a session. Report what
> breaks.

### ⚠️ Breaking

- **Overlay frame version 5.** Eight advice slots instead of four; the struct
  grew from 440 to 712 bytes. **`shm-bridge.exe` has to be updated** — a bridge
  built for 440 bytes maps too few, CSP silently refuses to open the mapping,
  and the panel waits forever for "AC Pro Engineer". **[B]** on the overlay card
  fetches a current one.
- **F10 and F11 are no longer bound to anything**, and the `--overlay-test-d`
  and `--overlay-test-vr` flags are gone. See below.

### 🗑 Removed

- **The desktop overlay on F10 and its control centre on F11.** There were two
  overlays. This one was a layered Win32 window drawn by the application
  itself, and on Linux it had no implementation at all: `OverlayManager` chose
  `None`, so F10 wrote a line to the log and did nothing. On Windows it drew a
  worse copy of what the CSP panel already draws — it did not survive exclusive
  fullscreen, never appeared in VR, and was invisible to AC's own screenshots
  and replays. Gone with it: `OverlayManager`, `native_window.rs` (422 lines of
  Win32), `openxr.rs` (a stub that never drew a pixel), `provider.rs`,
  `state.rs`, `ui/overlay.rs`, the `OverlayMode` modes, both key bindings and
  the two `--overlay-test-*` flags. There is one overlay now, and it is the CSP
  panel.
- A side effect worth knowing: the `SHOW_TELEMETRY` and `SHOW_ENGINEER` flags in
  the frame were the setting ANDed with a second switch on that manager. Nobody
  ever set the second one to false, but finding that out meant reading two
  structs. The frame now carries exactly what the settings say.

### ✨ Added

- **A slider for how many advice lines to draw, 1 to 8**, instead of four radio
  buttons, with the number the application actually sent printed underneath:
  "I asked for 8 and see 3" is the engineer having three things to say, not a
  setting that failed. The same limit was raised to 8 in the application
  (Settings → OVERLAY).
- **The panel works before the race and in the pits.** The application publishes
  a frame from its launcher screen and while AC has nothing in shared memory
  yet. The panel now tells two states apart: *waiting for the car* (everything
  is fine, telemetry starts on track) and *waiting for AC Pro Engineer* (the
  application is not running). It only ever said the second, which sent people
  hunting for a fault in the bridge and the Proton prefix that was not there.
  The status window gained a `car: in the garage / on track` row.
- **Your own keys** — a new **KEYS `[G]`** category in the terminal's settings.
  ENTER binds, DEL restores the default, ESC cancels; a key another action
  already holds is not written silently, it says which action holds it. Same on
  Linux and Windows, stored in `config.json` as text (`f1`, `ctrl+s`,
  `shift+tab`) so it can be edited by hand. Keyboard layout is handled: bind `s`
  and `ы` works too.
- **`shm-bridge.exe --verify`** opens the overlay mapping with exactly the call
  CSP makes and prints what is in it: the frame version, the sequence counter
  and the application's version. It is the one question that could not be
  answered from inside the prefix — *can a Windows process here see our frame at
  all*.

  ```
  protontricks-launch --appid 244210 shm-bridge.exe --verify
  ```

  No mapping, and it says what to start; a mapping that is empty, and it says
  the Linux side is not publishing.

### 🖼 Screenshots

- **No more SVG — PNG only.** Every screen was written twice: an SVG "as the
  exact record" and a PNG "to show". Nobody read the SVG, GitHub will not render
  one inline in a README anyway, and refreshing the screenshots put both in the
  diff. The SVG survives as an in-memory intermediate that the PNG is rasterised
  from — drawing a grid of coloured glyphs any other way would mean carrying a
  font rasteriser. `Ctrl+S` in the application saves a PNG too.
- **Pictures of the panel itself, one window per picture.** The README used to
  have exactly one picture of "the overlay", and it was the terminal's control
  centre for the overlay that no longer exists. There are now all five panel
  windows and all six tabs of its settings, drawn by the panel's own code.
  `apps/lua/love/portraits.sh` regenerates them: each run draws one window in a
  LÖVE window sized exactly to it, so there is nothing to crop.

### 🐞 Fixed

- **Pressing `[B]` on the launcher's overlay card killed the application.**
  `reqwest::blocking` builds a private tokio runtime and drops it while
  constructing a client, and dropping a runtime from a thread already inside one
  panics. `fetch_bridge_now` called it straight from the key handler, which runs
  inside `#[tokio::main]` — so the one key that fetches a bridge was the one key
  that could not be pressed. Both of the bridge's requests now run on a thread
  with no runtime context, and so do the Setup Cloud's two, which were the same
  shape and one keystroke away from the same crash. Every blocking request in
  the crate is now either behind that hop or the first thing on a thread of its
  own.
- **The panel forgot every checkbox, and the reason was not that it failed to
  save.** `settings = stored` made the panel's live settings table *be* CSP's
  `ac.storage` proxy, so every read and write went through its metatable. A
  proxy that accepts an assignment and does nothing with it did not merely fail
  to persist the value — it lost it outright, because there was no table
  underneath holding it. Tick a box, and the next frame read it back out of the
  proxy and drew it unticked again. The panel owns a plain table now; storage is
  read out of once and written to on save, and it is also cheaper, since these
  are read every frame in the draw path.
- **The settings are kept in a file as well.** `ac_pro_engineer_overlay.lua`, in
  the folder CSP names, written on every change and read at startup, winning
  over storage when they disagree — it is only ever written by a change the
  driver made. Plain text that can be opened, edited or deleted, which makes
  "did it save" a question with an answer; the Units tab shows the path. Where
  the Lua sandbox withholds file access the panel behaves exactly as before and
  says so.
- **Settings typed into the console did not redraw or save.** No
  `format.rebuild`, so a unit typed there did not reach the drawn strings until
  the next frame arrived from the application — with the feed stopped,
  `--units f` appeared to do nothing. And no `store.save`, so it lasted until
  some other control happened to trigger one. The seven one-press buttons go
  through the same function.
- **A checkout ran whichever bridge happened to be nearest, not the one built
  for it.** There were two different searches for `shm-bridge.exe`: the
  launcher's card and `bridge_probe` used one that knows about
  `target/x86_64-pc-windows-gnu/release/`, and the code that actually spawns the
  bridge had its own that did not — so the card could judge one file while the
  application launched another. Both searched the working directory first, so a
  single stale `shm-bridge.exe` at the root of a checkout shadowed the one just
  cross-compiled: the old one was spawned, reported out of date, and `[B]`
  offered to download a third. A bridge carrying this build's version now wins
  wherever it is, and the directory order only decides between copies that are
  all wrong. `cargo build --release -p shm-bridge --target
  x86_64-pc-windows-gnu` then `cargo run` uses what you just built, with nothing
  to copy or delete.
- **The key hints lied.** Every tab now has its own line at the bottom right,
  built from the same bindings that handle the keypress, so it cannot disagree
  with them; `the_hints_only_name_keys_that_do_something` walks every tab and
  requires the key a hint names to do what the hint claims. There used to be
  hints on two tabs of nine, and one of the two promised `'D' — Download` on a
  screen where `D` reached no handler at all. Also: `D` in the setup list now
  opens the browser, and the Analysis hint gained `E` for CSV export, which had
  worked all along and was written down nowhere.
- **The help page (F1) and the launcher line** are printed from the bindings
  too. The launcher named two keys out of six: `←/→`, `O`, `H` and `Q` all
  worked and were mentioned nowhere.
- **"F1: Dashboard", "F5: Analysis" — the tabs were never on function keys.**
  That is what the README's headings said, what the help pages' headings said,
  and what the guide said ("Look at the Analysis Tab (F5)"), while tabs actually
  switched on the digits. The help headings are printed from the binding now,
  and the README and the guide talk about the digit and the tab rather than a
  key that does nothing.
- **The panel forgot every setting when its window was closed.** The manifest
  said `LAZY = FULL`, which unloads the script when the last window closes —
  close the panel to look at the track, open it again, and the defaults are
  back. Saving also relied on assigning a value to itself in the storage proxy,
  which cannot be verified from this side. It is `LAZY = ON` now, only keys that
  actually changed are written, and each is read back after the write. The Save
  button says how many stuck.
- **The plate behind the engineer's advice took up a corner of the window.** The
  rectangle was drawn exactly 140 pixels tall: in the advice window that is a
  band across the top, with the text running out below it. It now fills the
  advice window, matches the block's height in the panel, and has symmetrical
  padding (it was 4 left, 3 top, 0 right).
- **`A / S / D` in the settings category caption** — there are five categories
  and three were named. The other two could only be reached with the arrows.
  It is `A/S/D/F/G` now.
- **Category names were clipped**: the width was counted in bytes, so `ОВЕРЛЕЙ`
  took twice the room the arithmetic thought it did and the key tag ran off the
  edge as `[F`.
- **"Dump settings to console"** wrote seventy lines into a buffer that keeps
  twelve, so only the end of the alphabet was ever visible. Three keys to a
  line, forty lines.
- **The tyre life bars were drawn over their own labels.** `Gauge::label`
  centres the text on the bar, so half the string sank into the coloured
  rectangle. Three columns now: wheel and percentage on the left, the bar in the
  middle, circles on the right. The bar scales between your `wear_critical`
  threshold and a new tyre rather than between 94 % and 100 %, so an empty bar
  means "finished by your own threshold" rather than "below 94".
- **`Calc...`** was a sentence cut in half that hung there for a whole stint.
  The projection needs a completed lap; until there is one, there is a dash.
- **The bar colours come from the same thresholds** as the engineer's advice —
  98/96 used to be hard-coded into them.
- **"Laps left" counted to `wear_warning − 2`** rather than to the end of the
  tyre, and "no data" was encoded as 99.0 — so a fresh set on a short lap looked
  like missing data.

### 🐞 Fixed — the panel and its harness

- **A nested `ui.tabBar` took the outer one down with it.** The harness set the
  current tab bar back to `nil` instead of restoring the parent, so every
  `ui.tabItem` in the *outer* bar after a nested one drew no label and ran its
  body unconditionally. The settings window lost four of its six tabs and drew
  their contents stacked under whichever one was selected.
- **`##id` was drawn as part of the label.** ImGui hides everything from `##`
  onward; the panel depends on it, because nearly every control is
  `ui.checkbox('##showHeader')` with the caption drawn beside it at a chosen
  size — CSP's font tiers cannot be scaled. Every setting in the harness read
  `##showHeader` in front of its name.
- **The corner readouts ran off the right edge of the telemetry window.** `row`
  is measured for "44.82 L": the value at 46 % of the width in body text. The
  string "26.8 psi 90°C 521°C 98%" did not fit at any window size, because the
  text scales with the width and the overflow stays the same fraction of the
  line. Those rows now use a narrow caption column and caption-sized text. The
  mapping name in the status window was losing its `.v1` for the same reason —
  and that is exactly the character worth reading there.
- **The `status` window was missing from the harness** although the manifest
  declares it alongside the other four. The fifth window could only be seen in
  game.

### 🧱 Structure

- **The Lua panel is split into modules.** It was one file of 2,429 lines; it is
  now `ac_pro_engineer.lua` (the entry point, 138 lines) and `acpe/` — settings,
  language, theme, layout, formatting, frame, blocks, widgets, console, and one
  file per window under `acpe/windows/`. The layering runs one way:
  settings → i18n/theme → layout → format → frame → blocks → windows. The
  installer writes the whole tree (19 files) and uninstalling removes the
  folders too; `every_lua_file_in_the_app_folder_is_shipped` will not let a new
  module be forgotten.
- The panel's version still equals the release's and is checked by tests —
  `PANEL_VERSION`, `VERSION` in the manifest and `Cargo.toml` have to agree.

### 🔧 The engineer's core

- **Four corners of one problem are now one line.** "FL COLD / FR COLD / RL COLD
  / RR COLD" filled every slot in the frame and read as noise. It is "All four
  COLD: 55 °C" now, and two wheels are named as an axle or a side ("Fronts",
  "Rears", "Left side"). Pressures, wear and brakes are folded the same way. The
  hysteresis stays per wheel — a flat spot on one tyre does not reset the timers
  on the others.
- **Wear no longer screams on lap three.** Critical was computed as
  `wear_warning − 2`, so with the default settings a tyre with 93.9 % left —
  the middle of a first stint — arrived as CRITICAL "WORN OUT". There is a
  separate `wear_critical` threshold now (85 % by default) with its own row in
  Settings → ENGINEER.
- **Pressure advice comes in the units you chose.** The temperature advice has
  gone through the formatter for a long time; the pressure advice printed raw
  psi, so anyone working in bar saw one number on the dashboard and a different
  one in the advice about it.
- **Brakes are named after wheels, not numbers.** It was "Brake 1"…"Brake 4" —
  the only place in the application where the corners were numbered.

### 📖 Documentation

- **The README was rewritten.** Installation separately for Windows and Linux, a
  section on the in-game panel and the bridge, every screen with a picture, the
  full key table, **every command-line flag** (there were none documented) and
  every environment variable, a reference for `config.json` with each key and
  its default, and a "what to do if" section with symptoms, causes and checks in
  order. Search keywords and section links, so a question about the application
  can be answered without opening the source.
- **The panel's windows are documented with the rest of the screens.** "Every
  screen" now covers both halves of the application: the terminal's nine tabs,
  then the panel's five windows and all six tabs of its settings — each with a
  description of what is in it and what it is for, rather than a one-line
  caption. The descriptions are written against the panel's code: what each
  threshold means, why the colour limits are yours, why the palette needs both a
  swatch and a picker, and which console command is the only way to turn
  developer mode on. The overlay section keeps installation and the bridge.
- **Everything in this repository is written in English.** The changelog's
  recent entries and the README's one Russian paragraph were not. Russian
  remains what it should be: a translation of the program, in `acpe/i18n.lua`
  and the terminal's own strings.
- **`ac_pro_engineer --help` finally says something.** Five of the seven flags
  had no description at all.

## [v0.3.4] - 2026-08-05

> ### ⚠️ The overlay in this release is a DEMO
>
> A preview, not a finished feature. It is published so that it can be checked
> on real machines, which is the one thing that cannot be done while developing
> it.
>
> - **It has never once been run on Windows.** The tests pass by
>   cross-compilation and clippy is clean against the Windows target, but not a
>   line of it has executed there.
> - **It has not been checked in game by the author of these changes.** The
>   panel is driven under LuaJIT and LÖVE and every `ui.*` call it makes is
>   checked against the installed CSP — that is not the same as a session.
> - Some console captions and the `Wear:`, `T:`, `B:` prefixes are not
>   translated yet.
>
> The official release comes after real sessions on both systems. Report what
> breaks: the panel's status window now shows every version at once.

**The point:** before this release the overlay could not work for anyone on
Linux. v0.3.3 was tagged eleven minutes before the commit that taught
`shm-bridge` to map the overlay, so every published bridge created only AC's own
pages. Confirmed by scanning the artifact.

### ⚠️ Breaking

- **Overlay frame version 4**: the application's version was added, so the panel
  can tell that the game is drawing an older copy of it. The field is last, so
  no offset moved. The application, the panel and `shm-bridge.exe` have to come
  from the same release; the panel installs itself, the bridge comes from **[B]**
  on the overlay card.

### 🐞 Fixed

- **The panel did not load at all.** `ui.Icons and ui.Icons.Settings` at file
  level: `ui.Icons` only has to be truthy to be indexed, and the table argument
  is built before `pcall`, so `pcall` does not protect it. Every window drew the
  error text instead of the panel.
- **Both developer-mode switches fell through to nil.** `applyDemo` and
  `DEMO_ADVICE` were declared below their callers, which makes them globals to
  those callers. The fourth instance of that trap here.
- **Both of the panel's versions lied.** `manifest.ini` showed `1.0` for eleven
  releases running, and the panel had no version of its own.
- **Both harnesses reported OK on a broken panel** — LuaJIT drew 27 strings
  instead of 140, and LÖVE did not count a load failure as an error.
- **The overlay card clipped its own diagnostics** at 66 columns.

### 🚀 Added

- **The bridge says who it is.** It writes `/dev/shm/acpe-bridge.info` and
  compiles its version into its own binary, so it can be identified without
  being run.
- **The card judges all three pieces**, and **[B]** downloads the published
  bridge, verifying it before it replaces anything. The old one is kept as
  `.previous`.
- **A bridge update check at startup** — it only looks; fetching is a keypress.
  The application's own version is not touched by this path.
- **The panel says the game is holding an older copy of it** and offers to
  restart AC. Switched off in Panel → Blocks.
- **`bridge_probe`** — which bridge is on disk, which is running, and whether
  the overlay can work at all.
- **`--export-overlay <dir>`** — write the panel out for a manual install.
- **`proton-setup.sh` in the archive** — the `protontricks` commands without
  which CSP does not load at all. There are no fonts in the archive and there
  cannot be: the terminal draws with its own font, the panel through CSP's
  DirectWrite, and fonts are installed into the prefix (`corefonts`), which is
  what the script does.


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
