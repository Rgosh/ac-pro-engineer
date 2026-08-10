# v0.3.7 — turning Analysis into an engineer

The plan, and where it stands. **Items 1–7 are built**; the notes under each say
what was actually done and where it fell short of the sketch. Item 8 is not, and
the reason is in its own section.

| | | |
|---|---|---|
| 1 | Corner-by-corner analysis | built — `core/src/corners.rs` |
| 2 | Cause → effect | built — `Chain` on `Recommendation`, one producer so far |
| 3 | Setup ↔ telemetry | built — `core/src/setup_history.rs` |
| 4 | Automatic lap decomposition | built — Analysis → CORNERS, `F` filters |
| 5 | Driver vs car | built — `core/src/driver_vs_car.rs` |
| 6 | Reference laps | local half built; remote deferred, see below |
| 7 | Engineer confidence | built — `core/src/confidence.rs` |

The Analysis tab draws what happened. This is about making it say **why**, and
about being honest when it does not know. The ideas below are the author's;
the notes under each are what the code already has, what it would need, and
where the difficulty actually sits.

## What already exists, and it is more than it looks

Worth stating first, because most of this needs no new telemetry at all.
`analyzer::TelemetryPoint` is recorded for every sample of every lap and carries:

```
distance  time_ms  speed  gas  brake  gear  steer  lat_g  lon_g  slip_avg  x  y  rpms
```

Distance, lateral G, steering and both pedals, against time, for the whole lap.
That is enough for corner detection, corner-by-corner deltas, and most of the
driver-versus-car separation below. `LapData` adds sector times, damper
histograms, oversteer/understeer/lockup counts, scrubbing incidents, ride
height, tyre temperatures per edge and the loaded setup.

The missing pieces are a **notion of a corner** and a **structure for advice
that has a cause**, not more data.

---

## 1. Corner-by-corner analysis

Not:

```
Speed: 183 km/h
```

but:

```
T7 — 0.18 s lost
  Braking:      14 m later than optimal
  Entry speed:  +7 km/h
  Minimum speed: −4 km/h
  Throttle:     0.21 s later
  Exit speed:   −3.8 km/h
```

— and say where the time actually went.

**What it needs.** The application has no idea what "T7" is. Two ways to get
one:

* **Detect corners from the trace.** A corner is a stretch where `|lat_g|` stays
  above a threshold for long enough, bounded by the straights either side.
  Numbered in distance order. Needs no track data at all, works on any track
  including mods, and is the right answer.
* A per-track table of corner positions. Accurate, and someone has to write and
  maintain one for every track anyone drives. Not worth it.

Detection has one trap worth naming now: a corner must be identified by
**distance**, not by index, or two laps with a different number of detected
corners cannot be compared — which is the whole point. Match by distance
window, and treat a corner that appears in one lap and not the other as no
comparison rather than as a large delta.

## 2. Cause → effect

The important one.

Not "Front-right 96 °C" but:

```
Front-right overheating
  Cause:    high lateral load through T4–T6
  Effect:   FR outer shoulder +11 °C
  Likely:   not enough front mechanical or aero stability
  Try:      −0.2 bar / camber / ARB
  Confirm:  FR I/M/O spread on the next lap
```

**What it needs.** `Recommendation` is currently a flat thing: component,
category, severity, message, action. This wants a chain — evidence, mechanism,
proposal, and *what to look at next time to know whether it worked*. That last
field is the one that makes it an engineer rather than a paragraph: it commits
the advice to being checkable.

It also means the analysis stops being a list of independent checks. "FR outer
shoulder is hot" and "the car understeers in T4–T6" are currently two unrelated
findings; the value is entirely in the link between them, and the link has to be
something the code states rather than something the reader infers.

## 3. Setup ↔ telemetry

Analysis knows what was in the setup, what the car did, and what the driver did.
So it can say:

```
Rear ARB 4/7
  Persistent oversteer in T8–T10 above 70% throttle.
  Rear roll stiffness may be contributing.
  Test: 3/7.
```

and after the change:

```
Setup change detected: rear ARB 4 → 3
  Exit oversteer  ↓ 18%
  T10 exit        +0.12 s
  Lap time        −0.21 s
```

**What it needs.** Both halves exist — `setup_manager` reads the loaded setup and
already diffs two of them, and the analyser has the behaviour. What is missing
is keeping a **history of (setup, laps driven on it)** so a change can be
attributed. Cheap to store and the most convincing output in this whole
document: it is the only part that closes the loop.

One caution: attribute nothing to a change when the tyres, fuel load or track
temperature moved as well, which on a real stint they always do. Say what
changed alongside it rather than claiming a cause.

## 4. Automatic lap decomposition

```
Lap 12       1:48.231
────────────────────────
T1            +0.08
T2            −0.03
T3            +0.17  🔴
T4            −0.02
T5            +0.01
```

and then: *show only where I lost more than 0.10 s*.

Far more useful than twenty graphs at once.

**What it needs.** Falls out of (1) for free once corners are detected. The
filter is the point — the value is in what it hides.

## 5. Driver vs car

The system has to tell "you drove that badly" from "the car will not go faster".

**Driver:** late brake release, excess steering, early throttle lift.
**Car:** understeer regardless of input, tyre overheating, traction limits,
bottoming, brake instability.

**What it needs.** This is the most valuable and the hardest, and the reason is
worth writing down: **one lap cannot tell them apart.** A car that understeers
and a driver who turns in too early produce the same trace. The discriminator is
whether the behaviour persists *across different inputs* — over several laps,
several corners, and varying entry speeds. So it needs the analyser to reason
over a stint rather than a lap, and it should refuse to answer before it has
one.

That refusal is not a limitation to work around. It is the feature — see below.

## 6. Reference laps

Not only a personal best:

```
Your lap  vs  your best  vs  a reference lap
```

```
You lose 0.31 s in T5 against your best.
You lose 0.47 s against the reference.
```

**What it needs.** Saved laps are already `.json` with full metadata, and the
Setup Cloud already exists. Carrying reference *laps* makes the cloud a source
of data rather than a folder of `.ini` files — which is a bigger change than it
sounds, because a lap is orders of magnitude larger than a setup and the current
cloud is fetched whole at runtime.

Start with what costs nothing: compare against the driver's own best, which is
already stored, and add remote references once the shape has proved itself
locally.

## 7. Engineer confidence

```
🟢 High     front tyres 8–11 °C above target across four corners
🟡 Medium   possible rear instability under throttle
🔴 Low      not enough data — one representative corner
```

**Do this one first.** It is the cheapest thing in this document and it changes
every other item, because confidence is a property each of them has to carry
anyway — and retro-fitting it later means revisiting all of them.

It is also the honest core of the whole idea. An engineer who says the same
thing about one strange frame and about four consistent corners is not an
engineer. Most of this document is analysis that will sometimes be wrong; what
separates it from something that merely explains numbers convincingly is
**saying how sure it is, and being willing to say "I do not know yet"**.

Mechanically it is a count of corroborating observations and their spread — the
same shape as the cornering-frames gate the camber advice already uses, which
exists for exactly this reason: it published four confident lines about a car
that was fine, because it judged on a single frame.

---

## Order, and what it turned into

1. **Confidence** — done first, as this said to. `Evidence` counts corroborating
   observations and their spread; `Confidence` is Low/Medium/High off that. One
   observation is never confident however extreme it is, and observations that
   cancel out are Low rather than a perfectly balanced car.

   One thing the sketch did not anticipate: several rules average a great many
   frames into one observation, and counting two settled wheels the same as two
   single frames is wrong in both directions. `Evidence::averaged_over` is that,
   and it let the camber rule's frame gate stop being a precondition bolted on
   beside the advice and become part of how sure the advice says it is.

2. **Corner detection** — done, keyed by distance throughout, with the
   direction having to agree so a chicane's left never matches its right.

3. **Lap decomposition** and the filter — done, on a CORNERS sub-tab. Sections
   tile the lap so the parts sum to the whole.

4. **Corner-by-corner deltas** — done: braking point in metres, entry, minimum
   and exit speed, and throttle timing after the apex.

5. **Cause → effect** — `Chain { cause, effect, confirm, evidence }` on
   `Recommendation`, with `confirm` the field that makes it checkable.

   **Only one rule fills it in so far** — camber, the one this document names.
   The other twenty-odd still carry the old hand-picked score and fall back to
   it. That is the remaining work on this item, and it is a rule at a time
   rather than a redesign.

6. **Setup history** — done, and the caution in §3 turned out to be the whole
   design rather than a footnote. It never claims a cause: it reports what
   changed, what happened after, and every confounder beside it.

7. **Driver vs car** — done, over a stint, and it **refuses below four laps**.

8. **Remote reference laps** — not built, deliberately. §6 says to start with
   the driver's own best and add remote references once the shape has proved
   itself locally; the local half is what CORNERS does. The remote half needs
   the Setup Cloud to carry laps rather than `.ini` files, and a lap is orders
   of magnitude larger than a setup — that is an infrastructure change, not an
   analysis one, and it should follow a release of people actually using the
   local comparison.
