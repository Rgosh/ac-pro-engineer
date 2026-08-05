# Handoff — the in-game overlay

State as of 2026-08-05. Read this first in a new session.

`main` is pushed to `origin`, 224 tests, clippy and fmt clean on Linux **and**
`x86_64-pc-windows-gnu`. Check both before pushing:

```bash
cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings
```

## The shape of the thing

The desktop application computes everything and publishes a 424-byte
`#[repr(C)]` `OverlayFrame` once per tick. A CSP Lua app reads fields and calls
ImGui. Lua runs on AC's render thread where LuaJIT collects garbage mid-frame,
so the panel formats text when a frame *arrives*, not when one is drawn, and
allocates nothing per frame that can be allocated once.

| Piece | Where |
|---|---|
| Struct, generator, flags | `core/src/overlay/frame.rs` |
| Install / uninstall / describe | `core/src/overlay/install.rs` |
| The panel | `apps/lua/ac_pro_engineer/ac_pro_engineer.lua` |
| Generated layout | `apps/lua/ac_pro_engineer/frame_layout.lua` |
| Manifest, five windows | `apps/lua/ac_pro_engineer/manifest.ini` |
| LÖVE harness | `apps/lua/love/` — see its README |
| LuaJIT harness | `apps/lua/tests/run_overlay.lua` |
| Engineer probe | `core/examples/engineer_probe.rs` |

Regenerate the layout after touching the struct, and rebuild the bridge if the
size changed:

```bash
cargo run -p ac_core --example gen_lua_layout > apps/lua/ac_pro_engineer/frame_layout.lua
```

```bash
cargo build --release -p shm-bridge --target x86_64-pc-windows-gnu
```

## Three pieces must agree

The application, `shm-bridge.exe` and the panel all encode the same frame. Every
failure that cost an evening was one of them being older than the others:

- a bridge built before the frame grew maps too few bytes and CSP refuses to
  open the mapping — the panel says "waiting for AC Pro Engineer" while
  `/dev/shm` has the file, at the right size, with the app running
- a panel from an older install reads every field after the change at the wrong
  offset — that is where `-1.7e27` in place of a tyre pressure came from
- the struct's field order and the generator's `FIELDS` list drifting apart does
  the same thing, and size and field-count checks do **not** catch it.
  `the_generator_lists_the_fields_in_the_struct_s_order` now does.

The launcher card reports the installed panel's frame version against the
application's. The bridge is not checked yet — see what is left.

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

The probe is how the engineer's "four tyres WORN OUT: 0.0%" was found: AC counts
wear down from 100, so all four corners at zero is a session that has not
published wear, not four destroyed tyres.

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

## What is left

1. **Version-check the bridge.** The application cannot tell that
   `shm-bridge.exe` is older than its frame. Have the bridge write its size
   somewhere the app can read, or ship it beside the binary and compare
   modification times.
2. **Package the bridge.** It has to be built by hand and placed next to the
   application; the README does not say so.
3. **Windows.** Clippy is clean for the Windows target and the installer uses
   `ac_paths`, but nothing has been *run* there. The bridge is a Linux concern
   only — on Windows the application writes the mapping directly.
4. **The rest of the suggestions**: tyre temperature window from the application
   (two fields, same shape as the pressure targets), sector times, and a small
   history plot in the panel.
5. The panel's strings are translated; the console's settings labels are only
   partly.

## The environment

| | |
|---|---|
| AC | `~/.steam/steam/steamapps/common/assettocorsa`, v1.16.4 x64 |
| CSP | 0.2.11 b3465 |
| Prefix | `steamapps/compatdata/244210/pfx`, Proton 9.0 / GE-Proton9-2 |
| Bridge | `~/projects/RaceEngineer/shm-bridge.exe`, run through protontricks |
| Panel installs to | `assettocorsa/apps/lua/ac_pro_engineer/` |

There is a second copy of the app in `extension/lua/ac_pro_engineer/` on this
machine; both are written during development. Delete the second before anyone
else sees this.

Getting AC, CSP and Content Manager running under Proton is in the README under
"Getting Assetto Corsa, CSP and Content Manager to run under Proton".
