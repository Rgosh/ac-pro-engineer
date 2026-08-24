# Plan — Assetto Corsa Competizione

Written 2026-08-16. The working list for the second simulator, in the order it
has to happen. `docs/roadmap.md` §6 is where this item came from; this is that
item opened up.

**The critical path is a capture, not code.** Everything below except the first
line is a few sittings of work. The first line needs the game running on a
machine, and nothing else can start until it exists.

Status of each line is one of: **done**, **doing**, **blocked** (waiting on
something outside the code), **owed** (agreed, not started).

The same table is published at `/acc/`, generated from the same list. If one
changes, change the other.

---

## Where it stands

| # | Line | Status |
|---|---|---|
| — | The neutral reading, so nothing above the game folder knows a page layout | **done** |
| — | Capabilities that withhold a verdict the game cannot support | **done** |
| — | Every file layout, process name and appid inside the game's folder | **done** |
| — | A registry: a game is one table entry, and no screen names a simulator | **done** |
| — | Detection: the game that is read is the one that is running | **replaced** — see below |
| — | `capture_pages`, which takes the bytes and says what they cannot prove | **done** |
| — | `inspect_capture`, which finds the fields in a captured page | **done** |
| 1 | A capture of ACC's three pages | **done** — 337 s at Spa, in the repository root |
| 2 | The structs, pinned against that capture | **done** |
| 3 | A discriminator, so the wrong parser cannot attach | **done** |
| 4 | The conversion into `Reading` | **done** |
| 5 | Capabilities, each one confirmed by the capture | **done** |
| 6 | Paths, process name, Steam appid | **done** — 805550, `AC2-Win64-Shipping.exe` |
| 7 | The registry entry flips to playable | **done** |
| 8 | Setups, or an honest no | **done** — the honest no |
| 9 | Linux: the bridge into ACC's own prefix | **done** |
| 10 | Thresholds checked against a real ACC lap | **doing** — five laps driven, advice not yet read back |
| 11 | The flags for what ACC measures and Assetto Corsa does not | **done** — brake wear, track limits, track grip |

**Detection stopped deciding which game is read.** It is a choice on the
launcher now, kept in `config.game`, and item 9 is why: the bridge has to be
running inside one game's Proton prefix *before* that game starts, so there is
no process to detect yet — and both games mirror into the same `/dev/shm`
files, so the strong test would answer with whichever published last. A driver
saying which simulator they are in costs one keypress and cannot be wrong.
Detection is still asked and still drawn ("WAITING FOR SIMULATOR…"); it just no
longer picks the thresholds the engineer runs.

---

## 1. A capture of ACC's three pages — done

**What.** The bytes ACC is publishing, taken off a running game and pasted into
`tests_suite/src/shm_layout_tests.rs`.

```bash
cargo run -p ac_core --example capture_pages > acc-capture.txt
cargo run -p ac_core --example inspect_capture acc-capture.txt
```

The second one is what turns bytes into a layout. It decodes every four-byte
word, says whether it is plausibly a temperature, a pressure, a lap time or a
pedal, and finds the runs of four matching floats that are almost always the
wheels. Run against Assetto Corsa's own capture it locates `wheels_pressure`
at 88, `tyre_wear` at 120, the brakes at 348 and the tread triplet at
368/384/400 — which is exactly where the layout tests pin them, and is why it
can be trusted on a page nobody has mapped yet.

**It suggests; it does not conclude.** Every offset it prints has to be
confirmed against a number that was visible in the game when the capture was
taken.

**Why it is first.** Assetto Corsa's offsets are pinned against a capture, and
that is the only reason the ACC-shaped graphics struct was ever caught. Every
other test in the workspace builds a value in Rust and reads it back, so it
round-trips through whatever layout the struct declares and cannot disagree
with the game at all. A struct transcribed from somebody's header file and
never checked is exactly the mistake this project has already made once — it
moved every field past `car_coordinates` by 964 bytes.

**What makes a capture worth having.** A page of zeros pins nothing: a wrong
offset also reads zero. The existing AC graphics capture cannot speak for
anything past offset 300 for that reason, and that was discovered a year
later. `capture_pages` prints where the last written byte is and warns when the
tail is long, so take it:

- past the first completed lap — several fields stay zero until then;
- mid-lap rather than in the pits, so speeds, temperatures and positions are
  all non-zero;
- with TC and ABS set to something other than zero, the lights on, and the pit
  limiter used at least once;
- the static page too, which only settles once a session has loaded.

**On Windows** this is: start ACC, drive, run the tool. **On Linux** see item 9
first — the game publishes inside its Proton prefix and needs the bridge there.

**Done when.** Three hex constants in the layout tests, each with a short tail
of zeros, and a note saying which car, which track and which build of the game
they came from — the AC ones say `1.16.4, Imola, abarth500`, and that is why
they are still trustworthy.

## 2. The structs — done

**What.** `games/assetto_corsa_competizione/structs.rs`: `#[repr(C)]` structs
with `TryFromBytes`, a compile-time size assertion per page, and `offset_of!`
assertions for every field the capture actually proves.

**Where.** A new folder beside `assetto_corsa/`. Nothing outside it changes.

**Done when.** The layout tests parse the captured bytes through the same call
the application uses — `try_read_from_bytes` — and the decoded values make
sense: a speed that matches the lap, temperatures in the right order, a track
length the distance agrees with. Values, not just sizes: the AC tests catch a
reordering because the numbers stop being plausible, which a size assertion
never would.

**What landed.** Physics 800 bytes, graphics 1588, static 820, in
`tests_suite/src/acc_layout_tests.rs` — and the whole 2048-byte mapping is kept
rather than the first `size_of` bytes, which buys the one check a struct-sized
capture cannot make: everything past the end of each struct is zero, so the
structs are not too *short*. That is the question AC's own graphics capture
could not answer for a year.

There is no track length. `track_spline_length` is zero, and so are
`tyre_radius`, `suspension_max_travel`, `max_torque` and `max_power` — which
settles nothing about whether `tyre_radius` is a scalar or four, and does not
need to.

## 3. A discriminator — done

**What.** A check that refuses to read ACC's pages with AC's parser and the
other way round.

**Why it matters more than it looks.** Both games publish under the same names
— `acpmf_physics`, `acpmf_graphics`, `acpmf_static`. The names match and the
layouts do not, which is the single most dangerous property of this pair: a
build that attaches the wrong parser reads plausible-looking garbage and says
confident things about it.

**How.** The mapped size is the first gate and is nearly free. A version or
packet field that has to be in range is the second. Refuse rather than read.

**Done when.** A test feeds AC's captured bytes to ACC's reader and asserts it
refuses, and the reverse.

**What landed.** The version at the top of the static page: Assetto Corsa
writes `1.7`, Competizione writes `1.9`, at the same offset in the same
encoding, which is what a version field is for. Each reader knows the one it
was written against and refuses anything else, on connecting *and* on every
tick — a session of the other game can start while a connection is open.

A page of **all zeros is allowed through**, deliberately: the mapping exists
before the game has published into it, and the simulator that stands in for the
game writes no version at all. Refusing zeros would turn "nothing published
yet", which is normal, into "wrong game", which is not.

Both directions are tested against the real captures, and the tests assert what
the wrong reader *would* have believed — AC's page decodes cleanly as ACC's
static page and names the car correctly, which is exactly why a plausibility
check on one field would not have been enough.

## 4. The conversion into `Reading` — done

**What.** `games/assetto_corsa_competizione/reading.rs`, the counterpart of
AC's. Units into the field names, conventions stated rather than inherited.

**The two to be careful about**, because they are where the boundary earns its
keep:

- **Gear.** `Car::gear` is −1 reverse, 0 neutral. Whatever ACC numbers it, the
  translation happens here and once. Getting this wrong is invisible in tests
  that use the same literal on both sides — it cost three screens a gear during
  the AC refactor, and identical screenshots did not catch it.
- **Tyre wear.** `Car::tyre_wear` is percent of tread **left**, counting down
  from 100. A game that publishes wear the other way round inverts it here
  rather than teaching the engineer a second convention.

**Done when.** A test builds ACC's structs, converts, and asserts the neutral
values — the same shape as
`games/assetto_corsa/reading.rs`'s conversion tests.

**Two more that were not on this list**, and both would have been silent:

- **The session table is not Assetto Corsa's.** ACC numbers practice 0 where
  this project's AC table numbers it 1, so the table copied across would have
  called every practice session a booking and every race a hotlap. The capture
  settles it: session 0, no lap count, no clock — the practice session that was
  driven. Its two extra formats, hot stint and superpole, are new `SessionKind`
  variants.
- **The aids are split across two pages.** The physics page says whether TC or
  ABS is cutting in *now*; the graphics page says which level the driver dialled
  in. AC keeps both on the physics page, so a conversion that reads only physics
  reports a GT3 with the aids switched off.

## 5. Capabilities — done

**What.** ACC's entry in `Capabilities`, every flag confirmed against the
capture rather than assumed.

**What the bytes said**, which is not what was expected:

- `tyre_edge_temps: false` — as predicted. ACC publishes core tyre temperature
  and leaves the tread triplet at 368/384/400 zero for a whole session. This
  withholds the camber rule and the tread-temperature band, which is correct
  and is the entire reason the flags exist.
- `sectors: true` — confirmed: the sector index reached 2 and the last sector
  time 117595 ms.
- **`tyre_wear: false`** — this plan predicted `true` and was wrong. Offsets
  120–132 are zero for the whole recording. ACC publishes brake pad and disc
  life instead, in millimetres, and *that* is the consumable a GT3 stint is
  decided by.
- `setups: false` — see item 8.

**ACC also publishes things AC does not**: brake pad and disc wear, tyre set,
rain tyres, stint time remaining. Each would be a new flag and a new rule, and
each is its own item — not part of the first release of ACC support.

**Done when.** Every flag traces to a value in the capture, and a comment says
which.

## 6. Paths, process and appid — done

**What.** Where ACC installs, where its documents live, what its executable is
called, and its Steam appid — as `PROCESS_NAMES`, `APP_ID` and a `paths.rs` in
its own folder, the way AC has them.

**Read off a machine that has the game**, which is the only place any of it
could come from. Steam's `appmanifest_805550.acf` names the appid and the
install directory; the game itself is `AC2-Win64-Shipping.exe` under
`AC2/Binaries/Win64/`, with `acc.exe` as the launcher Steam actually starts.

Two facts fell out of doing it rather than guessing: ACC's install folder is
`Assetto Corsa Competizione`, spaces and capitals included, where AC's is
`assettocorsa`; and there is nothing under it to scan for car specifications,
because the cars live inside packed Unreal assets. `scan_cars` returns an empty
list and says so.

## 7. The registry entry — done

**What.** `games::registry`'s ACC entry changes from `Support::Planned` to
`Support::Playable(Backend { … })`.

**And then there are two playable games**, which is where "whichever one is
running" turned out to be the wrong question — see the note under *Where it
stands*. `registry::chosen` answers it from the configuration instead, and
`detect_running` keeps its other job: telling a driver their game is up.

The entry carries one field it did not have: the Steam appid. It is what names
the Proton prefix, and it used to be a constant in the launcher — which is the
same as saying this program reads one game.

**Done when.** `registry::playable().count() == 2`, and the test that asserts
the planned list no longer names Competizione.

## 8. Setups, or an honest no — done, the honest no

ACC keeps setups as JSON under
`Documents/Assetto Corsa Competizione/Setups/<car>/<track>/`, not AC's INI.
That is a second `SetupStore`: scan, root, file name, serialise.

**Shipping without it is a supported state**, not a broken screen: `setups:
false`, no store, and the tab says the game keeps none this program can read.
That is what the capability flags bought, and it is the right first release if
the week is tight.

That is what shipped. The launcher says so before a driver starts, too: the
game row lists what the chosen simulator reports and what it does not, so
advice going quiet on ACC reads as a property of the game rather than as a
broken feature.

## 9. Linux: the bridge into ACC's prefix — done

**What.** `shm-bridge.exe` runs inside the game's Proton prefix and mirrors the
Windows mappings into `/dev/shm`. `tui/src/platform/linux.rs` launches it
against Assetto Corsa's appid, so on Linux ACC publishes into its own prefix
and nothing reads it.

**The bridge itself needed no change**: it maps 2048 bytes per page, which is
larger than any page either game writes — ACC's longest is the 1588-byte
graphics page — and both use the same mapping names. What changed is which
prefix it is launched into: the appid comes from the chosen game's registry
entry rather than from a constant in the launcher, and choosing another game on
the launcher stops the bridge and starts a new one in the other prefix.

**One consequence worth stating.** Both games mirror into the *same*
`/dev/shm/acpmf_*` files. Running both at once is not a supported state and the
discriminator in item 3 is what stops it being a silent one.

**On Windows none of this exists** — the game writes the mappings itself.

## 10. Thresholds against a real lap — doing

**A real session has now been driven through the application**: five laps in
Competizione on 17 August 2026, with the telemetry, the screens and the lap
analysis all reported correct. That closes the half of this item that was about
the *reading* — the numbers arriving are the game's own, and they are right.

What is still open is the half about the *advice*, and one thing about it is
worth writing down: those five laps ran on a build from before the car classes
landed, so the thresholds behind them were still the one-size band — 70–105 °C
tyres and an 800 °C brake ceiling — which on a GT3 is a band nothing ever
reaches. The engineer being quiet during that session is explained by it. The
next stint is the one that tests the class windows.

**The part that is not plumbing.** The engineer's numbers were chosen for AC's
tyre model. On ACC:

- **Camber and tread temperature are withheld**, so they cannot be wrong.
- ~~**Tyre pressure** runs against AC's compound names.~~ **Done.**
  `compound_band` matches on substrings and has tests naming ACC's own
  `dry_compound` and `wet_compound`: the first is a racing car on slicks and
  lands in `Racing`, the second reaches `Wet`.
- ~~**Brake temperature** runs on AC's thresholds.~~ **Done in v0.4.0**, by
  `games::car_class`: a GT3 is judged against 600 °C front and 450 °C rear from
  published operating windows, per axle, rather than against one 800 °C ceiling
  chosen for road cars. What is still owed is the stint that checks it.

The recording gives the first number to check against: **520 °C front and
257 °C rear** is normal running for a GT3 on carbon, and `alerts.brake_temp_max`
is 800 in the shipped configuration — chosen for road cars, and a threshold
those brakes would reach in a hard stint.

Two more are owed and are not in this list because nothing had measured them
until the capture:

- ~~**`surface_grip` is not published.**~~ **Done.** `track_grip` is a
  capability now: false on ACC, so the cold-pressure calculator adds nothing
  for a green track it was never told about, and the Strategy tab says "not
  measured" rather than drawing 0.0 % in red. The named states ACC publishes
  instead were *not* mapped to a fraction — that would be inventing the
  measurement this was about.
- ~~**Compound names.**~~ **Done**, see above. The static page carries the real
  rubber — `DHD2` and `WH` in the recording — which is where a band narrower
  than "racing slick" would come from if one is ever wanted.

**What is actually left**, as of v0.4.2: one thing, and it needs the wheel
rather than the keyboard. Everything in this section that could be settled by
reading code has been. Also new in v0.4.2 and worth a lap of attention: the
circuit now measures its own length from the car's distance travelled, because
ACC publishes none — so every answer this program gives in metres appears on
that game for the first time, and Spa should read about 7004 m.

**Done when.** A stint has been driven on ACC and every line of advice it
produced has been read against the numbers that produced it —
`engineer_probe` prints exactly that, and takes the game as an argument:

```bash
cargo run --bin simulator acc
cargo run -p ac_core --example engineer_probe -- 8 assetto_corsa_competizione
```

## 11. What ACC measures and Assetto Corsa does not — done

Three of the four flags §5 of the implementation brief listed have landed, each
with a rule that consults it and a test that takes it away:

* **`brake_wear`** — pad and disc thickness in millimetres. This is the trade
  the two games make: ACC publishes no tyre wear and does publish what is left
  of the brakes, and Assetto Corsa is the other way round. Neither gets the
  other's rule. The thresholds are millimetres and are the first thing to check
  against a stint, beside §10's.
* **`lap_validity`** — a lap the game called invalid is stored as invalid, so it
  stops setting best sectors. Assetto Corsa never says, and a lap nobody called
  invalid stays valid — which is the absence of a verdict rather than one.
* **`track_grip`** — see §10.

`rain_forecast` and `mfd_state` are still owed, and deliberately: their offsets
are pinned and there is no rule behind them yet, which would make the flag a
guess with a name.

**Setups are still `false`, and this is the reason.** ACC's setup files are
JSON whose numbers are *clicks*, not physical units, and the click-to-value
mapping differs per car. There is no ACC setup file on the machine this was
built on to pin the format against — the game writes `Setups/` only once a
driver saves one — so writing a reader from memory of the format is exactly the
mistake this project keeps a recording to avoid. One saved setup unblocks it.

---

## What ACC will not have, and it is a decision

- **No in-game panel.** The five-window overlay is a Custom Shaders Patch Lua
  app; CSP is an Assetto Corsa mod and ACC is Unreal Engine. §8 of the roadmap
  decided this and the site says it before anybody downloads expecting
  otherwise.
- **No camber advice**, and no tread-temperature band. See item 5.
- **No setups at first.** See item 8.

## The order

1 → 2 → 3 → 4 → 5 → 7 gets telemetry on screen. 6 and 9 are what make it
reachable on a real machine, and 9 is Linux-only. 8 and 10 are what make it
honest. 10 is the one that must not be skipped before calling it supported.

Everything except 10 has landed. **Until a stint has been driven, this is
telemetry that arrives and advice that has not been checked against it** — the
thresholds are still Assetto Corsa's, and the two known-wrong ones are named
above.
