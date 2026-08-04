# Handoff — in-game Lua overlay

State as of 2026-08-04. Read this first in a new session.

## Where the code is

`main` = `9032b30`, 222 tests, clippy and fmt clean on Linux **and**
`x86_64-pc-windows-gnu`. Nothing is pending; no branches other than `main`.

Check both targets before pushing — Windows-only breakage was introduced three
times, always the same way (a Linux-only item declared unconditionally):

```bash
cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings
```

## What the overlay does

Rust computes everything and publishes a 400-byte `#[repr(C)]` `OverlayFrame`
once per tick; a CSP Lua app reads fields and calls ImGui. Lua runs on AC's
render thread and LuaJIT collects garbage mid-frame, so the Lua side allocates
nothing per frame.

| Piece | Where |
|---|---|
| Struct + Lua generator | `core/src/overlay/frame.rs` |
| Shared-memory writer | `core/src/overlay/shared_writer.rs` |
| Self-installer | `core/src/overlay/install.rs` |
| The Lua app | `apps/lua/ac_pro_engineer/` |
| Runtime harness | `apps/lua/tests/run_overlay.lua` |
| Design rationale | `docs/overlay-lua-plan.md` |

The Lua files are embedded with `include_bytes!` and written into
`assettocorsa/apps/lua/ac_pro_engineer/` at startup, so the struct layout and
its declaration ship as one artifact and cannot drift.

Regenerate the layout after changing the struct:

```bash
cargo run -p ac_core --example gen_lua_layout > apps/lua/ac_pro_engineer/frame_layout.lua
```

Visibility follows the app: the `sequence` counter that guards torn reads
doubles as the liveness signal. Clean exit zeroes it; a kill freezes it and the
Lua side times out after 2 s.

### Four conformance tests, each of which caught a real bug

They skip when CSP or luajit is absent, so CI is unaffected.

- generated `ac.StructItem.*` exist in the installed CSP SDK — caught
  `explicitOrder`, which the published SDK calls it but **shipping CSP calls
  `explicit`**
- every `ui.*` the app calls exists — a missing one is a nil call mid-draw
- the manifest matches CSP's own key set and its `ICON` file exists — caught a
  missing `icon.png` and `SIZE_MIN` (the real key is `MIN_SIZE`)
- the app actually **runs** under LuaJIT with the CSP API stubbed

## Verified working

- shm-bridge under Proton creates the overlay mapping in `/dev/shm`
- the running app publishes frames; LuaJIT reads correct values for every field
- the app installs its own Lua files into the real game folder
- **CSP patches AC**: `All 2373 functions were found successfully`

## The environment, and how it was made to work

| | |
|---|---|
| AC | `~/.steam/steam/steamapps/common/assettocorsa`, v1.16.4 x64 |
| CSP | 0.2.11 b3465, install verified complete (695/695 files) |
| Prefix | `steamapps/compatdata/244210/pfx`, now Proton **9.0-203** |
| AC Documents | `pfx/drive_c/users/steamuser/Documents/Assetto Corsa` |
| Overlay app | `assettocorsa/apps/lua/ac_pro_engineer/` |

Two settings are **both** required, in AC's Steam properties:

- Compatibility → **Proton 9.0** (build 9.0-4f)
- Launch options → `WINEDLLOVERRIDES="dwrite=n,b" %command%`

Why each:

- Without the override, CSP never loads at all — it hooks by replacing
  `dwrite.dll`, and Wine uses its builtin unless told otherwise. Symptom: the
  CSP log is not written while AC's own log is, and no Lua apps exist.
- Without Proton 9.0, CSP loads but cannot patch: every symbol lookup fails on
  Proton 11 / cachyos. Symptom: "Failed to patch Assetto Corsa: Can't find
  CarLabel::render" and 74 more.

Steam must be **restarted** after installing a Proton version before it appears
in the compatibility dropdown.

Testing from a shell needs protontricks with an explicit version — running
Proton directly fails because AC needs Steam's context:

```bash
cd ~/.local/share/Steam/steamapps/common/assettocorsa/
PROTON_VERSION="Proton 9.0" WINEDLLOVERRIDES="dwrite=n,b" \
  protontricks-launch --appid 244210 acs.exe
```

## Open item: Content Manager is broken

Downgrading to Proton 9.0 rebuilt the prefix — Proton logged
`Upgrading prefix from 11.0-100 to 9.0-203` then `Removing newer prefix`. This
happens on any downgrade, including through Steam's UI.

`Documents/Assetto Corsa` survived (cfg, setups, logs, and the CSP override).
What did not survive is anything installed **into the prefix**, notably the
.NET Framework that Content Manager needs. CM now fails with ".NET 4.5.2 is not
installed" and a WPF `TypeInitializationException`.

Fix:

```bash
protontricks 244210 dotnet48
```

`dotnet472` also works. Expect several minutes and a few installer windows.
Re-run CM afterwards.

## Not yet seen

Whether the overlay panel actually appears in AC's app sidebar and how it
looks. CSP now patches the game, so it should — everything up to that point is
verified. The app is already installed and reinstalls itself on every launch of
the desktop application.

The desktop app must be running for the panel to show data; otherwise it reads
"AC Pro Engineer is not running".

## Local change made outside the repo

`Documents/Assetto Corsa/cfg/extension/general.ini` gained `CACHE_ACS=0` under
`[OPTIMIZATIONS]`. Backup at `general.ini.bak-acpe`. It removed one error line
but was not the root cause; safe to revert now that Proton 9.0 works.
