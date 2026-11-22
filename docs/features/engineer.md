[← What the core does](README.md) · [Main README](../../README.md)

# The race engineer

`engineer.rs`, `confidence.rs`, `steady.rs`, `advice_order.rs`, `followup.rs`,
`coach.rs`, `debrief.rs`, `driver_vs_car.rs`.

**Live advice** from the game's telemetry: tyre temperatures inner/middle/outer
(camber), pressures against the compound's window, brake heat per axle,
balance (understeer and oversteer, slow and fast corners), lock-ups and
wheelspin, fuel. Each finding is a `Recommendation` carrying:

- **the message and the action** — "High-speed understeer · more front wing";
- **a chain** — the mechanism, the measurement that showed it, and what to look
  at next run to know whether the change worked;
- **parameters** — the setting to change and the number to change it to, when
  the car's setup says what the range is;
- **confidence** — low, medium or high, from how much of the lap showed it.

**Thresholds by car class.** A GT3's brakes and tyres are judged against a
GT3's windows (`games::car_class`), not one band for every car: 600 °C front
brakes on carbon are normal, not a warning.

**Settled, not flickering.** A finding has to hold before it is drawn and has
to clear before it goes (`steady.rs`), so a tyre resting on its limit does not
blink in and out.

**In order.** When there is more than one thing to change, the first change to
make is marked first — balance before the brake bias that depends on it
(`advice_order.rs`).

**Following up.** The engineer remembers what it said last run on this car and
circuit and says whether it is fixed, still there or worse (`followup.rs`).
The coach picks the one thing worth working on and checks it next session
(`coach.rs`).

**Driver or car.** `driver_vs_car.rs` tells "the car is doing this" from "you
are doing this" — a setup change does not fix a late brake point.

**After the lap.** `debrief.rs` writes what is over, lap by lap, for the panel
and the windows.

**Never a zero for something not measured.** Every game says what it can
report (`Capabilities`); advice that needs a number the game does not publish
stays silent instead of reading zero as a verdict.
