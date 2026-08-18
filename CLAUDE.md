# CLAUDE.md — working on RaceEngineer

`AGENTS.md` describes the codebase; this describes how to work in it without
repeating the mistakes already made. Read `docs/HANDOFF.md` for where the
overlay work stands right now.

## What this project is

A telemetry and race-engineering suite for Assetto Corsa and Assetto Corsa
Competizione, in Rust, with two faces so far: a terminal application (`ac_tui`,
the binary `ac_pro_engineer`) and an in-game panel written in Lua for CSP
(`assets/frontends/csp-panel/`). **The panel is Assetto Corsa's only** — CSP is
an AC mod and ACC is Unreal Engine.

"So far" is doing work in that sentence. `docs/ARCHITECTURE.md` is the plan:
one folder per game under `core/src/games/`, one `Sink` per place the computed
frame goes, and any number of front ends. **Read it before adding anything that
knows what game is running or where the data comes out.** Two parts of it are
already built:

- `core/src/games/assetto_corsa/` and `assetto_corsa_competizione/` — the
  structs, the paths and the shared-memory readers. The reader used to live in
  `tui/src/lib.rs`, which meant a user interface owned the connection to the
  simulator. It does not now: the core reads the game and the interfaces read
  the core. Which of the two is read is `config.game`, chosen on the launcher —
  see *Adding a sink, or a game* for why it is not detected.
- `core/src/broadcast/` — the computed frame goes to a list of sinks. Shared
  memory for the in-game panel, UDP as JSON for anything else: a second front
  end, a friend watching from another machine, a relay for a championship.
  **A sink may be slow, may fail, may vanish, and the tick must not notice** —
  `publish` hands the frame over and never waits, and a sink that fails sixty
  times running is dropped rather than logged sixty times a second.

The split matters. **The application computes; the panel draws.** Lua runs on
AC's render thread, where a millisecond is a sixth of the frame budget at 165 Hz
and LuaJIT collects garbage mid-frame. Anything that can be computed on the
desktop side must be, and the panel formats text when a frame *arrives*, never
in the draw path.

## The frame contract

`core/src/overlay/frame.rs` owns a 712-byte `#[repr(C)]` `OverlayFrame` and the
generator that emits its Lua declaration. Three artefacts encode it:

1. the application, which writes it,
2. `shm-bridge.exe`, which maps it into the Wine prefix on Linux,
3. `assets/frontends/csp-panel/frame_layout.lua`, which the panel reads it with.

**Changing the struct means changing all three.** After any edit to the fields:

```bash
cargo run -p ac_core --example gen_lua_layout > assets/frontends/csp-panel/frame_layout.lua
```

```bash
cargo build --release -p shm-bridge --target x86_64-pc-windows-gnu
```

Bump `OVERLAY_VERSION` and `EXPECTED_VERSION` in the panel together —
`the_panel_reads_the_frame_this_build_writes` fails if they disagree, which
saves the evening where the panel loads, reads every offset correctly and draws
nothing but "Version mismatch". Field **order** must match between the struct
and `FIELDS` — size and count matching is not enough, and a mismatch reads eight
bytes of one field as another
(`the_generator_lists_the_fields_in_the_struct_s_order` catches it now).

`MESSAGE_SLOTS` is eight, and the panel counts from `#MESSAGE_KEYS` rather than
from a literal. Growing it means a line per slot in `FIELDS`, a name per slot in
the panel's `MESSAGE_KEYS`, and both harnesses' `ffi.cdef` —
`the_panel_names_every_advice_slot` catches the one that is easiest to miss.

Adding a bit to `flags` costs nothing: no layout change, no version bump.
Prefer that to a new field when the answer is yes-or-no.

## Versions, and which one answers which question

Four numbers, and confusing them wastes an evening:

| Number | Where | Changes when |
|---|---|---|
| `OVERLAY_VERSION` / `EXPECTED_VERSION` | `frame.rs`, the panel | a field moves (5 as of v0.3.5) |
| `app_version` in the frame | filled by `OverlayFrame::empty` | every release, on its own |
| `BRIDGE_PROTOCOL` | `bridge.rs`, `shm-bridge/src/main.rs` | the bridge's note gains a key |
| `PANEL_VERSION`, manifest `VERSION` | the panel, `manifest.ini` | every release |
| Cargo `version` | `Cargo.toml` | every release |

The last two must be **the same string**, and tests fail if they are not:
`the_panel_announces_this_builds_version` and
`the_manifest_announces_this_builds_version`. Bump `Cargo.toml` and both Lua
files together.

The frame version says nothing about how old a panel is — most releases leave
the struct alone — which is why `PANEL_VERSION` exists and why the launcher card
shows both.

The frame also carries the *application's* release, so the panel can notice that
the game loaded an older copy of it than the one now on disk. That case is
invisible from every other angle: the files are current, the frame version
matches, and the panel keeps drawing. Only the panel knows which copy the game
has in memory, and only if it is told what the current version is.

## The bridge is the third piece, and it is checkable now

`shm-bridge.exe` writes `/dev/shm/acpe-bridge.info` naming its version, the
bridge protocol, the bytes it mapped and under what name; it removes the file on
a clean exit. It also compiles `ACPE-SHM-BRIDGE-VERSION=<version>;` into its own
binary, so a bridge sitting on disk and not running can still be identified —
there is no running a Windows binary from Linux to ask it.

```bash
cargo run -p ac_core --example bridge_probe
```

Says which bridge is on disk, which is running, and whether the overlay can work
at all. Run this **before** looking anywhere else when the panel says "waiting
for Pro Engineer" with the mapping right there in `/dev/shm`.

A bridge older than the frame maps too few bytes, CSP silently refuses to open
the mapping, and nothing reports an error. A bridge older than the overlay maps
AC's four `acpmf_*` pages and never creates the overlay mapping at all — that is
what every release up to and including v0.3.3 published, because v0.3.3 was
tagged eleven minutes before the commit that added it.

`bridge_update` fetches a published bridge and **refuses one that does not carry
`AcTools.CSP.Limited.ACPE.v1` in its bytes**, so it cannot install a downgrade
into that bug. Until a release ships a bridge built after `187b914`, the only
working bridge is one built here.

## Verify before claiming

Nothing about the overlay can be confirmed by reading code. Four checks, in
ascending cost:

```bash
luajit apps/lua/tests/run_overlay.lua
```

Drives every `script.window*` the panel exposes under a real LuaJIT with the CSP
API stubbed. Catches nil calls, arithmetic on strings, and dead draw paths.

It synthesises a live frame when no application is publishing, and **fails if the
speed never reaches the screen**. Without both it only ever ran the "waiting for
Pro Engineer" branch and reported OK for a panel that drew nothing —
27 strings instead of 140. `ACPE_ALL=1` prints every one of them, which is how a
wrong unit or an untranslated caption is caught without launching the game.

Four more things it checks, each of which was once a bug that shipped:

- **tab bodies actually run.** `ui.tabItem` used to fall to the catch-all stub,
  which calls nothing — so every one of the fifteen settings tabs was skipped
  and `windowSettings: OK` meant a tab bar had been constructed. Running them
  took the count from 150 drawn strings to 261.
- **widgets report that nobody clicked them.** The catch-all returns `0`, and
  `0` is truthy in Lua, so the moment the tab bodies ran every
  `if ui.checkbox(...)` fired at once and inverted every toggle in the panel.
- **settings survive a reload.** `ac.storage` is stubbed and outlives a reload
  of the script, which is what CSP does when a window is reopened.
  `sliderMoved` drags one slider the way a driver would.
- **a frame with no car is not a missing application.** `carPresent = false`
  clears `CONNECTED`, and the panel has to say it is waiting for the car rather
  than for the application.

```bash
love apps/lua/love --test --settings
```

Runs the panel under LÖVE for 120 frames and exits non-zero if it threw. Add
`--shot name.png` to get a picture; the harness's own README explains the rest.

```bash
./apps/lua/love/portraits.sh
```

Every picture of the panel in the README, regenerated: each window on its own
and each settings tab on its own, cropped to the window. Run it after any panel
edit that changes what is on screen — it is also the fastest way to *see* a
layout change, since it takes about a minute and needs no game.

Two harness bugs hid real ones until v0.3.5, and both are worth knowing about
because they made the harness agree with a panel that was wrong:

- **a nested `ui.tabBar` cleared `currentTabBar` instead of restoring its
  parent**, so every `ui.tabItem` in the *outer* bar after a nested one drew no
  label and ran its body unconditionally. The settings window lost four of its
  six tabs and stacked their contents under whichever one was selected.
- **`##id` was drawn as part of the label.** ImGui hides everything from `##`
  onward; the panel relies on that for nearly every control, because the
  caption is drawn separately at a chosen size. Every setting read
  `##showHeader Speed and gear`.

```bash
cargo run --bin simulator
```

```bash
cargo run -p ac_core --example engineer_probe
```

Fake AC telemetry into `/dev/shm`, then the engineer's advice printed next to
the numbers that produced it. This is how false "four tyres WORN OUT" was found.

```bash
cargo run -p ac_core --example capability_matrix
```

What every game measures, read out of the registry — the fastest answer to
"why has this advice gone quiet on this game", and the copy to check the site's
published table at `/games/` against.

While working on one game, its own tests and the core are the two questions
worth asking after every change, and the rest is noise you have already read:

```bash
./tools/test-game.sh acc
```

One game's folder plus its layout tests, then everything in the core that does
not name a simulator, then the boundary tests. `ac`, `acc`, `core` and `all`.
**It is not the full run** and says so — the other game, the terminal, the
translations and the screenshots are not in it.

And the whole suite, on both targets, before pushing:

```bash
cargo test --workspace
```

```bash
cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings
```

And the Windows half can now be **run**, not only built:

```bash
./tools/test-windows.sh
```

Wine, a scratch prefix, and the same suite as a Windows binary. This is the
difference between "it compiles for that target" and "the Win32 calls in it
return what the code expects" — process enumeration, named mappings, paths with
drive letters. Fewer tests run than on Linux, which is correct: the bridge and
the `/dev/shm` paths are `#[cfg(not(target_os = "windows"))]`. It is not a
substitute for a real Windows machine, and it is most of the distance to one.

## How the panel is laid out

One file per thing, under `assets/frontends/csp-panel/`:

```
ac_pro_engineer.lua      the entry point CSP loads, and nothing else
frame_layout.lua         GENERATED — see the note at its top
manifest.ini             the windows CSP opens
acpe/settings.lua        what the driver chose, and making it stick
acpe/i18n.lua            the panel's own words, in two languages
acpe/theme.lua           colours, accents, the editable palette
acpe/layout.lua          text sizes, spacing, the measured window
acpe/format.lua          numbers into strings, once per settled frame
acpe/frame.lua           the shared block and the snapshot drawn from it
acpe/blocks.lua          one function per thing on screen
acpe/controls.lua        the widgets the settings window is built from
acpe/console.lua         typed commands, for what has no widget
acpe/windows/*.lua       one file per window
```

The layering runs one way and only one way: settings → i18n/theme → layout →
format → frame → blocks → windows. A `require` that goes back up that list is a
cycle, and LuaJIT will hand the requiring module a half-built table rather than
fail.

Two things live in the entry point and nowhere else: `EXPECTED_VERSION` and
`PANEL_VERSION`. `acpe/frame.lua` is where they are *compared* against a frame,
and it is handed them by `frame.configure` — but the installer greps
`ac_pro_engineer.lua` for both, and three cargo tests read them from there.

**A new module has to be added to `FILES` in `core/src/overlay/install.rs`.**
`include_bytes!` takes a literal path, so the list is written out by hand;
`every_lua_file_in_the_app_folder_is_shipped` fails when one is missed, because
the alternative is an install missing a `require` target — which fails at load,
in the game, with every window drawing the error.

## Adding a sink, or a game

Both have a shape now, and both are meant to be additive:

- **A sink** implements `broadcast::Sink`: a name, `publish`, and optionally a
  `min_interval` if it wants a slower rate than the tick. Remote ones should:
  a spectator cannot tell above about ten a second, and twenty cars at tick rate
  is megabytes a second arriving at a relay.
- **A game** is a folder under `core/src/games/` implementing `Source`. Say
  honestly what it can report in `Capabilities` — **"not measured" and
  "measured as zero" are different answers**, and every wrong verdict this
  project has shipped came from confusing them: four tyres reading zero wear
  reported as four destroyed tyres, a camber verdict about a lap that published
  no temperatures.

There are two now — Assetto Corsa and Competizione — and the second one is what
made the abstraction real. Three things it taught, which apply to the third:

- **Pin the layout to a recording, never to a header.** `tools/record-session.sh`
  runs the bridge in the game's prefix and watches every four-byte word for a
  whole session; what a word *did* over two laps identifies it, where one
  snapshot does not. `tests_suite/src/acc_layout_tests.rs` keeps the whole
  2048-byte mapping rather than the struct's worth, so it can also assert that
  nothing is written past the end — the check that says the struct is not too
  *short*.
- **Two games can publish under the same names.** ACC inherited AC's three
  `acpmf_*` pages and changed the layout, and on Linux both mirror into the same
  `/dev/shm` files. Each reader refuses a page declaring the other's
  shared-memory version — AC 1.7, ACC 1.9 — because the alternative is not an
  error but plausible numbers.
- **Which game is a choice, not a detection.** `config.game`, set on the
  launcher, and `registry::chosen` is the only thing that answers it. The bridge
  has to be in one game's Proton prefix before that game starts, so there is
  nothing to detect at the moment it matters. `detect_running` still says
  whether the chosen game is up, and that is all it decides.

What has *not* been done for ACC is the part that is not plumbing: its
thresholds are still Assetto Corsa's. See `docs/plan-acc.md` §10 before
trusting a line of advice on that game.

## The terminal's key map

`tui/src/keys.rs` is the only thing that decides what a key does, and the only
thing that prints one. Bindings live in `config.json` as text (`f1`, `ctrl+s`,
`shift+tab`); `resolve` turns a keypress into an `Action`, `describe` turns a
binding into something to draw.

**Do not write a key name into a string.** Every hint, the help overlay and the
Settings screen read from `keys::all` / `keys::hints`, and
`the_hints_only_name_keys_that_do_something` walks all nine tabs and insists
each key a hint names resolves to the action the hint claims on that tab. That
test exists because the Setup tab promised `'D' - Download` on a screen where
`D` reached no handler.

Adding an action means: a field on `KeyBindings`, an entry in `keys::all`, a
case in `keys::set`, a case in `keys::action_of`, and an arm in `resolve`. Two
tests count fields off the serialised struct, so forgetting the first three
fails rather than going quietly missing. Removing one means the same five
places, plus every test that reached for it by index — `keys::all` is ordered,
and the Settings screen's tests address rows by number.

**There is one overlay, and no key toggles it.** F10 and F11 used to drive a
second one: a layered Win32 window drawn by the application, with a control
centre in the terminal. On Linux its provider was `None`, so F10 logged a line
and did nothing; on Windows it drew a worse copy of what the panel draws. Gone
in v0.3.5, along with `OverlayManager`, `native_window.rs`, `openxr.rs`,
`provider.rs`, `state.rs`, `ui/overlay.rs` and the `--overlay-test-*` flags.
`core/src/overlay/` is now only the frame and the things that carry it.

## Lua traps that have cost real time

- **A local declared after its callers is a global to them — that is, nil.**
  This emptied the developer tab, the harness's control panel and the console,
  three separate times, and the tests passed each time because the file still
  loaded. Declare shared helpers above everything that uses them.
- `ui.begin` does not exist in CSP's app SDK. CSP owns the window; the script
  draws contents.
- `FUNCTION_SETTINGS` does nothing without `FLAGS = SETTINGS`, and the window it
  opens is CSP's, unresizable. `ui.addSettings` asks for one with a size.
- `ui.colorButton` is a swatch; `ui.colorPicker` edits in place and returns
  whether it changed, never a colour.
- An array of `string(64)` comes back as raw cdata. Four named string fields are
  the same bytes and read as Lua strings.
- `ac.setWindowSizeConstraints` removed the resize grip from every window in the
  app. Do not reach for it without a way to test first.
- **`require` is cached, and a "reload" that does not clear `package.loaded`
  is not a reload.** CSP throws the whole Lua state away between loads; both
  harnesses have to do the same by hand, or a reloaded entry point gets the
  module instances the previous load left behind, with their frame already read
  and their settings already applied.
- **A cdata reference does not keep its owner alive.** `b[0]` on an
  `ffi.new('F[1]')` is a reference into `b`; let `b` go out of scope and the
  next collection frees the memory underneath it, after which every field reads
  as zero. This sat in the LuaJIT harness undetected for as long as the panel
  was one file — nothing allocated enough between opening the mapping and
  reading it to trigger a collection. Twelve modules did.
- **`LAZY = FULL` in the manifest loses everything the script holds.** CSP
  unloads the script when the last window closes and loads it again when one
  opens, so a driver who closed the panel to look at the track got the defaults
  back. It is `LAZY = ON` now. Anything that has to survive that has to be in
  `ac.storage`, and the storage write has to be a *change* — assigning a key to
  itself relies on the proxy writing a value it already holds, which nothing on
  this side can check.
- CSP's five font tiers cannot be scaled. `ui.dwriteText(text, size, colour)`
  draws at any size, which is the only way the panel is readable at 4K.

## Editing style

- Match the surrounding code: comment density, naming, and the habit of
  explaining *why*, not what.
- A `str.replace` with no anchor check is a silent no-op. Two "split this tab
  into sub-tabs" edits did nothing and nothing failed. Assert the anchor exists.
- Run the Lua harnesses after **every** panel edit. They take a second.
- Install into the game folder after a panel edit if it is being tested there:
  the app installs on startup, but a running game has the old copy.

## Working on the user's machine

- Assetto Corsa may be running while you work. Anything that touches the prefix,
  the game folder or the windows can break a live session — screenshots are the
  only way to see the result, and only the user can take them.
- Before blaming code for something in game, check the obvious: is the running
  application the current build, and does `/dev/shm/AcTools.CSP.Limited.ACPE.v1`
  have the size this build writes? Two evenings went to a stale binary.
- Prefer reverting a change that broke something in the game over iterating on
  it blind.

## Commit and release

Conventional Commits in English, one commit per change, bodies that say what was
wrong, why it mattered and how it was verified — `AGENTS.md` has the detail and
the examples. Push only when asked; never tag or release without being asked.
