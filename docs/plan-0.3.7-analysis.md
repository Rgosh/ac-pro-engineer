# v0.3.7 — turning Analysis into an engineer

A plan. Nothing here is built.

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

## Order

1. **Confidence**, on the advice that exists. Small, and everything else assumes
   it.
2. **Corner detection** from the trace, keyed by distance.
3. **Lap decomposition** and the "only show me losses over 0.10 s" filter —
   free once corners exist.
4. **Corner-by-corner deltas** against the driver's own best.
5. **Cause → effect**: restructure `Recommendation` around evidence and a check
   for next lap.
6. **Setup history**, so a change can be attributed to an effect.
7. **Driver vs car**, over a stint rather than a lap.
8. **Remote reference laps** in the cloud.

One through four are a release on their own and would already change what the
Analysis tab is for. Five onwards is where it stops being a telemetry viewer.
