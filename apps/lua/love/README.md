# Overlay harness

The overlay panel, running under [LÖVE](https://love2d.org) instead of Assetto
Corsa. Same script, same file — `../ac_pro_engineer/ac_pro_engineer.lua` is
loaded from its own directory, so what is on screen is what would be installed.

```bash
./run.sh                 # simulated lap
./run.sh --shm           # read what the desktop app is publishing
./run.sh --help          # every flag
```

`F5` reloads the app script, `space` pauses, `F2` opens the settings window,
`escape` quits. **Windows are dragged by their title bars and resized from the
grip in the bottom-right corner**, and both are remembered — same as in game,
where the driver arranges the windows once and CSP keeps the layout. Sizes are
also on sliders in the Harness tab, and `Reset layout` puts everything back.

## Why

Judging the panel used to mean launching AC: right Proton, right CSP, right
`WINEDLLOVERRIDES`, two minutes to the pits. Layout mistakes do not need any of
that — a column landing on top of another is visible the moment something draws
it. This is that something, and it found the timing and fuel rows overlapping on
its first run.

## Getting it into the game

The desktop application writes the app into
`assettocorsa/apps/lua/ac_pro_engineer/` every time it starts, and says what it
found in a card on the launcher screen: where the game is, whether CSP is
there, and whether the files are current. `ENTER` installs them again from
that card, `[I]` does the same from **Settings → OVERLAY**, and `D` stops the
card appearing at startup.

Getting AC, CSP and Content Manager running under Proton in the first place is
in the [main README](../../../README.md#getting-assetto-corsa-csp-and-content-manager-to-run-under-proton).

## What the panel actually is

A CSP Lua app in `../ac_pro_engineer/`, installed into the game folder by the
desktop application on every launch. It computes nothing. Once a frame the
desktop side packs a 712-byte `#[repr(C)]` `OverlayFrame` into shared memory,
and the app reads fields out of it and hands them to ImGui — Lua runs on AC's
render thread, where LuaJIT collects garbage mid-frame, so the panel allocates
nothing per frame that can be allocated once.

The frame carries speed, revs and gear; four corners of pressure, temperature,
wear and brake heat; delta and lap times; fuel, laps left and consumption;
position, lap, air and road temperature, grip; up to eight lines of engineer
advice **with a severity each**; and a bit field of what the application wants
shown. A `sequence` counter that only ever moves by two guards against torn
reads and doubles as the liveness signal: frozen for two seconds means the
application is gone, and the panel says so instead of holding numbers.

## What is emulated

`csp.lua` puts CSP's globals in place on top of LÖVE:

| | |
|---|---|
| `ui.*` | text, fonts, cursor and group layout, `sameLine`, rectangles, and the interactive widgets — checkbox, button, slider, radio, tabs |
| `ac.readMemoryMappedFile` | hands back the harness's frame table instead of shared memory |
| `ac.storage` | the app's settings, persisted to LÖVE's save directory |
| `ui.begin`/`ui['end']`, style stacks | `WindowRounding`, `WindowPadding`, `WindowBg`, `StyleColor.Text`, window flags |
| window chrome | the rounded frame, the app icon, the name and the settings gear CSP puts around every app |
| `vec2`, `rgbm`, `bit`, `script` | as CSP defines them |

Layout follows ImGui's rules closely enough that `sameLine`/`beginGroup` code
lands where it does in game: an item advances the cursor down a line, `sameLine`
pulls the next one back up beside it, and a group measures as a single item.

Anything not emulated resolves to a no-op that counts itself and appears in the
**Log** tab — a CSP function the app starts using shows up as a line there
rather than as a crash mid-frame. That also means the harness runs app versions
it has never seen, including ones built around `ui.begin`/`ac.onRenderWidget`.

## The windows

Five, all of them declared in the app's manifest:

| Window | `FUNCTION_MAIN` | |
|---|---|---|
| AC Pro Engineer | `windowMain` | speed, revs, tyres and brakes, timing, fuel, session |
| AC Pro Engineer — advice | `windowEngineer` | the engineer's lines, on their own |
| AC Pro Engineer — telemetry | `windowTelemetry` | every field in the frame, as it arrived |
| AC Pro Engineer — status | `windowStatus` | is the mapping open, is anything arriving, do the versions agree |
| settings | `windowSettings` | every section on or off, engineer output, text size, VR mode, units |

Each is a separate entry in CSP's sidebar in game, moved and sized separately.
The harness draws all five through the same chrome, so what is arranged here is
what can be arranged there. Open the ones that start closed from the **Harness**
tab, or with `--status`, `--settings` and the Telemetry checkbox.

## Portraits, for the README

```bash
./portraits.sh
```

One picture per window and one per settings tab, cropped to the window and
written into `screenshots/`. Each is a separate run of the harness with
`--portrait ID`, which draws that window alone in a LÖVE window sized exactly to
it, so nothing needs cropping afterwards.

Portraits get storage that remembers nothing, so a run always starts from the
panel's defaults — otherwise the run that photographs the Dev tab leaves
developer mode on in every picture after it.

| | |
|---|---|
| `--portrait ID` | `main`, `engineer`, `telemetry`, `status` or `settings` |
| `--app-tab PATH` | a tab inside the app, `Look/Colour` for a nested one |
| `--app-dev` | turn the panel's own developer mode on, for the Dev tab |
| `--app-stopped` | freeze the feed, so the panel draws what a closed application looks like |
| `--size WxH` | the size to draw the portrait's window at |
| `--shot NAME` | where it lands in the save directory |

## The tabs

- **Telemetry** — a read-only summary of what the panel is being fed. With
  developer mode on it becomes the controls: where the frame comes from (a
  self-driving lap, the real shared-memory frame, or the sliders), every field
  as a slider, every `flags` bit as a checkbox, and the advice lines with their
  severities.
- **App settings** — the overlay's *own* settings window, drawn by the overlay's
  own code, docked into the panel. The gear in the app's title bar opens the
  same window where CSP opens it, floating beside the panel (`--settings`, or
  `F2`).
- **Harness** — font scale, panel size, backdrop (dark, checkerboard for
  translucency, or green), content outline. Saved and reused next run.
- **Dev** — a console that takes the same flags as `run.sh`, applied on the
  spot: `--source shm`, `--scale 1.4`, `--size 320x520`, `--vr`, `--help`.
  Nothing restarts. Developer mode lives here too, and the simulation controls
  stay behind it — hand-fed telemetry can make the panel show things no real
  session would, which is a good way to trust a layout that does not work.
- **Log** — what loaded, what threw, and which CSP calls the emulation ignored.

Every panel scrolls with the wheel when its contents outgrow it, the same way
CSP scrolls a window in game.

## What the application controls, live

Seven of the panel's decisions are made on the desktop side and published as
flags on every frame, so changing one in the TUI's **Settings → OVERLAY `[F]`**
reaches the panel on the next tick — no restart, no reload:

telemetry, engineer advice, session, lap timing and fuel blocks, the fuel
warning threshold, and how many engineer lines are published at all (0–4).

The panel has its own switch for each block, and both have to agree: the flag
means "there is nothing worth showing", the app's setting means "the driver
does not want to see it".

## Advice, coloured the way the application colours it

Severity travels with each line — 0 info, 1 warning, 2 critical — because the
text alone cannot carry it, and the same sentence should not mean one thing on
the desktop and another in the car. The panel marks them `i`, `!` and `!!` in
green, yellow and red, matching the terminal's own icons, and leaves the
sentence in the reading colour. Marker style, line count, wrapping and whether
advice is highlighted at all are settings.

## Nothing without the application

Every window shows one thing while the desktop app is not publishing: *AC Pro
Engineer is not running*, why it thinks so, and how long ago the last frame
arrived. No stale numbers, no half-drawn panel — a feed that died two minutes
ago looks exactly like a live one if the numbers stay on screen.

The Telemetry tab's **Stop desktop app** button freezes the sequence counter,
which is precisely what a closed application looks like from the panel's side.
That is the state worth checking before a race weekend, and it is one click.

## Telemetry sources

`sim` drives a lap on its own: speed and revs follow a corner-straight-corner
rhythm, tyres heat under load, wear only goes one way, fuel drains into the
warning flag, and the pit limiter comes on below 80 km/h. Every colour threshold
in the panel gets crossed within a minute.

`shm` reads the real `OverlayFrame` from `/dev/shm/AcTools.CSP.Limited.ACPE.v1`
— the same 400 bytes the Lua app reads in game, parsed through the same struct
declaration. Point it elsewhere with `--frame PATH`. When nothing is published
the Telemetry tab says so, and the panel falls back to its idle state on its
own, which is the behaviour worth checking.

`manual` leaves the frame alone; the sliders are the only thing writing to it.
Moving any slider switches to it automatically.

## Settings

Three places, resolving in this order for a given run: the saved file loads
first, a command-line flag overrides it, and anything changed in the window is
written back. Harness settings live in `harness.lua` and the app's own settings
in `app-settings.lua`, both under `~/.local/share/love/acpe-harness/`.

## Checking it in CI

`--test` runs 120 frames without anyone watching and exits non-zero if the app
threw:

```bash
love apps/lua/love --test --tab Log
```
