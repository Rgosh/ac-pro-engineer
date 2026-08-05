# CLAUDE.md — working on RaceEngineer

`AGENTS.md` describes the codebase; this describes how to work in it without
repeating the mistakes already made. Read `docs/HANDOFF.md` for where the
overlay work stands right now.

## What this project is

A telemetry and race-engineering suite for Assetto Corsa, in Rust, with two
faces: a terminal application (`ac_tui`, the binary `ac_pro_engineer`) and an
in-game panel written in Lua for CSP (`apps/lua/ac_pro_engineer/`).

The split matters. **The application computes; the panel draws.** Lua runs on
AC's render thread, where a millisecond is a sixth of the frame budget at 165 Hz
and LuaJIT collects garbage mid-frame. Anything that can be computed on the
desktop side must be, and the panel formats text when a frame *arrives*, never
in the draw path.

## The frame contract

`core/src/overlay/frame.rs` owns a 424-byte `#[repr(C)]` `OverlayFrame` and the
generator that emits its Lua declaration. Three artefacts encode it:

1. the application, which writes it,
2. `shm-bridge.exe`, which maps it into the Wine prefix on Linux,
3. `apps/lua/ac_pro_engineer/frame_layout.lua`, which the panel reads it with.

**Changing the struct means changing all three.** After any edit to the fields:

```bash
cargo run -p ac_core --example gen_lua_layout > apps/lua/ac_pro_engineer/frame_layout.lua
```

```bash
cargo build --release -p shm-bridge --target x86_64-pc-windows-gnu
```

Bump `OVERLAY_VERSION` and `EXPECTED_VERSION` in the panel together. Field
**order** must match between the struct and `FIELDS` — size and count matching
is not enough, and a mismatch reads eight bytes of one field as another
(`the_generator_lists_the_fields_in_the_struct_s_order` catches it now).

Adding a bit to `flags` costs nothing: no layout change, no version bump.
Prefer that to a new field when the answer is yes-or-no.

## Versions, and which one answers which question

Four numbers, and confusing them wastes an evening:

| Number | Where | Changes when |
|---|---|---|
| `OVERLAY_VERSION` / `EXPECTED_VERSION` | `frame.rs`, the panel | a field moves |
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
for AC Pro Engineer" with the mapping right there in `/dev/shm`.

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
AC Pro Engineer" branch and reported OK for a panel that drew nothing —
27 strings instead of 140. `ACPE_ALL=1` prints every one of them, which is how a
wrong unit or an untranslated caption is caught without launching the game.

```bash
love apps/lua/love --test --settings
```

Runs the panel under LÖVE for 120 frames and exits non-zero if it threw. Add
`--shot name.png` to get a picture; the harness's own README explains the rest.

```bash
cargo run --bin simulator
```

```bash
cargo run -p ac_core --example engineer_probe
```

Fake AC telemetry into `/dev/shm`, then the engineer's advice printed next to
the numbers that produced it. This is how false "four tyres WORN OUT" was found.

And the whole suite, on both targets, before pushing:

```bash
cargo test --workspace
```

```bash
cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings
```

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
