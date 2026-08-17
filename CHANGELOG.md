# Changelog - RaceEngineer (Pro Engineer)

All notable changes to this project will be documented in this file.

## [v0.4.0] - 2026-08-18

**The point:** a second simulator, and an engineer that knows what car it is
looking at.

### At a glance, everything since v0.3.6

- **Assetto Corsa Competizione is read.** Its three shared-memory pages, pinned
  to a recorded GT3 session at Spa rather than to a header file. Dashboard,
  tyres, brakes, fuel, strategy and lap analysis all work on it.
- **Which simulator is a choice on the launcher**, not a guess — `GAME: < … >`,
  on every platform, with a list of what that game can and cannot measure.
- **The wrong parser can no longer attach.** Both games publish under the same
  three names; each reader now refuses pages that declare the other's version.
- **The engineer judges a car against its class.** GT3, GT4, Formula, touring,
  road — tyre windows and per-axle brake ceilings from published operating
  ranges instead of one band meant for road cars.
- **Brake pad and disc wear**, on the game that measures it, in place of the
  tyre wear it does not.
- **Track limits**: a lap the game called invalid is stored as invalid.
- **The Proton helper no longer blocks the game from starting.** It comes up
  when you press START and goes away when you return to the launcher, and it
  mirrors pages the game already owns — so the order you start things in
  stopped mattering.
- **Corner-by-corner lap analysis**, confidence beside every line of advice,
  car-versus-driver over a stint, and a cause/effect/check chain on every rule.
- **Licence: MIT → AGPL v3** from this release. v0.3.6 and earlier stay MIT.
- **The name is Pro Engineer.** It read "AC Pro Engineer", which stopped being
  true the moment it read a second game. Nothing about an install changes: the
  binary, the panel's folder and the shared-memory names are what the game and
  Custom Shaders Patch look for, and renaming those would break every existing
  install for a word.

Everything below is the detail.

### 🏎️ The engineer knows what car it is

- **Tyre and brake thresholds come from the class of car.** One band —
  70–105 °C tyres, an 800 °C brake ceiling — was doing for a Formula car and a
  Fiat 500 alike, which is why so little was ever said: a GT3's 520 °C fronts
  never reached it and a Formula car's cold tyre never fell below it. Classes
  are read off the car's id and, on Assetto Corsa, the game's own tags; the
  windows come from published operating ranges, with the sources in
  `docs/plan-0.4.0-car-classes.md`.
- **Brake ceilings are per axle.** Fronts do the work; one number for four
  corners is either too low for the fronts or blind to the rears.
- **An unrecognised car keeps your own settings**, and so does any threshold
  you have set yourself. Only untouched defaults are replaced.
- **Tyre temperature is judged on whatever the game measures** — the tread mean
  where there is one, the core where there is not — and the verdict says which.
  Competizione used to get no word about tyre temperature at all.

**The point:** the Analysis tab drew what happened. It says **where** now, and
how sure it is.

### ⚖️ Licence: MIT → AGPL v3

The project is licensed under the **GNU AGPL v3** from this release onward.
Nothing changes for anyone using the application: it is free, it stays free,
there is no account and no paid tier. What changes is what may be done with the
source.

- **Using it, and changing it for yourself, is unrestricted.** The licence
  conditions passing a copy on, not what runs on your own machine.
- **Publishing something built on it** now requires that your source is open
  too, under the same licence, with this project credited. Under MIT it did not.
- **Keeping your own source closed, or selling a product with this code inside
  it**, needs written permission first — rgoshbbb@gmail.com. There is no price
  list; terms are per case.
- **v0.3.6 and every release before it stay MIT, permanently.** That grant
  cannot be withdrawn, and `LICENSE-MIT-HISTORICAL` keeps its terms. The AGPL
  applies to everything committed after `9ba92a3`.
- **`shm-bridge` is untouched.** It is a fork of poljar's work and keeps its own
  MIT licence.

`LICENSING.md` is the plain-language version — what needs asking, what does not,
and what happens to a closed product found shipping this code without asking.
`CONTRIBUTING.md` covers the sign-off patches now need, and why.

### 🏁 A second simulator: Assetto Corsa Competizione

**Telemetry from ACC arrives.** Its three shared-memory pages are read, turned
into the same neutral reading Assetto Corsa produces, and every screen, the
analyser and the engineer work on it unchanged — none of them knows which game
it is looking at.

- **The layout was measured, not transcribed.** 337 seconds and 8376 samples of
  a Huracán GT3 EVO at Spa, recorded off a running game under Proton, with what
  every four-byte word did over the session. That recording is in the
  repository and it is what the tests assert against: 4.24 litres a lap that
  the tank divides by to give the 14.6 laps the next field holds, 520 °C front
  brakes against 257 °C rear, pad and disc thickness in millimetres.
- **The wrong parser can no longer attach.** Both games publish under the same
  three names — `acpmf_physics` and the rest — and on Linux they mirror into
  the same `/dev/shm` files, so a reader on the wrong pages gets numbers rather
  than an error. Each now refuses a page that declares the other's
  shared-memory version, on connecting and on every tick.
- **What ACC does not measure, it says so.** No tyre wear and no tread
  temperatures across the tyre, so the wear advice and the camber advice stay
  silent on this game instead of reporting four unworn tyres as four destroyed
  ones. The launcher lists what the chosen game reports before you start, so
  advice going quiet reads as a property of the game.
- **Tyre temperature falls back to the core where a game measures no tread.**
  Every screen averaged the three tread readings, which on ACC are zero — so
  four tyres read 0 °C, which is not a gap but a confident wrong answer in the
  direction of "stone cold". The core temperature is a real reading of the same
  tyre and is what is drawn now. The advice that rests on the tread stays
  silent, because inner minus outer has no substitute.
- **Brake wear advice, on the game that measures it.** ACC publishes what is
  left of the pads and discs in millimetres, per corner, and the engineer reads
  it: pads low, pads done, disc thin — with the same chain every other rule
  carries, and a check a driver can take off this screen two laps later. It is
  the trade the two games make, and neither gets the other's rule.
- **A lap the game called invalid is stored as invalid.** ACC reports track
  limits; Assetto Corsa never has, so every lap there was treated as clean —
  which was the absence of a verdict, not one. An invalid lap now sets no best
  sector.
- **Track grip stopped being invented.** ACC does not publish a grip figure, and
  the missing zero was read as the greenest track there is: the cold-pressure
  target quietly gained 0.3 psi on every lap of every session. The Strategy tab
  says "not measured" now, and the calculator adds nothing.
- **No setups on ACC yet, and no in-game panel ever.** Its setups are JSON whose
  numbers are clicks rather than units, and the click-to-value mapping differs
  per car — writing that reader from memory of the format is the mistake the
  recording exists to prevent, and there is no ACC setup file to pin it against
  until one is saved in the game. The panel is a Custom Shaders Patch app, and
  CSP is an Assetto Corsa mod.
- **The simulator stands in for either game.** `simulator acc` publishes
  Competizione's pages — the same drive, in that game's layout, with the arrays
  it does not publish left at zero. `engineer_probe` takes a game as an
  argument and reads it through the registry, so the advice can be read against
  the numbers that produced it on both.

**Which simulator is a choice now, on the launcher** — `GAME: < … >`, on every
platform. It used to be detected, and detection is the wrong tool for this
pair: the Linux bridge has to be running inside one game's Proton prefix
*before* that game starts, so there is no process to detect yet, and both games
publish under the same names. A wrong answer costs a bridge in the wrong
prefix and an engineer running the other game's thresholds. The row also says
what the chosen game can and cannot measure, so advice going quiet reads as a
property of the game rather than as a broken feature. Choosing another game
restarts the bridge in its prefix.

**One thing is still owed, and it is named here rather than left to be found.**
The engineer's temperature thresholds were chosen against Assetto Corsa's cars
and have not been read against a stint on a GT3 — the brake alert sits at
800 °C where ACC's carbon runs at 520, so it is likelier to stay quiet than to
cry wolf, and the brake-wear thresholds are millimetres that want the same
check. `docs/plan-acc.md` §10 is the list, and `engineer_probe` is how it gets
done: every line of advice printed next to the numbers that produced it.

### ✨ Added

- **A CORNERS sub-tab: where the lap actually went.** Corners are found in the
  trace — a stretch where lateral load stays up long enough to be a corner
  rather than a kink — so it needs no track data and works on mods. Each is
  charged the track from its own entry to the next corner's entry, which is
  where a bad exit is paid for, and the per-corner deltas plus the run to T1 add
  up to the lap's own delta. The worst corner is pulled apart: braking point in
  metres, entry, minimum and exit speed, throttle timing after the apex.
- **`F` hides everything that cost less than a tenth.** That filter is the
  point — twenty corners with a number beside each is another table to read.
- **How sure the engineer is**, beside every line of advice rather than after
  it: a count of corroborating observations and their spread, not a number
  picked while writing the rule. One observation is never confident however
  extreme, and four that cancel out are Low rather than a balanced car.
- **The car or the driving, over a stint.** A car that understeers does it on
  the lap you got right and the lap you got wrong; a mistake follows the
  driving. **It refuses to answer below four laps**, which is the feature and
  not a limitation — the wrong answer here sends somebody to change a setup that
  was never the problem.
- **What a setup change did**, measured against the laps either side of it —
  and, in the same breath, everything else that moved at the same time. It never
  claims a cause: the tyres are always older and the track is never the same
  temperature, and a driver told "the ARB gained you 0.2 s" has been misled by
  their own tooling.
- **Advice carries a chain** — cause, effect, and *what to look at next time to
  know whether it worked*. Every rule with a mechanism now states one:
  pressures, tyre temperatures, wear, brakes, brake bias, camber, bottoming,
  rake, force feedback, coasting, understeer, oversteer and over-rotation.
  `confirm` is the field the idea rests on — advice nobody can check is not
  advice — so it names something a driver can actually go and look at, in the
  units they chose.

  **The two fuel rules deliberately have none**, and that is a finding rather
  than an omission: "the fuel will not last" is arithmetic on what is in the
  tank, not a mechanism, and its check is the same number a lap later, already
  on the screen. A chain there would be three fields of ceremony making the
  advice look better researched than it is.

  A third state is also deliberate. A rule that can explain itself but counts
  one whole-lap number rather than several agreeing observations carries a chain
  with **empty evidence** and keeps its hand-picked confidence — one counter is
  one observation however large it gets, and feeding it to the evidence model
  would dress a single number up as corroboration.

  Two rules were sharpened on the way. **Bottoming now knows which corners**
  grounded rather than only that one did — the loop stopped at the first, which
  was enough to raise the alert and not enough to say anything about it — and
  the wear rule says *laps left on the worst corner* where it used to leave a
  percentage, because a percentage is not what a driver deciding whether to stop
  needs to hear.

### 🔌 For anyone building on it

- **The other end of the UDP feed.** Set `overlay.receive_from` and this shows
  another machine's telemetry and *their* engineer's advice in your own panel.
  One network only — two houses behind two routers is NAT, which needs a relay
  that is not built. No discovery and no championship mode: you type an address.
- **A frame knows when it is not yours.** A new flag, which costs no frame
  version, so a lap counter about a car you are not sitting in cannot be
  mistaken for your own telemetry gone wrong.

### 🗑 Removed

- **Attributing a change to a setup**, which was written and then cut before
  release. It rests on identifying the loaded setup, and Assetto Corsa publishes
  fuel, brake bias, pressures and camber and nothing else — so two setups
  differing only in a roll bar are indistinguishable, which is exactly the
  change the feature existed to measure. Worse, fuel burning off mid-stint could
  make a *different* saved setup start matching and invent a change that never
  happened. A feature that misses the real thing and occasionally reports a
  false one is worse than no feature. It comes back when the setup can be
  identified rather than guessed.

### 🐞 Fixed

- **The screenshot mock filled `distance` with metres where AC publishes a
  normalised 0..1.** Every consumer of a trace read that as three thousand
  laps — the delta graph resampled to a ceiling of 1.0 and drew a single
  sample.
- **The POST-STINT column had never been screenshotted**, so it went out
  unreviewed release after release.
- **Confidence markers drew as a dash.** 🟢🟡🔴 are recent enough that the
  terminal font renders all three identically; filled, half and hollow circles
  carry it without needing colour.

## [v0.3.6] - 2026-08-10

**The point:** the panel could tell you what was happening; it had nothing to
say about the lap you had just finished. Now it does, with the last three laps
to page through.

> The frame changed, so **Linux needs a matching `shm-bridge.exe`** — **[B]** on
> the launcher's overlay card fetches one. Everything else is optional and off
> until you turn it on.

### ⚠️ Breaking

- **Overlay frame version 6**, 712 bytes to 2484, so **`shm-bridge.exe` has to
  be updated on Linux**. Everything new is appended — no existing offset moved,
  and a panel or bridge one version behind misreads nothing, it simply does not
  see the debrief.

### ✨ Added

- **A lap debrief in the game.** Its own window: what the engineer made of the
  lap you have just finished — pressures and temperatures against your own
  windows, camber per axle, brakes, and how the lap was driven. `<` and `>` page
  through the last three laps. Eight lines per lap rather than four, because a
  lap can go wrong in more than four ways at once and whatever came fifth used
  to be dropped silently.
- **Sector times against the session's best.** Four tenths spread across a lap
  is a shrug; four tenths in sector three is a corner to go and look at, and a
  lap time on its own cannot tell them apart.
- **What is left**: laps of tyre life in the worst corner, laps of fuel, and how
  long you have been on this set of tyres. All three existed already and only
  ever reached the terminal — which is on the other monitor with a helmet in the
  way.
- **Compare against your best lap, not just the one before.** People race their
  own best; the lap before is what says whether the last change helped.
- **Inner / middle / outer tyre temperatures** under each corner, coloured by
  the spread rather than the heat: a tyre can be in its window and still riding
  on one edge. Off by default.
- **Settings of its own** for all of it — how many lines (zero switches the whole
  thing off), what to show, whether a finished lap pulls the window back to it,
  and its own look: backing plate from transparent to solid black, text size,
  line spacing, a rule between lines, upper case. The window scrolls, which it
  needs to: eight lines plus a header plus sectors do not fit a window sized for
  four.

  Assigning a wheel button to the paging was in this release and has been taken
  out again: the binding was written into Assetto Corsa's own `controls.ini`
  correctly and the press never arrived back. It returns when it works, rather
  than shipping as a control that looks bound and does nothing.

### 🐞 Fixed

- **The camber verdict in a debrief was backwards half the time.** The lap
  summary took the temperature spread's magnitude and threw its sign away, so a
  front tyre whose *outer* edge ran 13 °C hot — a car short of negative camber —
  was told to take camber out. There is a test for each direction now.
- **The terminal and the panel gave different advice about the same lap.** The
  post-stint column computed its own verdicts, in three hundred lines where the
  analysis and the spans that drew it were the same code, and the two had
  already drifted. One implementation now, 311 lines lighter, and a clean lap
  says so instead of leaving an empty column that reads as "it did not run".
- **A lap that published nothing says nothing.** Every average reads zero on a
  session that ended before the analyser had anything to average, and zero
  pressures are not flat tyres.
- **Four corners of one problem are one line** in the lap summary too, the way
  the live advice has been since v0.3.5.
- **`Wear:`, `T:` and `B:` were English in both languages** — the only words in
  the panel that were, sitting under every corner where they are hardest to
  miss.
- **A spent GitHub allowance said "403 Forbidden"**, which reads as a permission
  problem and sends people looking for a token they do not need.
- **The LuaJIT harness reported success on a panel that failed to load.**
- **`README.txt` described an archive that no longer exists** — it was written
  for a Windows-only bundle while the published one holds both builds and the
  bridge.

### 🧱 Under it

- **The core reads the game.** The shared-memory reader lived in the terminal,
  so a user interface owned the connection to the simulator. It is
  `core/src/games/assetto_corsa/` now, behind a `Source` trait, so a second
  simulator becomes a folder beside it rather than conditionals through the
  middle of the engineer. The panel moved to `assets/frontends/csp-panel/`: it
  is the Assetto Corsa front end, not "the overlay".
- **The computed frame goes to a list of sinks** rather than to one hard-wired
  writer, and the producer never waits for any of them — a sink that fails is
  dropped rather than allowed to stall the loop feeding the driver's overlay.
- **A raw UDP feed, for anyone who wants to write a front end.** Set
  `overlay.broadcast_to` and the computed frame goes out as JSON. That is all it
  is: **nothing ships that reads it, and there is no spectating, no LAN mode and
  no relay** — this is a documented hook and an address to point at, off unless
  you set one. `docs/ARCHITECTURE.md` says where it is meant to go.
- The lap summary is **data now, not five hundred lines of ratatui**:
  `debrief::debrief(&LapData, &AppConfig) -> Vec<Recommendation>`, which is
  exactly why the panel had never had a word to say after a lap.

## [v0.3.5] - 2026-08-08

**The point:** the in-game overlay stopped being "the panel that sometimes
works". It opens before the race and in the pits, keeps its settings when you
close its window, and shows up to eight lines of advice instead of four. There
is one overlay now rather than two. In the terminal, every key does what the
screen says it does, and every one of them can be rebound.

Run in a real session on Linux (Proton) and on Windows 10.

### ⚠️ Breaking

- **The overlay frame grew from 440 to 712 bytes** (version 5, eight advice
  slots). On Linux this means **`shm-bridge.exe` has to be updated**: an older
  bridge maps too few bytes, CSP silently refuses the mapping, and the panel
  waits forever for an application that is running. Press **[B]** on the
  launcher's overlay card to fetch a current one, or **[C]** for a report on
  which of the three pieces does not fit.
- **F10 and F11 are no longer bound to anything**, and the `--overlay-test-*`
  flags are gone.

### ✨ Added

- **The panel works outside a session** — in the garage, in the pits, before the
  green. It now tells *waiting for the car* apart from *waiting for AC Pro
  Engineer*; it only ever said the second, which sent people hunting for a
  broken bridge that was fine.
- **Up to eight lines of advice**, with a slider for how many to draw and the
  number the application actually sent printed under it.
- **Rebind any key** — a KEYS category in Settings, stored as text in
  `config.json`, working on both keyboard layouts.
- **A `Changed` tab in the panel's settings**: everything that differs from the
  defaults, what it was, and a reset beside each line. The setting making the
  panel look wrong is the one you do not remember touching.
- **New advice is drawn differently** from advice that has been on screen for
  four laps.
- **`[C]` — the whole overlay diagnosis, in the application.** *Why is the panel
  blank* is the question this program gets asked most; it used to have one
  answer, and it was a `cargo` command.
- **`shm-bridge.exe --verify`** opens the mapping with exactly the call CSP
  makes and prints what is inside it.

### 🗑 Removed

- **The desktop overlay on F10 and its control centre on F11.** There were two
  overlays. That one had no implementation on Linux at all, and on Windows it
  drew a worse copy of what the CSP panel already draws — it did not survive
  exclusive fullscreen, never appeared in VR, and was invisible to AC's own
  screenshots. About 1,500 lines went with it.

### 🐞 Fixed

- **The panel now really does install itself.** Two separate reasons it did
  not. Windows detection started from two literal paths under Program Files and
  read Steam's library list only out of a root found that way, so a machine
  with Steam anywhere else had no libraries at all and the game was
  unfindable — it asks the registry first now, then the environment, then every
  mounted drive, and it will also find a copy that is in no Steam library.
  And the install was attempted once, while the application was starting, and
  never again: whatever went wrong that one time meant no panel for the rest of
  the session, recorded in a log file and nowhere anyone would look. It is
  retried when Assetto Corsa starts, and a failure now says so on the launcher
  card in the operating system's own words.
- **The panel forgot everything.** Closing its window unloaded the script, and
  the settings table was CSP's storage proxy rather than a table — so a ticked
  box was not merely unsaved, it was lost on the next frame. Settings are also
  written to a plain file now, which makes "did it save" a question with an
  answer.
- **The keys the screens named.** Hints promised keys that reached no handler,
  headings named function keys the tabs were never on, and the launcher listed
  two of its six keys. Every hint, heading and help page is printed from the
  bindings now, and a test walks all nine tabs to keep it that way.
- **`[B]` fetched the newest published bridge**, not the one built for this
  release — and then reported "nothing to fetch", which is indistinguishable
  from everything being fine.
- **Pressing `[B]` crashed the application**, and the Setup Cloud was one
  keystroke from the same crash: a blocking HTTP client built inside the async
  runtime.
- **The engineer said one thing four times.** "FL COLD / FR COLD / RL COLD / RR
  COLD" filled every slot in the frame; it is "All four COLD" now, or "Fronts",
  or "Left side".
- **The camber advice fired on every straight**, four lines at a time, about a
  car with nothing wrong with it — a tyre only says something about camber
  while it is loaded sideways. It is judged over cornering now, and the degrees
  in it come from what AC reports the car is running rather than from a step
  index in the setup file that no reading can turn into degrees. The post-stint
  verdict on it was also backwards half the time.
- **Wear no longer screams on lap three**, and the tyre bars scale to your own
  threshold instead of a hard-coded 94 %.
- **Advice comes in the units you chose.** The pressures printed raw psi to
  someone working in bar.
- Plus the panel's plate, its clipped captions, its overflowing corner
  readouts, the console's `Again` button, a checkout running whichever
  `shm-bridge.exe` was nearest rather than the one built for it, and two bugs in
  the test harness that had been agreeing with a panel that was wrong.

### 🧱 Under it

- **The Lua panel is 19 modules instead of one 2,429-line file** — settings,
  language, theme, layout, formatting, frame, blocks, widgets, console, and one
  file per window.
- **Every picture in the README is generated by the code that draws it**, the
  terminal's and the panel's alike, and there are now pictures of the panel
  itself rather than of a window that no longer exists.
- The README was rewritten: installation per platform, every screen, the full
  key table, every command-line flag, and a troubleshooting section for the
  overlay.

## [v0.3.4] - 2026-08-05

> ### ⚠️ The overlay in this release is a DEMO
>
> A preview, not a finished feature. It is published so that it can be checked
> on real machines, which is the one thing that cannot be done while developing
> it.
>
> - **It has never once been run on Windows.** The tests pass by
>   cross-compilation and clippy is clean against the Windows target, but not a
>   line of it has executed there.
> - **It has not been checked in game by the author of these changes.** The
>   panel is driven under LuaJIT and LÖVE and every `ui.*` call it makes is
>   checked against the installed CSP — that is not the same as a session.
> - Some console captions and the `Wear:`, `T:`, `B:` prefixes are not
>   translated yet.
>
> The official release comes after real sessions on both systems. Report what
> breaks: the panel's status window now shows every version at once.

**The point:** before this release the overlay could not work for anyone on
Linux. v0.3.3 was tagged eleven minutes before the commit that taught
`shm-bridge` to map the overlay, so every published bridge created only AC's own
pages. Confirmed by scanning the artifact.

### ⚠️ Breaking

- **Overlay frame version 4**: the application's version was added, so the panel
  can tell that the game is drawing an older copy of it. The field is last, so
  no offset moved. The application, the panel and `shm-bridge.exe` have to come
  from the same release; the panel installs itself, the bridge comes from **[B]**
  on the overlay card.

### 🐞 Fixed

- **The panel did not load at all.** `ui.Icons and ui.Icons.Settings` at file
  level: `ui.Icons` only has to be truthy to be indexed, and the table argument
  is built before `pcall`, so `pcall` does not protect it. Every window drew the
  error text instead of the panel.
- **Both developer-mode switches fell through to nil.** `applyDemo` and
  `DEMO_ADVICE` were declared below their callers, which makes them globals to
  those callers. The fourth instance of that trap here.
- **Both of the panel's versions lied.** `manifest.ini` showed `1.0` for eleven
  releases running, and the panel had no version of its own.
- **Both harnesses reported OK on a broken panel** — LuaJIT drew 27 strings
  instead of 140, and LÖVE did not count a load failure as an error.
- **The overlay card clipped its own diagnostics** at 66 columns.

### 🚀 Added

- **The bridge says who it is.** It writes `/dev/shm/acpe-bridge.info` and
  compiles its version into its own binary, so it can be identified without
  being run.
- **The card judges all three pieces**, and **[B]** downloads the published
  bridge, verifying it before it replaces anything. The old one is kept as
  `.previous`.
- **A bridge update check at startup** — it only looks; fetching is a keypress.
  The application's own version is not touched by this path.
- **The panel says the game is holding an older copy of it** and offers to
  restart AC. Switched off in Panel → Blocks.
- **`bridge_probe`** — which bridge is on disk, which is running, and whether
  the overlay can work at all.
- **`--export-overlay <dir>`** — write the panel out for a manual install.
- **`proton-setup.sh` in the archive** — the `protontricks` commands without
  which CSP does not load at all. There are no fonts in the archive and there
  cannot be: the terminal draws with its own font, the panel through CSP's
  DirectWrite, and fonts are installed into the prefix (`corefonts`), which is
  what the script does.


## [v0.3.2] - 2026-08-04

A small follow-up to v0.3.1. Four pieces of functionality that were fully
implemented but had no way to reach the user are now wired up, one wrong
number in the analysis tab is corrected, and three things that ran far more
often than they needed to no longer do.

### 🚀 New Features

- **Screenshot the interface with Ctrl+S.** A complete SVG renderer for a
  drawn terminal buffer already existed inside `tui_tester`, where it
  generates the images in the README; the application itself had no way to
  capture what it was showing. Frames are written to
  `<data>/screenshots/<timestamp>.svg` and the path is reported in the status
  line. SVG keeps the text selectable and needs no image encoder.
- **Tyre pressure targets are on screen.** `ColdPressureCalculator` and
  `TyrePressureOptimizer` were both fully implemented in `ac_core` and called
  only by the test suite. A third Engineer sub-tab shows what to set the tyres
  to cold so they reach the configured hot target at the current air
  temperature and track grip, and what each corner's inner-versus-outer
  temperature spread says to change.
- **Frame and tick timing in the footer.** The render loop and the background
  tick thread contend for the same state mutex, so when one stalls it is
  usually because the other holds the lock — and from the outside both look
  identical, because the numbers stop moving either way. The footer now shows
  frames per second and how long ago the tick completed, in red past 500ms.

### 🛡️ Fixed

- **A missing sector split no longer zeroes the best sector.** The analysis tab
  computed each best sector as a plain minimum over the raw values, which
  includes the zeroes left by a lap whose split was never captured and by the
  unused third slot of a two-sector track. One such lap pinned that sector to
  0.000 and made the "Optimal" row a lap time no car could set. The analyzer's
  own `theoretical_best_lap_ms` — which filters those out and had no callers
  outside its unit test — is used instead, and a sector with nothing recorded
  renders as a dash rather than as a time.
- **The config is no longer rewritten on every launch.** The decision to save
  compared the file's text against a re-serialisation, so different
  indentation, a different key order, or a serialisation failure all triggered
  a write. The comparison is now between values, and formatting stops
  mattering. Migration and validation still write, which they must.
- **The mouse is no longer captured.** Capture was enabled at startup and no
  mouse event was ever handled, so the only effect was taking selection and
  copy away from the terminal — which is how anyone gets a lap time or an
  error message out of a TUI and into a bug report.
- **The timing readout stays blank until a frame is measured**, rather than
  reporting a fabricated "0fps" before anything has been drawn.

### ⚡ Performance

- **The delta-versus-best series is cached.** It was recomputed every frame,
  and computing it resamples two telemetry traces — cloning and fully sorting
  up to 7200 points each — to arrive at an answer that cannot change, since
  both laps are finished.
- **Setup folders are rescanned on a ten second heartbeat** instead of twice a
  second. The scan walks three directory trees and parses every setup ini in
  them, for a directory that changes only when the user saves a setup from
  inside the game.

### 🧹 Internal

185 tests, up from 171. The SVG renderer moved out of `tui_tester` into
`ui::screenshot` so the binary and the application share one implementation;
the README screenshots regenerate byte-identical from it.

## [v0.3.1] - 2026-08-03

A bug-fix release, and a large one. Three features that the interface has
always advertised — the version carousel, saving your settings, and the Setup
Cloud browser — did not work at all and now do. Four reachable crashes are
gone. Assetto Corsa is finally found on Linux.

47 commits, 171 tests (up from 130), green on Linux and Windows.

### ⚠️ Read This First

- **Your cold tyre pressure targets will change.** The calculator scales its
  recommendation by `surface_grip`, which used to read a constant `0.0` and
  clamp to a floor of `0.80` — so every recommendation carried the same fixed
  compensation regardless of track state. With real grip being read, a
  well-rubbered track (≈0.94) produces roughly a third of the previous
  adjustment. Numbers will differ from v0.3.0 for the same car and track.
  This is the fix working, not a regression.
- **Any settings you saved before this release were never written to disk.**
  The Settings tab did not persist anything, so it comes up with defaults one
  last time. From now on it saves as you edit.
- **Lap records saved before this release may be missing.** Personal bests
  were compared against the world record rather than your own history, so
  `records.json` only ever gained an entry from someone who had beaten it.

### 🚀 New Features

- **Assetto Corsa is found on Linux.** The install root was probed as four
  hardcoded Windows drive letters, so `content/cars` was never located and
  every car-spec lookup returned nothing. Setups were looked for in
  `~/Documents`, but under Proton the game is a Windows process writing inside
  its own prefix. The new `ac_paths` module walks the real Steam roots
  (`~/.steam/steam`, `~/.local/share/Steam`, Flatpak and Snap homes, Program
  Files on Windows), reads Steam's `libraryfolders.vdf` so a library on any
  drive is found rather than guessed at, and locates the Proton prefix by app
  id. `ac_install_path` and `ac_documents_path` in the config override both.
- **The Setup Cloud browser works.** The Setup tab handled only Up, Down and
  B, so pressing B opened a browser onto a permanently empty setup list with
  no way to install anything — while the tab's own hint line, the help overlay
  and the README all documented `D` to download. Arrows navigate, Enter
  reloads a car, `D` installs, PgUp/PgDn scroll the details. Fetching runs off
  the render thread, so the UI no longer freezes on a five-second HTTP call.
- **Fuel strategy no longer waits on AC.** Every fuel figure was gated on
  `gfx.fuel_x_lap`, which reads zero for the whole of lap one and sits in the
  part of the graphics page not yet confirmed against a live capture.
  Consumption measured across completed laps now fills in, so the strategy tab
  works from lap two regardless of that field.
- **Honest connection status.** The footer distinguishes `LIVE`,
  `AC RUNNING - NO DATA` and `AC NOT RUNNING` rather than collapsing three
  tracked states into ONLINE/OFFLINE. Panels with no telemetry say which it is
  instead of drawing nothing.
- **Richer CSV export.** RPM, lateral G, longitudinal G and average slip were
  being dropped even though the trace carries them — the three things an
  external tool is most often opened for. Files are named after the car, track
  and lap instead of colliding on `lap_3_export.csv`, and a failed export now
  reports itself instead of failing silently.
- **Terminal-too-small screen.** Below 80x20 the app shows its current and
  required size instead of drawing into an area that cannot hold the layout.
  The startup resize is now grow-only, so it stops shrinking the window of
  anyone running maximised.
- **Ghost delta.** The `show_ghost_delta` toggle now selects the delta source:
  with it on, the readout compares against your own recorded best lap through
  `calculate_ghost_delta`, which was fully implemented and had no caller.

### 🛡️ Crashes Fixed

- **Narrow terminals.** Four `Rect` fields in the Setup tab subtracted
  constants from a `u16` width and height. Below 20 columns they wrapped to
  around 65530 and indexed out of the render buffer.
- **Mid-download panic.** The updater's progress bar built its trailing
  segment with `"░".repeat(20 - filled)` on an unclamped percentage, so a
  response body longer than its Content-Length aborted the app while the user
  watched it update.
- **NaN from stale shared memory.** `Gauge::ratio` asserts its input is within
  0.0..=1.0 and `clamp` returns NaN unchanged, so a single garbage float from
  a zeroed `/dev/shm` page took the app down. All nine gauge call sites reject
  non-finite input first.
- **100% CPU from a config file.** `AppConfig::validate` had no caller outside
  its own unit test, so `update_rate: 0` reached `event::poll` and
  `thread::sleep` and spun two cores. Validation now runs on load, and covers
  the pressure targets, alert bands, temperature limits and shift point that
  previously had no bounds at all.

### 🛡️ Things That Silently Did Nothing

- **Version carousel arrows.** `check_for_updates` dropped every release older
  than the running one, so on the newest build the list held a single entry
  and Left/Right had nowhere to move — while the launcher rendered a "you
  won't be able to switch back" warning for versions that could never appear.
- **Update checks after being offline.** The check ran once at startup, so a
  machine behind a captive portal kept an empty carousel for the whole session
  with no way to retry. Selecting the UPDATE item now re-checks, debounced to
  once a minute.
- **Saving settings.** `handle_input` mutated the config and nothing wrote it
  back; `apply_config` had no callers, so changes did not take effect until a
  restart. The `auto_save` and `show_ghost_delta` toggles were read by nothing.
- **Personal bests.** Compared against the world record, and the whole block
  was nested inside a car-specs lookup that always failed on Linux — so no
  record was created, compared or saved there at all, which also left
  `world_record` as None and disabled the off-pace advice.
- **Setup auto-detection.** `match_score` can only produce 0/20/25/30/45/50/
  55/75 and the threshold was `> 60`, so only a perfect three-way match ever
  qualified. One lap of burnt fuel dropped it to 55 and silently blanked the
  "(NOW: x%)" hints in the brake-bias and camber advice.
- **Suspension roll-asymmetry warning.** It compared `avg_ride_height[0]`
  against itself, so the difference was always exactly zero. AC publishes ride
  height per axle, not per corner, so the check cannot be written against this
  data and has been removed rather than left looking functional.
- **Simulator detection on Linux.** `is_process_running` matched only
  `simulator.exe`, but the Linux build is called `simulator`, so the launcher
  waited forever on the platform the bridge exists for.

### 🛡️ Wrong Numbers

- **Driving-style aggression** combined the lateral and *vertical* G axes, so
  a stationary car scored 40% and braking or acceleration was invisible to it.
- **Out-laps scored perfect tyre management.** With no sample above the speed
  gate, pressure deviation computed to 0.0 and the score to a perfect 100 — an
  out-lap rated better than a hot lap, and the advice recommended inflating by
  27.5 psi against a 0.0 psi reading.
- **Mistake counts scaled with Update Rate.** Oversteer, understeer, lockup
  and scrubbing counters were divided by a fixed sample count, so changing the
  rate in Settings halved every score and made laps recorded at different
  rates incomparable.
- **The final sector split raced the lap counter** and could land in the
  following lap. It is derived from the lap time now. `AcStatic::sector_count`
  is honoured too, so 2- and 4-sector mod tracks produce a theoretical best.
- **Fuel targets under-fuelled.** A timed race ends when the leader
  *completes* the lap the clock ran out on, and the lap already in progress
  still has to be finished; the target accounted for neither.
- **Stale fuel warnings.** `fuel_laps_remaining` was never cleared, so
  BOX BOX BOX could fire after a refuel on a value measured before the stop.
- **Torn shared-memory reads.** The physics page is rewritten at 333 Hz while
  ~600 bytes are copied out of it. Pages are re-read when AC's `packet_id`
  moves mid-copy, so a frame spliced from two game ticks no longer reaches the
  jerk accumulators and peak-G tracking as a phantom lockup.
- **Track-map bounds** were serialised as `f32::MAX`/`f32::MIN` sentinels when
  a lap had no usable coordinates, so anything computing `max - min` from a
  saved lap got -6.8e38.
- **Units were ignored.** Target pressures printed a hardcoded "PSI" and
  ambient temperatures a hardcoded "C" whatever the Display settings said;
  alert thresholds printed no unit at all. Tyre temperature *spreads* were
  converted as absolute temperatures, adding a 32°F offset that does not
  belong to a difference. Min Speed was folded from a seed of 999.0, so an
  empty trace displayed "999.0 km/h" as if it were a measurement.

### 🛡️ Keys, Text and Alerts

- The first-run prompt could not be exited with Ctrl+C, q or Esc — the first
  screen every new user sees, and Enter was the only way out.
- F1 did not close the help modal that says "PRESS ESC, ?, Q, OR F1 TO CLOSE"
  in nine places.
- Esc in the analysis load menu quit the whole session back to the launcher,
  while the menu's own footer promised "ESC: Close".
- Held keys were dropped on Windows, which reports them as `Repeat` rather
  than `Press`.
- `S` in the analysis tab saved the fastest lap rather than the selected one.
- Tabs were documented as F1–F9 in nine screen titles, the navigation summary
  and the README; they are 1–9. The footer advertised "[H: Help]" for a key
  that is not handled, and F10 was described as a compact UI mode when it
  toggles the game overlay. Keys documented nowhere — Tab/Shift+Tab, F11,
  Ctrl+L, E, PgUp/PgDn, the A/S/D category switches — are now listed.
- Brake and tyre-temperature alerts pushed a fresh recommendation on every
  frame the condition held — roughly sixty a second per corner, burying every
  other message. They now use the same hysteresis as every comparable alert.
- Status messages never cleared, so "Exported CSV: ..." stayed pinned to the
  footer for the session and a stale message looked like a fresh one.
- Twelve locale keys existed only in Russian; a test now enforces parity. A
  malformed locale override produced an empty dictionary in silence,
  degrading the whole UI to raw key names.

### 🛡️ Data, Shutdown and Security

- **Durability.** The records file, config and CSV export renamed a temp file
  into place without flushing it first, so a power loss could publish a
  correctly-named empty file. Two instances saving at once also shared a temp
  path, which is the one way that pattern corrupts rather than merely loses.
- **Records validation.** A zero or negative lap time was accepted, written to
  disk, then dropped by the read path on next load — which reads to the driver
  as a personal best vanishing between sessions.
- **Crash reports and logs** were written relative to the working directory,
  unwritable when launched from a shortcut or installed under Program Files.
  The crash report was then dropped in silence. A logging failure also aborted
  startup before the TUI was drawn.
- **Stale `/dev/shm` mappings.** shm-bridge's cleanup returned on the first
  failure, leaving the remaining pages behind zero-filled — and the app maps
  those without complaint, reporting a healthy connection to a dead feed.
- **Quitting could hang forever** waiting on a bridge that never acknowledged
  the exit request. Bounded to five seconds, and errors inside that task are
  no longer discarded.
- **A missing `protontricks-launch` was fatal**, so anyone running AC natively,
  through another launcher, or simply reviewing saved laps offline could not
  start the app at all.
- **INI injection.** A newline in a downloaded setup's notes field opened a new
  line in the file AC parses as a car setup, letting a `[SECTION]` be smuggled
  past everything the downloader validates.

### ⚡ Performance

- `is_process_running` reads every process on the system and was called twice
  per frame from the launcher — roughly 124 full process-table scans a second
  while sitting in a menu. Cached for one second.
- Loading a car's cloud setups no longer blocks the render thread.

### 🧹 Internal

- **Shared-memory layout tests** parse graphics, physics and static pages
  captured verbatim from a live AC 1.16.4 session through the same zerocopy
  call the app uses. Previously every test built an `Ac*` value in Rust and
  read it back, so none could detect a mismatch with the game.
- **The test suite now compiles under the workspace edition and lints.** It
  was pinned to edition 2021 against the workspace's 2024 and omitted
  `[lints] workspace = true`, so `unwrap_used` and `panic` were silently
  unenforced across it. Two modules that asserted nothing about this project
  were removed — one never imported the crate under test, the other spawned
  `sh` and checked its exit status.
- **CI builds with `--locked`** and runs on `actions/checkout@v6`, matching the
  release workflow.
- **Version numbers come from the manifest.** The release scripts and the
  generated screenshots hardcoded `v0.2.3`, two releases behind.
- **Screenshots regenerated**, including `Help_Modal.svg`, which was
  byte-identical to `Analysis_Radar.svg` because the tester set a field the
  renderer does not read.
- **The commit convention is written down** in AGENTS.md.

## [v0.3.0] - 2026-08-02

### 🚀 New Features & Enhancements
- **Automated Release Pipeline**: Added `cargo-dist` configuration and a GitHub Actions workflow that builds and publishes Linux and Windows binaries, with shell and PowerShell installers.
- **Continuous Integration**: Added a CI workflow running `cargo fmt --check`, `cargo clippy --workspace --all-targets` and `cargo test --workspace` on Linux and Windows.

### 🛡️ Bug Fixes & Stability
- **In-App Updater Platform Selection**: The updater looked for a `-linux` asset suffix that no release has ever published, so on Linux no update was ever offered. Asset selection is now based on the running OS, rejects artifacts that are not the application (`shm-bridge`, installers, checksums), and refuses to install a build for a foreign platform.
- **In-App Updater Archive Support**: The updater now unpacks the application binary out of release archives (`.tar.gz` on Linux, `.zip` on Windows) instead of only handling bare binaries.

---

## [v0.2.3] - 2026-07-30

### 🚀 New Features & Enhancements
- **Live Micro-Sector & Predictive Lap Time Engine**: Real-time sector split analytics (S1, S2, S3) and predictive delta estimation built into `ac_core::analyzer` and UI tabs.
- **Crash Diagnostic Logging & Panic Hook**: Added custom `std::panic::set_hook` diagnostic logger that captures unhandled exceptions and exports detailed crash trace dumps (`crash_report_<timestamp>.log`) to the logs directory.
- **External JSON Localization System**: Moved all UI translations to external `data/locales/en.json` and `data/locales/ru.json` files with embedded compile-time fallbacks.
- **Linux Bash Build Script (`build_release.sh`)**: Added executable Linux release packaging script for native Linux TUI binaries and Wine/Proton `shm-bridge.exe`.
- **Pixel-Perfect PNG Text Glyph Renderer**: Enhanced `tui_tester` tool with bitmap text glyph rendering for readable English PNG screenshots.

### 🛡️ Bug Fixes & Stability
- **Safe Lock Protection**: Converted `SafeLock` mutex primitives to handle poisoned locks without crashing or panicking.
- **Cross-Platform Compatibility**: Gated Win32 file mapping APIs cleanly under Linux target stubs so `cargo check --workspace` passes without errors on all platforms.
- **Clippy Cleanliness**: Resolved all Clippy warnings and enforced strict workspace linting rules (`unwrap_used = "deny"`, `panic = "deny"`).

---

## [v0.2.2] - 2026-07-30

### 🌟 Features
- Added ratatui TUI dashboard, telemetry analyzer, setup manager, and overlay manager.
- Added cross-platform shared memory reader for Assetto Corsa.
