[← What the core does](README.md) · [Main README](../../README.md)

# Strategy

`stint.rs`, `engineer.rs` (fuel and wear), `setup_manager.rs`.

- **The stint** (`stint.rs`): laps done, laps left on the fuel in the tank and
  on the tyres, the pace trend, and when to stop.
- **Fuel**: per lap measured from the laps driven, not from the car's file;
  what to add for a race of so many laps or minutes, with a margin.
- **Tyre and brake life**: wear per lap and laps remaining, per corner — and on
  Competizione, where tyre wear is not published, brake pad life in its place.
- **Cold pressures**: what to set in the pits so the hot pressure lands in the
  compound's window, adjusted for the air and track temperature and, where the
  game publishes it, the track's grip.
- **Setups**: read, compared and written in the game's own format
  (`setup_manager.rs`, `games::assetto_corsa::setups`).
