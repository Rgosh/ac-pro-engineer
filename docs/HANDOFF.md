# Handoff — the in-game overlay

State as of 2026-08-05. Read this first in a new session.

`main` is pushed to `origin`, 224 tests, clippy and fmt clean on Linux **and**
`x86_64-pc-windows-gnu`. Check both before pushing:

```bash
cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings
```

## The shape of the thing

The desktop application computes everything and publishes a 440-byte
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

## Where this stands: v0.3.4 is a demo release

Cut and published deliberately as a preview, because the two things that
remained could only be done on someone else's machine: running it on Windows,
and running it inside a session. The changelog states both limits at the top of
the release, so nobody reads a rough edge as a promise.

## The one thing that blocked a beta, and how v0.3.4 answers it

**No published release contains a bridge that can serve the overlay.** v0.3.3
was tagged at 04:15 and `187b914`, the commit that added the overlay mapping to
`ACC_FILES`, landed at 04:26. The published `shm-bridge.exe` therefore maps AC's
four `acpmf_*` pages and nothing else: it starts, reports no error, and the
overlay mapping is never created, so on Linux the panel waits forever with the
application running and the file sitting in `/dev/shm`.

Confirmed by scanning the published artifact — it does not contain the string
`AcTools.CSP.Limited.ACPE.v1` anywhere, and the bridge built from this checkout
does.

Nothing in the code could fix this; a release had to be cut from a commit at or
after `187b914`, and v0.3.4 is it. Everything else was already in place:

- `bridge_update` finds the asset dist actually publishes
  (`shm-bridge-x86_64-pc-windows-gnu.zip`, not the bare `.exe` that only v0.2.2
  had), unpacks it, and **refuses** a bridge that does not carry the overlay
  mapping's name — so pressing [B] today downloads v0.3.3, inspects it, and says
  why it will not install it, rather than installing a downgrade into this bug.
- the launcher card and `bridge_probe` both name the state.

## What is left

1. **Windows.** Clippy is clean for the Windows target, the test suite passes
   cross-compiled, and the installer uses `ac_paths` — but nothing has been
   *run* there. The bridge is a Linux concern only; on Windows the application
   creates the named mapping itself and `bridge::status` reports `NotRequired`.
2. **The rest of the suggestions**: tyre temperature window from the application
   (two fields, same shape as the pressure targets), sector times, and a small
   history plot in the panel.
3. The panel's strings are translated; the console's settings labels are only
   partly, and `Wear:`, `T:` and `B:` in `formatFrame` are English in both
   languages.
4. `fetch_bridge_now` blocks the UI thread for the length of a download. One
   small file behind an explicit keystroke, so it has not been worth a thread —
   revisit if the asset grows.

## Done since the last handoff

1. ~~**Version-check the bridge.**~~ It writes `/dev/shm/acpe-bridge.info` and
   compiles its version into its own binary; `bridge::status` judges both, and
   `cargo run -p ac_core --example bridge_probe` prints the verdict. Verified
   end-to-end under Wine, including the incompatible and behind cases.
2. ~~**Package the bridge.**~~ dist already publishes it; [B] on the launcher
   card fetches and verifies it.

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
