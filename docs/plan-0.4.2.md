# v0.4.2 — what the core owes the window

Written 2026-08-24, after reading the core against what RG Pro Engineer needs
from it. **Everything here is a change in this repository**; the window's own
list is `docs/PLAN-NEXT.md` in `Rgosh/RGProEngineer` and does not overlap.

## What kind of release this is

A patch. The rules that follow from that, and each one is a thing that would
otherwise be discovered late:

* **The frame does not change.** `OVERLAY_VERSION` stays 6 and the struct stays
  2484 bytes, so every `shm-bridge.exe` and every installed panel from v0.4.0
  onward keeps working and nobody has to fetch a bridge.
* **Nothing already public changes shape.** New fields on serialised types are
  added with `#[serde(default)]`, so a lap saved by v0.4.0 still loads.
* **One number for the family.** The core, the terminal and the window are
  released together — `Cargo.toml` here, the panel's `PANEL_VERSION`, the
  manifest's `VERSION`, and the window's own `Cargo.toml` in the other
  repository.

Baseline before any of this: 521 tests pass on Linux, clippy is clean on both
targets, and `cargo deny check advisories` **fails** — see A2.

## A. The fixes, which are the reason to cut a release

### A1. Saved laps land wherever the program happened to be started

`tui/src/ui/tabs/analysis/mod.rs:230` writes to `saved_laps/`, a path relative
to the working directory, with `fs::write`, under a name made of the car, the
track and the lap time. `RGProEngineer/src/saved.rs:24` reads from the same
relative path. Four consequences, and the first is the one people report:

* The window is started from a desktop entry, whose working directory is the
  home folder; the terminal is usually started from its own install folder.
  **They do not see each other's laps**, and neither of them is wrong.
* `fs::write` is not atomic. This core has `atomic_file::write_atomic` and
  `config`, `records` and the telemetry export all use it; a lap is the one
  thing written straight, and a crash mid-write leaves a truncated file that
  fails to parse on the day somebody wants it.
* Two identical lap times overwrite each other silently.
* The logic exists twice, in two front ends, which is precisely what the core
  is for.

**The work.** A `core::laps` module: the folder beside `config::app_dir()`,
where `records.json` already lives; `save`, `list`, `load`, `delete`; atomic
writes; a name that cannot collide. Plus a one-time migration — if a
`saved_laps/` folder exists beside the executable or in the working directory,
move what is in it and say so once. Both front ends then call the core instead
of each holding half of this.

**Verified by** a test that saves two laps with the same car, track and time
and gets two files back; a test that a lap written by v0.4.0's naming still
loads; and by starting the terminal from `/tmp` and finding the lap the window
saved.

### A2. Eight advisories, three of them in the code path that installs software

`cargo deny check advisories` fails today:

| Advisory | Crate | Fix |
|---|---|---|
| RUSTSEC-2026-0186 | `memmap2` 0.9.10 | 0.9.11 — a direct dependency, and the crate that maps the game's pages |
| RUSTSEC-2026-0190 | `anyhow` 1.0.100 | 1.0.103 |
| RUSTSEC-2026-0007 | `bytes` 1.11.0 | 1.11.1 |
| RUSTSEC-2026-0258 | `h2` 0.3.27 | needs `reqwest` 0.12 |
| RUSTSEC-2026-0098, -0099, -0104 | `rustls-webpki` 0.101.7 | needs `reqwest` 0.12 |
| RUSTSEC-2025-0134 | `rustls-pemfile` | disappears with `reqwest` 0.12 |
| RUSTSEC-2024-0436 | `paste` | unmaintained only; comes from `ratatui` 0.26 and `image` 0.24 — **not** in this release, see C |

The first three are `cargo update -p <crate>` and nothing else. The rest are one
change: **`reqwest` 0.11 → 0.12**, which brings `hyper` 1, `h2` 0.4 and
`rustls-webpki` 0.103. The feature list here is `default-features = false` plus
`blocking`, `json`, `rustls-tls`, all of which exist in 0.12; the migration is
small in surface and worth doing carefully because two of those webpki
advisories are **certificate name-constraint** bugs, and the code holding this
dependency downloads a binary and puts it on the user's disk.

**Verified by** `cargo deny check advisories` coming back clean, the updater's
own tests, and one real fetch: `cargo run -p ac_core --example bridge_probe`
and a manual update check against GitHub.

### A3. The documents a new session reads first are wrong about the frame

`CLAUDE.md` and `docs/HANDOFF.md` both describe a **712-byte** frame at version
**5**. The struct is version 6 and 2484 bytes — `core/src/overlay/frame.rs:24`
and `shm-bridge/src/main.rs:58`. That is the exact class of mistake that costs
an evening, and it is in the file whose whole job is to prevent those.

`docs/plan-acc.md` §10 also still lists two things as owed that are done: the
compound bands (`compound_band` matches ACC's `dry_compound` and `wet_compound`
and has tests naming them) and `surface_grip`. What is genuinely still open
there is one thing only — **a stint driven on ACC with every line of advice
read against the numbers that produced it**, which needs the wheel and not the
keyboard.

## B. What to add, so the window has something to draw

Each of these is a computation, which means it belongs here and not in a front
end — the panel and the terminal get them in the same breath.

### B1. The delta, as a trace

`corners::time_at(trace, distance)` already interpolates a lap time at a
distance, and `decompose` uses it internally to charge each section. What is
missing is the obvious public thing between them: **time gained and lost against
a reference, sampled along the lap**, as `Vec<(f32, i32)>` or an iterator.

It is four lines of arithmetic and the single most-asked-for view in this kind
of program: the racing line coloured by where the time went rather than by
speed. The window draws it on the map; the terminal can put it under the trace
it already plots.

**Verified by** a lap compared against itself summing to zero everywhere, and a
lap against a reference whose endpoint equals `Decomposition::total_ms` — the
two answers have to agree or one of them is wrong.

### B2. Braking, measured

`Corner` carries `brake_point` and `CornerComparison` can say how many metres
earlier it was than the reference. Nothing measures what happens *between* that
point and the apex, which is where the time in a braking zone actually goes:

* braking distance and duration,
* peak deceleration, in g,
* how quickly the pedal comes off — the trail-braking shape,
* and the same three on the reference, so the comparison is a subtraction.

Additive fields on `Corner` with `#[serde(default)]`. The detector already
walks the trace for the braking point, so this is the same pass.

**Verified by** a synthesised trace with a known deceleration; and by
`engineer_probe`, which prints advice beside the numbers that produced it.

### B3. ACC's track length, measured rather than published

ACC publishes `trackSPlineLength` as zero — pinned by the layout tests against
a real recording, so this is the game's behaviour and not a parsing mistake.
The consequence today is that everything reported in metres is withheld on the
game most GT3 drivers are on: no "braking 14 m earlier", no corner distances.

But `Reading::distance_travelled_m` is read from **both** games and used by
nothing at all. Over one completed lap the difference between two crossings of
the line is the circuit's length, measured by the car that just drove it.

**The rule this must not break.** It is a *measurement*, not a published value,
and it must be marked as one: available only after a full lap, never carried
back into `Capabilities::track_length` as though the game had reported it, and
absent — not guessed — until it exists. The Spa recording is the check: 7004 m.

### B4. A session, not a stint — the storage half

The window's list opens with "nothing survives a session ending". The half that
belongs here is the storage: laps grouped under a session id, a small summary
per session, and the same rules as A1 — atomic, beside the settings, and
**nothing from demo mode is ever written**. The drawing is the window's.

This is the largest item on the page, and it is the one to drop first if 0.4.2
is to be cut this week: A1 gives it its foundation, and it lands as cleanly in
0.4.3.

## C. Deliberately not in this release

* **`ratatui` 0.26 → 0.29 and `image` 0.24 → 0.25**, which is what would clear
  the `paste` advisory. Unmaintained is not a vulnerability, and a terminal
  redraw is not a patch release.
* **Merging the two updaters.** The window updates from a manifest on
  proengineer.app and the terminal from GitHub releases; `RGProEngineer/src/update.rs`
  explains why, and the explanation still holds. A shared channel is a design
  change, not a fix.
* **Anything touching the frame**, for the reason at the top.
* The window's own list — beginner mode, the map colouring, the braking screen.
  B1–B3 are what those are built from; drawing them is the other repository.

## The order, and what to run

1. A2's three trivial bumps, then `reqwest` 0.12 on its own commit.
2. A1, with both front ends moved onto it.
3. B1, B2, B3 — each with its tests, each usable by the window the day it lands.
4. A3 last, so the documents describe what was actually shipped.
5. B4 if the week allows, otherwise 0.4.3.

After every item:

```bash
./tools/test-game.sh core
```

```bash
cargo test --workspace
```

```bash
cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings
```

Before the tag:

```bash
./tools/test-windows.sh
```

```bash
cargo deny check advisories
```

```bash
luajit apps/lua/tests/run_overlay.lua
```

The panel is untouched by all of this, and the harness run is the proof of that
rather than a formality — the frame is what the window and the panel share.
