[← What the core does](README.md) · [Main README](../../README.md)

# Lap analysis

`analyzer.rs`, `laps.rs`, `corners.rs`, `braking.rs`, `line.rs`, `grip.rs`,
`shifts.rs`, `slips.rs`, `consistency.rs`, `repeatability.rs`, `track.rs`,
`session_report.rs`, `records.rs`, `memory.rs`.

- **Every lap kept** with its whole trace — speed, pedals, gear, revs,
  position, g-forces, tyres — and compared **by distance, never by time**, so
  two laps through one corner line up on the track.
- **Corners found in the trace** (`corners.rs`): entry, apex, exit, minimum
  speed, and where each lap gained or lost against the reference.
- **The ideal lap**: your best of each corner joined into one lap, and how much
  is on the table.
- **Braking** (`braking.rs`): where the pedal went down, how hard, how long,
  trail braking, against the reference corner by corner.
- **The line, in metres** (`line.rs`): how far across the road you were from
  the reference at each corner's turn-in, apex and exit, wide or tight, and the
  corners where a different line cost time — "T5: 1.8 m wide of your reference
  at the apex · +0.21 s".
- **Grip used** (`grip.rs`): the friction circle scored, corner by corner —
  how much of the tyre a driver actually used.
- **Shifts** (`shifts.rs`): every gear change and what it cost; early and late
  upshifts against the power curve.
- **Lock-ups and wheelspin, located** (`slips.rs`): where on the lap, which
  wheel.
- **Consistency** (`consistency.rs`, `repeatability.rs`): how many laps in a
  row were close to your best, and which corners vary most.
- **Records** (`records.rs`): best laps by car and circuit, kept across
  sessions, shared with the launcher.
- **Ghost laps and references**: a lap of your own, a friend's from the lap
  library, or one loaded from a file.
- **MoTeC export**: the trace as CSV for i2.
