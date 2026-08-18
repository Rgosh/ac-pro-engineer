# Handoff — where the work stands

It was the overlay's handoff and it still carries most of that detail, because
the overlay is still the part with three pieces that have to agree. It is not
only that any more: there are two games under `core/src/games/` now, and the
bugs worth writing down came from the second one.

State as of 2026-08-18, with v0.4.1 prepared and uncommitted. Read this
first in a new session.

`main` is at v0.4.0; the working tree carries the 0.4.1 patch described at
the bottom of this file. 348 tests in the core, clippy and fmt clean on
Linux **and** `x86_64-pc-windows-gnu` — and as of this patch the Windows
target is *run* as well as built, through Wine. Check all of it before
pushing:

```bash
cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings
```

```bash
./tools/test-windows.sh
```

## The shape of the thing

The desktop application computes everything and publishes a 712-byte
`#[repr(C)]` `OverlayFrame` once per tick — frame version 5, which carries
eight advice lines rather than four. **Any `shm-bridge.exe` older than v0.3.5
maps 440 bytes, CSP refuses to open the mapping, and the panel waits forever
beside a file that is right there.** A CSP Lua app reads fields and calls
ImGui. Lua runs on AC's render thread where LuaJIT collects garbage mid-frame,
so the panel formats text when a frame *arrives*, not when one is drawn, and
allocates nothing per frame that can be allocated once.

| Piece | Where |
|---|---|
| Struct, generator, flags | `core/src/overlay/frame.rs` |
| Install / uninstall / describe | `core/src/overlay/install.rs` |
| The panel | `assets/frontends/csp-panel/` — entry point plus `acpe/` |
| Generated layout | `assets/frontends/csp-panel/frame_layout.lua` |
| Manifest, five windows | `assets/frontends/csp-panel/manifest.ini` |
| LÖVE harness | `apps/lua/love/` — see its README |
| LuaJIT harness | `apps/lua/tests/run_overlay.lua` |
| Engineer probe | `core/examples/engineer_probe.rs` |

Regenerate the layout after touching the struct, and rebuild the bridge if the
size changed:

```bash
cargo run -p ac_core --example gen_lua_layout > assets/frontends/csp-panel/frame_layout.lua
```

```bash
cargo build --release -p shm-bridge --target x86_64-pc-windows-gnu
```

## Three pieces must agree

The application, `shm-bridge.exe` and the panel all encode the same frame. Every
failure that cost an evening was one of them being older than the others:

- a bridge built before the frame grew maps too few bytes and CSP refuses to
  open the mapping — the panel says "waiting for Pro Engineer" while
  `/dev/shm` has the file, at the right size, with the app running
- a panel from an older install reads every field after the change at the wrong
  offset — that is where `-1.7e27` in place of a tyre pressure came from
- the struct's field order and the generator's `FIELDS` list drifting apart does
  the same thing, and size and field-count checks do **not** catch it.
  `the_generator_lists_the_fields_in_the_struct_s_order` now does.

All three are checked now. The launcher card reports the installed panel's frame
version *and* its release against the application's, and the bridge's version,
protocol and mapped size against what this build needs — see "the one thing
blocking a beta" below for what that check found.

## Checks that exist

```bash
luajit apps/lua/tests/run_overlay.lua
```

```bash
love apps/lua/love --test --settings
```

```bash
cargo run --bin simulator
```

```bash
cargo run -p ac_core --example engineer_probe
```

```bash
cargo run -p ac_core --example bridge_probe
```

```bash
./tools/test-game.sh acc
```

```bash
cargo run -p ac_core --example capability_matrix
```

The engineer probe is how "four tyres WORN OUT: 0.0%" was found: AC counts wear
down from 100, so all four corners at zero is a session that has not published
wear, not four destroyed tyres.

The bridge probe is the first thing to run when the panel says "waiting for AC
Pro Engineer" and `/dev/shm` has the file at the right size. It names the bridge
on disk, the bridge running, and which of them is the problem.

## What the panel does

Five windows in the manifest: panel, advice, settings, telemetry, status.
Settings has tabs — Panel (Blocks, Corners, Limits, Fields, State), Advice, Look
(Screen, Size, Colour), Units, Console, and a red Dev tab that only appears in
developer mode.

Over a hundred settings, persisted through `ac.storage` and saved by comparing
against the last written values at the end of any frame that changed something.
Text is drawn with `ui.dwriteText` at a size the driver sets, because CSP's five
font tiers cannot be scaled and a 4K screen needs more than the largest.

## Traps found the hard way

- **A local declared after its callers is a global to them.** This emptied the
  developer tab, the harness's control panel and the console, three times.
  Declare shared helpers above everything that uses them.
- `ui.colorButton` is a swatch, not a picker; `ui.colorPicker` edits in place and
  returns *whether it changed*, never a colour.
- `ui.begin` does not exist in the app SDK. CSP owns the window.
- `FUNCTION_SETTINGS` needs `FLAGS = SETTINGS` or nothing opens it, and the
  window it opens is CSP's and cannot be resized. `ui.addSettings` is how to ask
  for one with a size of your choosing.
- `ac.setWindowSizeConstraints` took the resize grip off *every* window in the
  app. Reverted; do not reach for it again without a way to test first.
- An array of strings comes back as raw cdata. Four named `string(64)` fields are
  the same bytes and read as Lua strings.
- A `str.replace` with no anchor is a silent no-op — two "split this tab into
  sub-tabs" edits did nothing and the tests still passed.

## Where this stands: v0.4.1, prepared and uncommitted

**Two games, not one.** The overlay is still Assetto Corsa's only — CSP is an
AC mod — but everything else reads both. `docs/ARCHITECTURE.md` is the plan and
`core/src/games/` is the half of it that exists.

The v0.3.4-era warning that used to be here is gone: the published bridge
creates the overlay mapping, and the three-piece version check is in place. What
replaced it is a different lesson, from the first real Competizione session:
**every remaining bug was the program reporting something nobody measured.**

Five of them, all fixed in the working tree:

- a slide held for half a lap counted as hundreds of incidents, because the
  counters added one per *sample* and divided by a fixed run length. `Episodes`
  in `core/src/analyzer.rs` counts an episode once, when it starts.
- the car-versus-driver verdict blamed the driver without checking whether the
  driving had varied at all — one half of its own two-part rule.
- ride height, tread temperature and wind were drawn as zeros on Competizione.
  Each is a capability now; `ride_height` and `wind` were added for it.
- "Bottoming out" fired on every lap of Assetto Corsa: the height is in metres
  and the threshold was written in millimetres.
- a telemetry page left by an earlier run read as a live session, because the
  bridge sized the file it created and never cleared it.

## What is left

1. **The slip thresholds on Competizione are not measured.** `Episodes` removed
   the absurd counts; whether 0.2/0.3 in `analyzer.rs` are the right numbers on
   *that* game is unanswered, and the honest way to answer it is a lap with
   `/dev/shm/acpmf_physics` sampled beside it. Do not guess them.
2. ~~**The bridge must ship with the application.**~~ It does, and
   automatically: `dist` publishes `shm-bridge-x86_64-pc-windows-gnu.zip`
   beside the application's archives on every tag — check any release's assets.
   Nothing to sequence by hand. What is still true is that [B] fetches from a
   *published* release, so the fix is not available to anyone until the tag is
   pushed.
3. **54 mutants unrun** in `car_class.rs` and `driver_vs_car.rs`.
   `cargo mutants -p ac_core --file <path>` — it is slow and it competes with
   whoever is using the machine, so run it deliberately. Everything it found in
   `analyzer.rs` and `debrief.rs` has been closed.
4. **The panel's strings are translated; the console's settings labels are only
   partly**, and `Wear:`, `T:` and `B:` in `formatFrame` are English in both
   languages.
5. **`fetch_bridge_now` blocks the UI thread** for the length of a download. One
   small file behind an explicit keystroke, so it has not been worth a thread.

## Done since the last handoff

1. ~~**Nothing has been run on Windows.**~~ It runs now — `./tools/test-windows.sh`
   puts the whole workspace through Wine as a Windows binary. Not a substitute
   for the real thing, and most of the distance to it.
2. ~~**Version-check the bridge.**~~ `/dev/shm/acpe-bridge.info`, the compiled-in
   marker, and `bridge_probe`.
3. ~~**Package the bridge.**~~ [B] on the launcher card fetches and verifies it.

## The environment

| | |
|---|---|
| AC | `~/.steam/steam/steamapps/common/assettocorsa`, v1.16.4 x64 |
| CSP | 0.2.11 b3465 |
| Prefix | `steamapps/compatdata/244210/pfx`, Proton 9.0 / GE-Proton9-2 |
| Bridge | `~/projects/RaceEngineer/shm-bridge.exe`, run through protontricks |
| Panel installs to | `assettocorsa/assets/frontends/csp-panel/` |

There is a second copy of the app in `extension/lua/ac_pro_engineer/` on this
machine; both are written during development. Delete the second before anyone
else sees this.

Getting AC, CSP and Content Manager running under Proton is in the README under
"Getting Assetto Corsa, CSP and Content Manager to run under Proton".
