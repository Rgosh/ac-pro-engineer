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
`escape` quits. **Windows are dragged by their title bars** and remember where
they were put — same as in game, where the driver arranges them once and CSP
keeps the layout.

## Why

Judging the panel used to mean launching AC: right Proton, right CSP, right
`WINEDLLOVERRIDES`, two minutes to the pits. Layout mistakes do not need any of
that — a column landing on top of another is visible the moment something draws
it. This is that something, and it found the timing and fuel rows overlapping on
its first run.

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

The app declares two in its manifest, and CSP opens a third from the gear:

| Window | `FUNCTION_MAIN` | |
|---|---|---|
| AC Pro Engineer | `windowMain` | speed, revs, tyres and brakes, timing, fuel, session |
| AC Pro Engineer — advice | `windowEngineer` | the engineer's lines, on their own |
| settings | `FUNCTION_SETTINGS` | every section on or off, text size, VR mode, units |

Each is a separate entry in CSP's sidebar in game, moved and sized separately.
The harness draws all three through the same chrome, so what is arranged here
is what can be arranged there.

## The tabs

- **Telemetry** — where the frame comes from (a self-driving lap, the real
  shared-memory frame, or the sliders), every field as a slider, the five
  `flags` bits as checkboxes, and the engineer messages.
- **App settings** — the overlay's *own* settings window, drawn by the overlay's
  own code, docked into the panel. The gear in the app's title bar opens the
  same window where CSP opens it, floating beside the panel (`--settings`, or
  `F2`).
- **Harness** — font scale, panel size, backdrop (dark, checkerboard for
  translucency, or green), content outline. Saved and reused next run.
- **Log** — what loaded, what threw, and which CSP calls the emulation ignored.

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
