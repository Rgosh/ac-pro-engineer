# Where this is going: a modular core, several games, several faces

A plan, not a description. Nothing below is built yet except where it says so.

The ask, in one sentence: **the core should read whatever game is running,
compute everything, and broadcast it; anything that draws should be a separate
thing that subscribes.** Today the core is shaped around Assetto Corsa and has
exactly two consumers, both wired in by hand.

## What is actually game-specific today

Worth being precise, because it is less than it looks. Of `core/`:

| Module | Game-specific? |
|---|---|
| `ac_structs.rs` | **Yes** — AC's four shared-memory pages, field for field |
| `ac_paths.rs` | **Yes** — where Steam puts AC, where its setups live |
| `setup_manager.rs` | **Yes** — AC's setup `.ini`, and its click indices |
| `content_manager.rs` | **Yes** — reads AC's car data |
| `analyzer.rs` | No — laps, sectors, averages, histograms |
| `engineer.rs` | No, once it stops taking `AcPhysics` |
| `debrief.rs` | No, same |
| `records.rs`, `ring_buffer.rs`, `atomic_file.rs` | No |
| `overlay/` | **Transport**, not a game — see below |

So the split is not "rewrite the core". It is: put a model in the middle, move
four modules behind a trait, and let the rest talk to the model.

## The shape

```
                 ┌──────────────┐
   Assetto Corsa │ sources/ac   │─┐
   (shared mem)  └──────────────┘ │
                 ┌──────────────┐ │   ┌─────────┐   ┌───────────┐
   another game  │ sources/…    │─┼──►│  model  │──►│ engineer  │
                 └──────────────┘ │   └─────────┘   └───────────┘
                                  │        │              │
                                  │        └──────┬───────┘
                                  │               ▼
                                  │        ┌─────────────┐
                                  └───────►│  broadcast  │
                                           └──────┬──────┘
                        ┌─────────────┬───────────┼────────────┐
                        ▼             ▼           ▼            ▼
                   shared mem     WebSocket    terminal    (your next
                   → CSP panel    → anything    (in-proc)    front end)
```

### `telemetry` — the model

One neutral `Sample`: speed, gear, rpm, four corners of pressure/temperature/
wear/brake, ride height, fuel, lap and sector timing, session state. Plus a
`Capabilities` set, because this is the part that always goes wrong: a game that
cannot report inner/outer tyre temperature must be *distinguishable* from a game
reporting zero. The camber advice is built on that difference, and a model that
cannot express "not measured" turns every missing field into a wrong verdict.

### `sources/` — one module per game

```rust
pub trait Source {
    fn id(&self) -> &'static str;              // "assetto_corsa"
    fn capabilities(&self) -> Capabilities;
    fn detect() -> Option<Self> where Self: Sized;   // is it installed / running
    fn poll(&mut self) -> Option<Sample>;
    fn setups(&self) -> Option<&dyn SetupStore> { None }
}
```

`sources/assetto_corsa/` gets today's `ac_structs`, `ac_paths`, `setup_manager`,
`content_manager` and the `Memory` reader currently living in `tui/src/lib.rs`.
Nothing else in the tree should mention `AcPhysics` again.

### `engineer` — advice from the model only

Takes `Sample` and `Lap`, not `AcPhysics`. This is mostly a signature change:
`debrief.rs` already works off `LapData` and would move over almost unchanged.

### `broadcast` — transports, plural

The frame in `overlay/frame.rs` becomes one transport among several, and it
**stays** for the in-game panel. That is not legacy: CSP Lua runs on AC's render
thread, and a 2 KB memcpy from a mapping is the cheapest thing that can possibly
happen there. A WebSocket in the panel would put a socket read in the draw path,
which is the one thing the whole design exists to avoid.

WebSocket is for everything that is *not* inside the game: a browser overlay, a
stream widget, OBS, a phone on the desk, someone else's tool. JSON, one message
per tick, the same data the frame carries.

```json
{ "t": "sample", "seq": 12043, "speed_kmh": 214.0, "gear": 5,
  "tyres": { "pressure_psi": [26.8, 27.0, 26.4, 26.6], "temp_c": [...] },
  "capabilities": ["tyre_edges", "sectors"] }
{ "t": "advice", "lines": [ { "severity": 1, "text": "…", "action": "…" } ] }
{ "t": "lap", "number": 12, "time_ms": 91234, "sectors": [...], "debrief": [...] }
```

Versioned like the frame is, for the same reason: the moment somebody writes a
widget against it, the schema is a contract.

## Assets

`assets/frontends/csp-panel/` is not "the overlay", it is *the Assetto Corsa
front end*. It moves to `assets/frontends/csp-panel/`, and `assets/` is where
per-game and per-frontend files live from then on. `install.rs` keeps embedding
it; only the `include_bytes!` paths change.

## Stages

Each one leaves the tree working, tested and releasable. That matters more than
usual here: the frame version has already moved three times this cycle, and
every move costs every Linux driver a bridge update.

1. **Assets move.** Mechanical, no behaviour change. Touches `install.rs`,
   `portraits.sh`, both harnesses and their tests.
2. **The model and the AC source.** `telemetry::Sample`, the `Source` trait, AC
   behind it. The engineer keeps its current signatures via a thin adapter, so
   nothing else changes yet.
3. **The engineer moves onto the model.** Delete the adapter. This is where the
   compound-aware thresholds below belong, because the model is where a compound
   becomes a first-class thing rather than a string match.
4. **`broadcast`, with the frame as its first transport.** No new behaviour, but
   the panel now reads from a transport rather than from a hand-wired publisher.
5. **WebSocket transport + a schema document.** Additive; nothing existing
   changes. This is the point at which somebody else can write a front end.
6. **A second game.** Only now, because until a second source exists every
   abstraction above is a guess. The first one will find three wrong assumptions
   in the model, and that is cheaper to fix with one consumer than five.

## The advice itself

Separate from the architecture, and worth doing regardless. The current
verdicts are thin in one specific way: **thresholds are global constants where
the physics is car-specific.**

- `analyze_tyre_pressure` reads the compound and picks a band —
  street/sport/eco/semislick, wet/rain, or racing.
- `analyze_tyre_temperature` and the wear checks do **not**. One
  `tyre_temp_min`/`tyre_temp_max` pair from the config is applied to a GT3 slick
  and to an Abarth on street tyres, whose working windows are nowhere near each
  other. "All four cold 62 °C" is right for one and nonsense for the other.

That is the same shape of mistake as the camber bug: a fixed number standing in
for something the car decides. Same for brake temperature, where a carbon disc
and an iron one differ by hundreds of degrees.

The fix is a per-compound table in the model rather than four numbers in a
config, which is why it belongs in stage 3 and not before.
