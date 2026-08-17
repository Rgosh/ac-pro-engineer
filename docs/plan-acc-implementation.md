# ACC — implementation brief

Written 2026-08-16, and **built**. Kept because it is the evidence: this is
what the recording said, field by field, before any of it was code. Where the
implementation found something this brief got wrong, the correction is marked
*corrected* and the wrong version is left visible — a brief that quietly agrees
with the code afterwards is worth nothing as evidence.

`core/src/games/assetto_corsa_competizione/` is the result;
`tests_suite/src/acc_layout_tests.rs` is the same recording as assertions.

`docs/plan-acc.md` is the *why* and the order. This is the *what*, field by
field, with every number that has been measured marked as such.

**Read `CLAUDE.md` and `AGENTS.md` first.** The rules there are not style
preferences — most were written after a bug that cost an evening.

---

## 0. What already exists — do not rebuild it

| Piece | Where | What it does |
|---|---|---|
| Neutral reading | `core/src/games/reading.rs` | `Reading { car, session, fixed, capabilities }`. Nothing above `games/` knows a page layout. |
| Capabilities | `core/src/games/mod.rs` | Flags that make rules withhold. Default is **nothing measured**. |
| Registry | `core/src/games/registry.rs` | One table. ACC is already an entry, `Support::Planned`. |
| Detection | `registry::detect_running` | Process present **and** telemetry reachable. |
| Capture tool | `core/examples/capture_pages.rs` | One snapshot as a hex constant. |
| Recorder | `core/examples/record_pages.rs` | A whole session as ranges per four-byte word. |
| Inspector | `core/examples/inspect_capture.rs` | Guesses what each offset is. |
| Session script | `tools/record-session.sh` | Bridge into the game's prefix + recorder, one command. |
| Boundary tests | `tests_suite/src/boundary_tests.rs` | Fail if any file outside `games/` names a simulator or its layout. |

The engineer, the analyser, the corner detection, the confidence model, the
debrief and every terminal screen are **game-neutral already**. None of them
changes for ACC.

## 1. The folder to create

```
core/src/games/assetto_corsa_competizione/
    mod.rs        GAME_ID, CAPABILITIES, PROCESS_NAMES, APP_ID,
                  telemetry_is_reachable, impl Source
    structs.rs    the three #[repr(C)] pages, with offset assertions
    reading.rs    structs -> Reading (Car, Session, Fixed)
    paths.rs      install root, documents dir
    setups.rs     later — see §7
```

Mirror `games/assetto_corsa/` exactly. Nothing outside this folder changes
except one line in `registry::GAMES`.

## 2. Measured offsets — physics page

**Verified** from a real recording: 337 s, 8376 samples, a GT3 car, two laps,
Assetto Corsa Competizione under Proton. `assetto-corsa-competizione-20260816-2051.txt`
in the repository root is that recording; keep it, it is the evidence.

Where a range is given, that is what the field actually did during the session.

| Offset | Field | Type | Measured |
|---|---|---|---|
| 0 | `packet_id` | i32 | 0→130019 |
| 4 | `gas` | f32 | 0→1.0 |
| 8 | `brake` | f32 | 0→1.0 |
| 12 | `fuel` | f32 | 62.0 |
| 16 | `gear` | i32 | 0→7, **8 distinct** |
| 20 | `rpm` | i32 | 0→8511 |
| 24 | `steer_angle` | f32 | −0.9→1.0 |
| 28 | `speed_kmh` | f32 | 0→252.73 |
| 32,36,40 | `velocity[3]` | f32 | ±66 |
| 44,48,52 | `acc_g[3]` | f32 | −9.2→8.5 |
| 56–68 | `wheel_slip[4]` | f32 | 0→12.7 |
| 72–84 | `wheel_load[4]` | f32 | **all zero — not published** |
| 88–100 | `wheel_pressure[4]` | f32 | 27.77 / 27.32 / 26.76 / 26.56 psi |
| 104–116 | `wheel_angular_speed[4]` | f32 | 0→203.7 |
| 120–132 | `tyre_wear[4]` | f32 | **all zero — not published** |
| 136–148 | `tyre_dirty_level[4]` | f32 | **all zero — not published** |
| 152–164 | `tyre_core_temp[4]` | f32 | 92.5 / 87.9 / 90.4 / 88.2 °C |
| 168–180 | `camber_rad[4]` | f32 | **all zero — not published** |
| 184–196 | `suspension_travel[4]` | f32 | −0.0096→0.075 |
| 200 | `drs` | f32 | zero (GT3) |
| 204 | `tc` | f32 | 0→1.0 |
| 208 | `heading` | f32 | −3.14→3.14 |
| 212 | `pitch` | f32 | −0.095→0.179 |
| 216 | `roll` | f32 | −0.111→0.069 |
| 248 | `pit_limiter_on` | i32 | 0→1 |
| 252 | `abs` | f32 | 0→1.0 |
| 264 | `autoshifter_on` | i32 | 0→1 |
| 276 | `turbo_boost` | f32 | 0→0.0254 |
| 288 | `air_temp` | f32 | 27.16 °C |
| 292 | `road_temp` | f32 | 27.98 °C |
| 296,300,304 | `local_angular_vel[3]` | f32 | ±3.2 |
| 308 | `final_ff` | f32 | −1.79→2.21 |
| 348–360 | `brake_temp[4]` | f32 | **519.7 / 509.2 / 257.2 / 256.1 °C** |
| 364 | `clutch` | f32 | 0→1.0 |
| 368–380 | `tyre_temp_i[4]` | f32 | **all zero — not published** |
| 384–396 | `tyre_temp_m[4]` | f32 | **all zero — not published** |
| 400–412 | `tyre_temp_o[4]` | f32 | **all zero — not published** |
| 420–464 | `tyre_contact_point[4][3]` | f32 | world XYZ per wheel |
| 468–512 | `tyre_contact_normal[4][3]` | f32 | ±1.0 |
| 516–560 | `tyre_contact_heading[4][3]` | f32 | ±1.0 |
| 564 | `brake_bias` | f32 | **0.76** |
| 568,572,576 | `local_velocity[3]` | f32 | −8.9→70.2 |
| 588 | `current_max_rpm` | i32 | **8650** — agrees with static `max_rpm` |
| 640–652 | `slip_ratio[4]` | f32 | ±0.48 |
| 656–668 | `slip_angle[4]` | f32 | ±0.43 |
| 696–708 | tyre temperature, repeated | f32 | identical to 152–164 |
| 716–728 | per-corner constant | f32 | 0.76 / 0.76 / 0.24 / 0.24 |

**The tail, identified** (*corrected* — this section used to end with a list of
offsets nobody had settled). The page is **800 bytes**, and its last written
byte in the recording is 795:

| Offset | Field | Measured |
|---|---|---|
| 580, 584 | `p2p_activations`, `p2p_status` | zero — no GT3 has push-to-pass |
| 592–636 | `mz[4]`, `fx[4]`, `fy[4]` | **all zero — not published** |
| 672, 676 | `tc_in_action`, `abs_in_action` | zero; `int` per ACC's header, unproven |
| 680–692 | `suspension_damage[4]` | zero — nothing was damaged |
| 712 | `water_temp` | **84 °C** |
| 716–728 | `brake_pressure[4]` | 0.63/0.63/0.20/0.20 braking, peaking at the bias |
| 732, 736 | `front/rear_brake_compound` | 1, 1 |
| 740–752 | `pad_life[4]` | **29 mm** |
| 756–768 | `disc_life[4]` | **32 mm** |
| 772–780 | ignition, starter, engine running | 0, 0, 1 |
| 784–796 | kerb / slip / g / ABS vibration | ±1.0 |

`brake_pressure` is the one worth reading twice: it is per-corner and it lands
on exactly `brake_bias` front to rear, 152 bytes away, which is why both are
pinned.

## 3. Measured offsets — graphics page

| Offset | Field | Type | Measured |
|---|---|---|---|
| 0 | `packet_id` | i32 | 0→39853 |
| 4 | `status` | i32 | 0→2 |
| 8 | `session_type` | i32 | constant (practice) |
| 12–41 | `current_time` | u16[15] | UTF-16 |
| 42–71 | `last_time` | u16[15] | UTF-16 |
| 72–101 | `best_time` | u16[15] | UTF-16 |
| 102–131 | `split` | u16[15] | UTF-16 |
| 132 | `completed_laps` | i32 | 0→1 |
| 136 | `position` | i32 | 0→1 |
| 140 | `i_current_time` | i32 | 0→161642 ms |
| 144 | `i_last_time` | i32 | 2147483647 when no lap |
| 148 | `i_best_time` | i32 | 2147483647 when no lap |
| 152 | `session_time_left` | f32 | −1.0 (unlimited) |
| 156 | `distance_traveled` | f32 | 0→13244.6 m |
| 164 | `current_sector_index` | i32 | 0→2 |
| 168 | `last_sector_time` | i32 | 117595 ms |
| 176–196 | `tyre_compound` | u16[33] | UTF-16 |
| 248 | `normalized_car_position` | f32 | 0→1.0 |
| **252** | **`active_cars`** | i32 | **1** |
| **256** | **`car_coordinates[60][3]`** | f32 | first car XYZ matches physics `tyre_contact_point` |
| 976 | `car_id[60]` | i32 | — |
| 1216 | `player_car_id` | i32 | — |
| 1232 | `ideal_line_on` | i32 | 0→1 |
| 1236 | `is_in_pit_lane` | i32 | 0→1 |
| 1244 | `mandatory_pit_done` | i32 | 0→1 |
| 1260 | `main_display_index` | i32 | 0→4 |
| 1268 | `tc` | i32 | 0→8 |
| 1280 | `abs` | i32 | 0→6 |
| **1284** | **`fuel_x_lap`** | **f32** | **4.24 L/lap** |

**This is where ACC diverges from Assetto Corsa.** AC has `car_coordinates[3]`
directly after `normalized_car_position`; ACC inserts `active_cars` and then a
sixty-car array. That is the 964 bytes that shifted every field in an early
version of this project, and it is now measured rather than assumed.

**`fuel_x_lap` is a float.** One published Rust binding declares it `i32`. The
bytes say otherwise — 4.24 litres per lap, which is a GT3 figure.

## 4. Measured offsets — static page

| Offset | Field | Type | Measured |
|---|---|---|---|
| 400 | `sector_count` | i32 | 3 |
| 412 | `max_rpm` | i32 | 8650 |
| 416 | `max_fuel` | f32 | 120.0 |

*Corrected.* This section used to say everything after 416 read zero. It does
not: the **numbers** after 416 do, and the **strings** do not. The static page
is 820 bytes and its last written byte is 754.

| Offset | Field | Measured |
|---|---|---|
| 0 | `sm_version` | **"1.9"** — Assetto Corsa writes "1.7", and this is what tells the two apart |
| 68 | `car_model` | `lamborghini_huracan_gt3_evo` |
| 134 | `track` | `Spa` |
| 200, 266 | `player_name`, `player_surname` | `Andrea`, `Caldarelli` — the nickname is empty |
| 420–460 | suspension travel, tyre radius, turbo | zero |
| 524 | `track_configuration` | the literal placeholder `track config` |
| 604 | `car_skin` | the literal placeholder `skin` |
| 680 | `pit_window_end` | −1000 |
| 688 | `dry_tyres_name` | **`DHD2`** |
| **754** | `wet_tyres_name` | **`WH`** — two-byte aligned, so 754 and not 756 |

The zeros still matter, and the two consequences below are unchanged:

* the disagreement between sources over whether `tyre_radius` is a scalar or
  `[f32; 4]` **cannot be settled from this capture** and does not matter for
  reading, since the field is empty either way;
* **there is no track length.** `Fixed::track_length_m` stays 0 on ACC, and
  everything that reports metres must say "not measured" rather than invent
  one. `LapData::track_length_m` already treats 0 that way.

## 5. Capabilities

### Existing flags, decided by the capture

| Flag | ACC | Why |
|---|---|---|
| `tyre_edge_temps` | **false** | 368–412 all zero. Withholds the camber rule and the tread-temperature band. |
| `tyre_wear` | **false** | 120–132 all zero. Withholds the wear rule **and** the stint wear projection. |
| `sectors` | **true** | `current_sector_index` and `last_sector_time` both move. |
| `setups` | **false** at first | See §7. |

`tyre_wear: false` contradicts what `docs/plan-acc.md` predicted. The capture
is right and the plan was wrong; the plan says so now.

### New flags — three landed, two still owed

Each needs a field on `Capabilities`, **a rule that consults it**, and a test
that takes it away and asserts the verdict disappears — the shape already in
`engineer.rs`'s `mod capabilities`. A flag with no rule behind it is a guess
with a name, which is why the two without a rule are still not there.

| Flag | Means | AC | ACC | State |
|---|---|---|---|---|
| `brake_wear` | pad and disc life are published | false | true | **landed** — offsets 740 and 756, and a rule that reads them |
| `lap_validity` | the game says whether a lap counted | false | true | **landed** — offset 1408; an invalid lap sets no best sector |
| `track_grip` | `surface_grip` is a real fraction | true | false | **landed** — see below |
| `rain_forecast` | rain now, in 10 and in 30 minutes | false | true | owed — offsets 1560/1564/1568 pinned, no rule yet |
| `mfd_state` | what the driver dialled into the MFD | false | true | owed — offsets 1532–1552 pinned, no rule yet |

`track_grip` was not on the original list, and it is the one the recording
added: ACC leaves offset 1240 zero and says `track_grip_status` instead. Left
ungated, the cold-pressure calculator clamps the missing figure to 0.80 and
adds 0.3 psi for a green track — on every lap of every session, for a condition
nobody measured. That is a rule that was *wrong* rather than one that was
missing, which is why it landed with the first three.

## 6. What ACC gives that Assetto Corsa cannot

From the documented inventory, to be confirmed field by field against a
recording. **This is the payoff of supporting ACC** — it is not a lesser
Assetto Corsa, it is a different set of measurements.

**Physics:** `brake_pressure[4]`, `pad_life[4]`, `disc_life[4]`,
`front/rear_brake_compound`, `slip_ratio[4]`, `slip_angle[4]`,
`suspension_damage[4]`, `water_temp`, ignition/starter/engine-running,
four vibration channels.

**Graphics:** `rain_intensity` + `_in_10min` + `_in_30min`, `is_valid_lap`,
`delta_lap_time`, `estimated_lap_time`, `fuel_estimated_laps`, `used_fuel`,
`gap_ahead`, `gap_behind`, global flags including yellow per sector,
`mfd_tyre_pressure`, `mfd_fuel_to_add`, `mfd_tyre_set`, `current_tyre_set`,
`track_grip_status`, `driver_stint_time_left`, `wiper_stage`.

**Static:** `dry_tyres_name`, `wet_tyres_name`.

Three of these are worth features of their own, in this order:

1. **`pad_life` / `disc_life` replace tyre wear.** ACC withholds tyre wear and
   publishes brake wear instead. For a GT3 stint that is the consumable that
   actually decides the race.
2. **`mfd_tyre_pressure` reopens setup attribution.** §1.2 of the roadmap cut
   that feature because Assetto Corsa cannot say which setup is loaded. ACC
   publishes what the driver dialled in, so "what you set" versus "what the
   engineer recommends" becomes answerable — **on ACC only**.
3. **`is_valid_lap` fixes a real gap.** The analyser treats every lap as valid
   because AC never says otherwise. ACC does.

## 7. Setups

ACC keeps setups as JSON under
`Documents/Assetto Corsa Competizione/Setups/<car>/<track>/`, not AC's INI.
That is a second `registry::SetupStore` — `scan`, `root`, `file_name`,
`serialise`, all four or none.

**Shipping without it is a supported state**, not a broken screen: `setups:
false`, no store, and the tab already says the game keeps none this program can
read.

## 8. Order of work — done, except the last

Each step leaves the tree green. All nine landed; §9's thresholds are the one
that needs a stint rather than a compiler.

1. **`structs.rs`** from §2–§4. `#[repr(C)]`, `TryFromBytes`, a compile-time
   size assertion per page, and `offset_of!` assertions for **every field the
   recording actually proves**. Do not assert an offset the capture leaves at
   zero — a wrong offset also reads zero.
2. **Layout tests** in `tests_suite/src/shm_layout_tests.rs`, in the same shape
   as AC's: parse the captured hex through `try_read_from_bytes` and assert the
   decoded values are the ones in the tables above. Values, not sizes.
3. **The discriminator.** Both games publish under the same names
   (`acpmf_physics` and the rest), so the wrong parser must be unable to
   attach. A test feeds AC's captured bytes to ACC's reader and asserts it
   refuses, and the reverse. **This is not optional** — today
   `SharedMemory::get` accepts any mapping at least as large as the struct.
4. **`reading.rs`.** The two conventions that must be got right:
   * **`Car::gear` is −1 reverse, 0 neutral.** ACC uses AC's numbering —
     measured: 8 distinct values starting at 0, so 0 is reverse. One published
     binding claims 0 is neutral; the bytes disagree. Getting this wrong is
     invisible in a test that uses the same literal on both sides.
   * **`Car::tyre_wear` is percent remaining, 100 = new.** ACC does not publish
     it at all, so leave it at default and set `tyre_wear: false`.
5. **Capabilities**, §5. Every flag traced to a value in the bytes, with a
   comment saying which.
6. **`paths.rs`, `PROCESS_NAMES`, `APP_ID`.** Read off a machine that has the
   game — **appid 805550**, confirmed from a Steam manifest. The executable
   name must be read the same way, not guessed.
7. **The registry entry** flips from `Support::Planned` to
   `Support::Playable(Backend { … })`. `registry::playable().count()` becomes 2
   and `detect_running` starts doing real work.
8. **Linux: the bridge.** No change to `shm-bridge` itself — it maps 2048 bytes
   per page and ACC's largest page fits (1588). What changed is which prefix it
   is launched into: the appid comes from the registry entry now.

   *Which* entry turned out to be the interesting part. Not the running game:
   the bridge has to be up before the game asks for the mappings, so there is
   no process to detect yet, and both games mirror into the same `/dev/shm`
   files so the telemetry test would answer with whichever published last. It
   is the game the driver chose on the launcher — `config.game` — and choosing
   another one stops the bridge and starts a new one in the other prefix.
9. **Thresholds.** See §9.

## 9. The thresholds will lie before anything else does

The engineer's numbers were chosen against Assetto Corsa's tyre and brake
models. On ACC:

* **Brake temperature is the rule most likely to be wrong first.** The
  recording shows **520 °C front, 257 °C rear** as normal running temperature
  for a GT3 on carbon. Whatever `alerts.brake_temp_*` is set to was picked for
  road cars.
* **Tyre pressure** runs, and the target is a user setting so it is less
  dangerous — but `analyze_tyre_pressure` matches AC's compound names as
  strings, and ACC's compound names will not match any of them and will fall to
  the default band.
* Camber and tread temperature are withheld by §5, so they cannot be wrong.

**Done when** a stint has been driven on ACC and every line of advice has been
read against the numbers that produced it. `cargo run -p ac_core --example
engineer_probe` prints exactly that.

## 10. How to verify, at every step

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings
cargo run --release --bin tui_tester      # then: git diff --stat screenshots
```

The screenshots must not change while ACC is being added. Anything that moves
them is a change to Assetto Corsa's behaviour, which this work must not make.

To take another recording:

```bash
./tools/record-session.sh
```

## 11. About the reference project

`gitlab.com/ai-projects219/race-engineer/frontend/acc-shared-memory-rust`,
MIT OR Apache-2.0, by Naresh Kumar. It was read for its **field inventory** —
what ACC exposes — and §6 above comes from its documentation.

**No code was taken and none should be.** Not because the licence forbids it
— MIT permits reuse with the copyright notice kept — but because this project's
own capture is better evidence, and the reference already disagrees with the
bytes twice: it declares `fuel_x_lap` an `i32` where the measurement says
float, and it documents gear 0 as neutral where the measurement says reverse.
Copying either would have imported a bug that no test here would catch.

If some part of it is ever worth using, take it openly and add the copyright
notice to `NOTICE`, the way `webpki-roots` is already handled.
