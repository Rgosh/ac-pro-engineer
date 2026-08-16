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
| — | Detection: the game that is read is the one that is running | **done** |
| — | `capture_pages`, which takes the bytes and says what they cannot prove | **done** |
| — | `inspect_capture`, which finds the fields in a captured page | **done** |
| 1 | A capture of ACC's three pages | **blocked** — needs the game |
| 2 | The structs, pinned against that capture | **owed** |
| 3 | A discriminator, so the wrong parser cannot attach | **owed** |
| 4 | The conversion into `Reading` | **owed** |
| 5 | Capabilities, each one confirmed by the capture | **owed** |
| 6 | Paths, process name, Steam appid | **blocked** — needs the game |
| 7 | The registry entry flips to playable | **owed** |
| 8 | Setups, or an honest no | **owed** |
| 9 | Linux: the bridge into ACC's own prefix | **owed** |
| 10 | Thresholds checked against a real ACC lap | **owed** |

---

## 1. A capture of ACC's three pages — blocked

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

## 2. The structs — owed

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

## 3. A discriminator — owed

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

## 4. The conversion into `Reading` — owed

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

## 5. Capabilities — owed

**What.** ACC's entry in `Capabilities`, every flag confirmed against the
capture rather than assumed.

**What is expected**, and none of it is settled until the bytes say so:

- `tyre_edge_temps: false` — ACC publishes core tyre temperature, not the
  tread across it. This withholds the camber rule and the tread-temperature
  band, which is correct and is the entire reason the flags exist.
- `sectors: true`, `tyre_wear: true` — expected, unconfirmed.
- `setups: false` at first — see item 8.

**ACC also publishes things AC does not**: brake pad and disc wear, tyre set,
rain tyres, stint time remaining. Each would be a new flag and a new rule, and
each is its own item — not part of the first release of ACC support.

**Done when.** Every flag traces to a value in the capture, and a comment says
which.

## 6. Paths, process and appid — blocked

**What.** Where ACC installs, where its documents live, what its executable is
called, and its Steam appid — as `PROCESS_NAMES`, `APP_ID` and a `paths.rs` in
its own folder, the way AC has them.

**Blocked, and deliberately.** None of these is written down until it is read
off a machine that has the game. A guessed appid is the kind of thing that
costs an evening the day it turns out to be wrong, and there is no way to tell
from here.

## 7. The registry entry — owed

**What.** `games::registry`'s ACC entry changes from `Support::Planned` to
`Support::Playable(Backend { … })`.

**And then there are two playable games**, which is the first time
`registry::detect_running` does real work. It already asks both halves —
process present *and* telemetry reachable — so a driver in ACC gets ACC.

**Done when.** `registry::playable().count() == 2`, and the test that asserts
the planned list no longer names Competizione.

## 8. Setups, or an honest no — owed

ACC keeps setups as JSON under
`Documents/Assetto Corsa Competizione/Setups/<car>/<track>/`, not AC's INI.
That is a second `SetupStore`: scan, root, file name, serialise.

**Shipping without it is a supported state**, not a broken screen: `setups:
false`, no store, and the tab says the game keeps none this program can read.
That is what the capability flags bought, and it is the right first release if
the week is tight.

## 9. Linux: the bridge into ACC's prefix — owed

**What.** `shm-bridge.exe` runs inside the game's Proton prefix and mirrors the
Windows mappings into `/dev/shm`. `tui/src/platform/linux.rs` launches it
against Assetto Corsa's appid, so on Linux ACC publishes into its own prefix
and nothing reads it.

**The bridge itself needs no change**: it maps 2048 bytes per page, which is
larger than any page either game writes, and both use the same mapping names.
What changes is which prefix it is launched into — the appid comes from the
game's registry entry rather than from a constant in the launcher.

**One consequence worth stating.** Both games mirror into the *same*
`/dev/shm/acpmf_*` files. Running both at once is not a supported state and the
discriminator in item 3 is what stops it being a silent one.

**On Windows none of this exists** — the game writes the mappings itself.

## 10. Thresholds against a real lap — owed

**The part that is not plumbing.** The engineer's numbers were chosen for AC's
tyre model. On ACC:

- **Camber and tread temperature are withheld**, so they cannot be wrong.
- **Tyre pressure** runs. The target is a setting the driver sets, so it is
  less dangerous than it looks, but the compound bands in
  `analyze_tyre_pressure` are AC's compound names matched as strings — ACC's
  compound names will not match any of them and will fall to the default band.
  Check that on a real lap before calling ACC supported.
- **Brake temperature** runs on AC's thresholds. GT3 carbon brakes work far
  hotter than most AC cars, so this is the rule most likely to say something
  wrong first.

**Done when.** A stint has been driven on ACC and every line of advice it
produced has been read against the numbers that produced it —
`engineer_probe` prints exactly that.

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
