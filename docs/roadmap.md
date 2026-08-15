# Roadmap — what to do after v0.3.6, in order

Written 2026-08-14, after the licence change and the site rebuild, out of the
question "does online sharing make sense before ACC?". It is a working list to
go through one item at a time, not a promise and not a release plan. Nothing
here has a date on it.

**The order follows one rule:** finish what is started, then remove what blocks
everything else, then add. Two items near the middle — §4 and §5 — are not
features and will not show up in a changelog anybody reads, and they are the
reason the two after them are possible at all.

Status of each item is one of: **owed** (agreed, not done), **open** (needs a
decision first), **deferred** (deliberately not now, with the reason).

---

## 1. Finish v0.3.7 — owed

`docs/plan-0.3.7-analysis.md` has items 1–7 built. What is actually left is
smaller than the plan makes it look, and it is one thing.

### 1.1 `Chain` on the rest of the rules — **DONE**, `d28eace`, 2026-08-14

Thirteen rules gained a cause, a measurement and a check. The two fuel rules
kept `None`, with the reason written above them. A source-level test now fails
on any rule added later that ships mute, and the bottoming and wear rules were
sharpened on the way. The item as it was written follows.

**What.** `Recommendation` carries `Chain { cause, effect, confirm, evidence }`.
**One rule fills it in** — camber. The other twenty-odd still carry the old
hand-picked score and fall back to it.

**Why it matters.** `confirm` is the field that makes advice checkable: what to
look at next run to know whether the change worked. A driver told "the ARB
gained you 0.2 s" with nothing to verify against has been misled by their own
tooling. Twenty rules that cannot say it are twenty rules the confidence model
does not really reach.

**Where.** `core/src/engineer.rs`, `core/src/confidence.rs`.

**Done when.** Every rule that can state a cause states one; the ones that
genuinely cannot say so rather than inventing a `confirm`. `engineer_probe`
prints a chain beside each line.

**Shape of the work.** A rule at a time, not a redesign. Can be split across
several sittings and stopped at any point without leaving anything broken.

### 1.2 Setup attribution — leave cut

**Decision: keep it cut.** Written, tested, removed before release. It rests on
knowing which setup is loaded and nothing publishes that: AC's shared memory
carries fuel, brake bias, pressures and camber, so two setups differing only in
a roll bar score identically — exactly the change the feature existed to
measure. Worse, fuel burns off during a stint, so a "quali" file starts matching
halfway through a run and invents a change that never happened.

It comes back when the setup can be **identified** rather than guessed, which
probably means reading the file AC loaded rather than matching telemetry against
a folder of candidates. Not scheduled.

### 1.3 Remote reference laps — leave deferred

**Decision: keep deferred.** Item 8 of the plan. The local half is what CORNERS
does. The remote half needs the Setup Cloud to carry *laps* rather than `.ini`
files, and a lap is orders of magnitude larger than a setup — infrastructure,
not analysis. It should follow a release of people actually using the local
comparison, and that release has not happened yet.

---

## 2. Licensing leftovers — owed, small

What is left after the AGPL change. Neither is code.

Contributor consent is **not** on this list. Maksym agreed on PR #10, in writing
and from his own account, to relicense all past and future contributions under
AGPLv3, and the commercial half is settled with him directly. Everyone after him
signs off against `CONTRIBUTING.md`, which asks for both at once.

### 2.1 A dependency audit before the first sale — **DONE**, `e7d15ff`, 2026-08-14

`deny.toml` plus a `dependency licences` job in CI. It caught two things about
this repository on its first run — `ac_core`/`ac_tui` rejected for being AGPL,
and `tests_suite` carrying no licence at all — and was verified by removing the
MPL exception and watching it fail. The item as written follows.

**Run once on 2026-08-14, off `cargo metadata`, and it comes back clean.** 328
third-party crates; every one permissive — MIT, Apache-2.0, BSD, ISC, Zlib,
Unicode-3.0, CC0, BSL-1.0 — with exactly one exception:

    webpki-roots 0.25.4 — MPL-2.0

That is Mozilla's CA root store, pulled in by `reqwest`/`rustls`, and **it does
not block a commercial licence.** MPL-2.0 is copyleft per *file*: it may ship
inside a proprietary product provided its own files stay available, which for an
unmodified crate means pointing at upstream. Two consequences and no more:

- it belongs in `NOTICE` as a third-party component, named with its licence;
- **do not patch it in-tree.** A vendored, edited copy is the case where the
  obligation stops being a footnote.

**What is actually left**, and it is the point of the item: make this a
*standing* check rather than a fact that was true once. A dependency added in
six months is exactly how a copyleft crate gets in — nobody re-runs an audit
they remember passing.

**Where.** `cargo deny check licenses` in `.github/workflows/ci.yml`, with a
`deny.toml` that allows the permissive set above, names `MPL-2.0` as a
deliberate exception rather than a blanket allow, and denies GPL/LGPL/AGPL/SSPL
outright.

**Done when.** CI fails on a pull request that introduces a copyleft
dependency, and says which one.

### 2.2 The commercial licence text — deferred, on purpose

`LICENSING.md` currently says "no price list, describe what you are building,
rgoshbbb@gmail.com" and that is the whole offer. **Keep it that way** until
somebody actually writes. Terms drafted against a real case are better than
terms drafted against an imagined one, and the draft wants a lawyer's pass
before it is signed, not before it is needed.

What to have ready when that mail arrives: what is being built, how it is sold,
which parts of the code it uses, and whether they want updates. Everything else
follows from those four.

---

## 3. The Tailscale page — **DONE**, `e9bb985`, 2026-08-14

A section at `/technical/#watching`, a matching FAQ entry, and
`core/examples/share_probe.rs` — which was written first and immediately caught
that the engineer needs its one-second alert hold before it says anything, so
the first run reported an empty frame. Three pages that said internet sharing
was impossible were corrected. The item as written follows.

**What.** A short section on `/technical/` and an FAQ entry: how to watch a
friend who is not on your network.

**Why now.** The interesting half already works, and it is worth being precise
about how much. `core/src/broadcast/udp.rs` puts into every message:

- `advice` — all eight slots, each with its **text and severity**. That is the
  engineer's own sentences, not numbers to re-interpret: *"Fronts over 28.4 psi
  (target 27.5)"*, *"T3 cost 0.34 s — 14 m late on the brakes"*.
- the debrief — three laps, eight lines each.
- the four wheels, the fuel, the lap timing.

and `core/src/broadcast/receiver.rs` rebuilds it into a frame, so **the watcher's
own panel draws the other driver's engineer.** The feature people ask for exists.
The only thing missing is crossing NAT, and Tailscale or ZeroTier solves that
today, for free, with no server: the two machines get addresses on one virtual
network and none of this code changes.

**One thing it does not carry**, and worth knowing before promising it: the
`corners` field in the message is the four *wheels*, not the track's corners.
A corner-by-corner loss reaches a spectator only when it is one of the eight
advice lines. The full CORNERS table is not broadcast. If watching somebody
else's lap analysis is the point, that is a frame change — a real one — and it
belongs in §9 rather than here.

**Why before any relay.** This is a page, not a feature. It costs an afternoon
and tells you something a guess cannot: **whether anybody actually wants it.**
If nobody uses it, §9 is answered.

**Done when.** The page exists, and one person other than you has done it.

---

## 4. A neutral reading — **DONE**, 2026-08-15

`games/reading.rs` holds a `Reading` in three parts — `Car`, `Session`, `Fixed`
— and `Source::poll` returns one instead of a boolean. The AC accessors are
gone, so there is no longer a way to read the game without going through the
trait, and `tui/src/lib.rs` holds a `Box<dyn Source>` rather than an
`AssettoCorsa`. All 109 references are moved; `grep` outside `games/` finds
nothing but the fake-telemetry simulator, which writes AC's pages and says so.

Four things worth recording, because three of them are the reason to do this
kind of change with the compiler rather than by eye:

- **Nothing changed on screen.** All 31 terminal screenshots are byte-identical
  to the pre-refactor build, checked by generating both sets. The four that
  differ from the *committed* PNGs differ on an unmodified tree too — they draw
  install paths and bridge status, which are properties of this machine.
- **`TelemetryPoint` kept its field names**, and the compiler is what noticed:
  a lap is serialised to disk, so `gas` and `rpms` are a file format rather
  than a naming choice.
- **The gear was the one real bug, and screenshots did not catch it.** AC counts
  reverse as 0 and neutral as 1. `Car::gear` is −1/0/1 and the AC folder
  translates once — but three screens decoded AC's numbering inline, and moving
  the convention left all three a gear out on live telemetry. The screenshots
  stayed byte-identical throughout, because the mock's literal `6` did not
  change with the meaning of the field. Found by reading the call sites, not by
  running anything. There is one `widgets::gear_label` now, with a test:
  a convention kept in three places is one that gets changed in two.
- **The boundary is a test now.** `tests_suite/src/boundary_tests.rs` fails on
  any file outside `games/` naming an AC struct, and on any accessor reappearing
  beside `poll`. Both were verified by breaking them. Without that, this item
  would be a thing that was true on one afternoon — which is exactly what the
  first version of the trait turned out to be.

The item as it was written follows.

**This is the one that unblocks everything else, and it adds no features.**

**Checked against the knowledge graph on 2026-08-14, and it is worse than this
item said.** `Source` is implemented once — `impl Source for AssettoCorsa` — and
**used as a trait nowhere**: no `dyn Source`, no generic bound, no parameter of
that type. `Source::poll` has no callers at all (the only `.poll()` in the tree
is `FrameReceiver::poll`, a different type), and `Source::capabilities` has
none either, confirmed both by `trace_path(direction="inbound")` returning zero
with tests included and by grep.

So the trait is not thin-but-load-bearing. It is **bypassed**: `tui/src/lib.rs`
holds a concrete `AssettoCorsa` (line 205), constructs it at 1475, and calls
`.physics()`, `.graphics()` and `.stat()` on it directly at 967, 975, 983, 1219
and 1483. That is the boundary, and it is a struct rather than an interface.

One correction worth keeping, because it is a lesson about the tool rather than
about the code: the graph reported *no callers* for those three accessors too,
which is wrong — they are reached through `self.mem.as_ref().map(|mem| …)` and
the indexer did not resolve the closure. `check_index_coverage` reported no
recorded issue for that file all the same. Coverage being clean is not proof of
completeness; the five call sites came from grep.

**What is wrong.** `Source` is `id()`, `capabilities()`, `poll()`. The telemetry
does not come out through it: it comes out through AC-specific accessors
`physics()`, `graphics()`, `stat()` returning `AcPhysics`, `AcGraphics`,
`AcStatic`. Outside `core/src/games/` there are **109 references to those three
types across 10 files**:

| File | refs |
|---|---|
| `core/src/engineer.rs` | 47 |
| `tui/src/lib.rs` | 23 |
| `core/src/analyzer.rs` | 11 |
| `core/src/session_info.rs` | 6 |
| `core/src/broadcast/mod.rs` | 1 |
| plus `tui/src/ui/tabs/{engineer,strategy,telemetry}.rs`, both test binaries | |

So the engineer, the analyser and five screens all read Assetto Corsa's memory
layout directly. The folder-per-game boundary is real as a *folder* and does not
carry data.

**What to do.** A reading type the trait returns — the shape the rest of the
program already works in — and move those 109 references onto it. The AC folder
converts its structs into it; nothing above learns a second game's layout.

**Why it cannot be skipped.** Adding ACC without this means either a second set
of conditionals through the middle of the engineer or a second engineer. The
module's own header already says it: *"the second game will find the wrong
assumptions in this trait, and it is far cheaper to fix them while there is one
consumer than five."*

**Done when.** `grep -r AcPhysics core/src tui/src` outside `games/` returns
nothing, and the whole suite is green on both targets with no behaviour change.
This is a large diff that should change no output at all — screenshots before
and after should be identical.

---

## 5. Capabilities that actually gate — **DONE**, `c3a3e52` / `5c513a2`, 2026-08-15

The flags travel on the `Reading`, the engineer is told them once a tick, and
three rules withhold: camber and tyre temperature without tread temperatures,
wear without wear. The camber *history* stops being collected too — withholding
only at the rule leaves a stint of zero-minus-zero averaged into
`camber_spread`, which reads as a perfectly cambered car. Two screens say what
they cannot say: the sector table and the setup list.

Three things worth keeping:

- **The default is nothing measured**, and that is the whole safety argument. A
  consumer never told what its game reports goes silent, which is loud. The
  permissive default fails one wrong verdict at a time, which is not.
- **It caught two probes immediately.** `engineer_probe` and `share_probe` read
  AC's pages and never said so, so their tyre advice vanished the moment the
  gate existed. That is the mechanism working on its first day.
  `assetto_corsa::CAPABILITIES` is where the answer lives now, once.
- **`AppState::capabilities` returns an `Option`.** `None` is "no game", not "a
  game that measures nothing" — a lap read from a file has its sectors whether
  or not a simulator is running, and the screens are tested for that too.

Found on the way, and fixed separately in `ce1458c`: `DiscordClient::new`
connected to Discord's IPC socket from `AppState::new`, so the test suite went
from 0.4 s to 374 s the moment Discord was opened on this machine — on an
unchanged tree. The remaining 58 s is the same mistake in `SetupManager`, whose
background thread opens with a blocking manifest fetch that its `Drop` waits
for. **Still owed.**

The item as it was written follows.

**What is wrong.** `capabilities()` is **called nowhere** outside `games/`. Not
once — confirmed by the graph (zero inbound callers, tests included) and by grep,
which is two independent methods agreeing. It is declared, documented, tested against a real capture — and nothing
consults it.

**Why it is harmless today and dangerous tomorrow.** AC can report all four, so
nothing has ever gone wrong. ACC does not publish inner/middle/outer tyre
temperatures — only core — and the camber rule is built *entirely* on
inner-minus-outer. It would read zeros and produce a confident verdict about a
car nobody drove. That is not a hypothetical: it is the exact class of bug this
project has already shipped, and the reason `Capabilities` was written.

**What to do.** Every rule that depends on a measurement asks whether the game
publishes it, and withholds rather than guesses. The terminal says "not measured
by this game" where it currently would print a number.

**Done when.** A test sets a capability to `false` and asserts the verdict
disappears — for each of the four, and for each new flag added in §6.

---

## 6. ACC: the folder

Only worth starting after §4 and §5. Everything here is additive once they are
done.

**6.1 The structs, from a real capture.** ACC uses the same page names
(`acpmf_physics` and the rest) with a different and longer layout. Take them
from bytes captured off a running game and pin the offsets at compile time, the
way `tests_suite/src/shm_layout_tests.rs` does for AC. Do not transcribe from
somebody's header file.

**6.2 A discriminator.** Same names, different sizes — so the wrong parser must
be unable to attach. Check the mapped size, and refuse rather than read garbage.

**6.3 New capability flags.** ACC publishes things AC does not (brake pad and
disc life, tyre set, rain tyres, MFD pressures, stint time remaining) and omits
things AC has. Each is a flag, and §5 is what makes the flags mean something.
**Verify every one against a capture** — "ACC does not publish X" is a claim,
not a fact, until the bytes say so.

**6.4 Paths and the appid.** Steam appid 805550, not 244210 — that is process
detection and the Proton bridge's launch line.

**Done when.** The terminal runs on ACC and every screen either shows a real
number or says the game does not publish it. No advice appears that rests on a
measurement ACC does not make.

---

## 7. ACC: setups — after 6, or never

ACC setups are JSON under
`Documents/Assetto Corsa Competizione/Setups/<car>/<track>/`, not AC's INI. The
setup browser, the comparer and the cold-pressure target reader all need a
second implementation.

Until then `setups: false` and the tab says so. This is exactly what §5 buys —
shipping ACC without setups becomes a truthful state rather than a broken
screen.

---

## 8. ACC: the overlay decision — open, decide before announcing ACC

**The unpleasant one.** All of `assets/frontends/csp-panel/` is a Custom Shaders
Patch Lua app. **ACC has no CSP and no Lua app API.** On ACC the product is the
terminal and the UDP feed, and nothing in the car.

Two options:

- **Accept it.** Say so on the ACC page and in the simulator picker, the same
  way the picker already says "not yet" instead of "coming soon".
- **Bring back a desktop-drawn overlay.** It existed until v0.3.5 and was
  removed for good reasons: it did not survive exclusive fullscreen, never
  appeared in VR, and was invisible to the game's own screenshots.

**Recommendation: accept it.** The removed overlay was removed on merit, and
reviving it to fill a gap is how a worse version of a solved problem comes back.

**Done when.** The decision is written down, and whichever way it goes the site
says it before anybody downloads expecting the other.

---

## 9. An online relay — deferred, and only on evidence

**Not before ACC.** The reasoning, in short:

- **What is missing is only NAT.** §3 solves it for free, today, with no server.
- **A relay is not a feature, it is an operation.** A machine to pay for, keep
  up and secure; rooms and the question of who may watch whom; abuse. One
  person's project acquires a service that can be down.
- **It contradicts the strongest thing the site says.** "No account, no cloud,
  nothing leaves your machine" is a column where the comparison table beats
  everyone. Spending it needs a better reason than novelty.
- **AGPL §13 applies to anything hosted**, so a fork can run its own relay. The
  relay is not a moat.
- **ACC fixes the weakest cell in that same table** — "supports exactly one
  simulator; the others cover four to seven" — which is a line written on the
  site by its own author.

**What is genuinely unclaimed.** Several tools already share telemetry over a
LAN or a typed address, and several share *laps* after a session. Nothing found
shares the **engineer's verdict** live — "T3 cost 0.34 s, 14 m late on the
brakes", with its confidence marker, on somebody else's screen. The
differentiator is not the transport, and **that part is already built** (§3):
the advice slots and the debrief travel in the UDP message today.

So the sequence is not "build sharing". It is: make the existing thing reachable
(§3), and only then decide whether it needs infrastructure.

**If it is ever built, the work is two things and neither is a server:**

- **Broadcast the corner-by-corner analysis.** Today only the eight advice lines
  cross the wire; the CORNERS table does not. Watching somebody's lap fall apart
  turn by turn needs those sections in the frame or in the message, which is a
  frame version bump and all three artefacts that encode it — see the frame
  contract in `CLAUDE.md`. Do this one on evidence too: it is the difference
  between "my mate can see my engineer" and "my mate can debrief me".
- **A pairing story.** Right now you type an address, which is the whole
  security model and is fine for one network. The moment it crosses the
  internet, "who is allowed to watch me" stops being answered by the router.

**Revisit when.** People who used §3 ask for it. Not before.

---

## The order, on one line

1 → 2 → 3 → 4 → 5 → 6 → 7 → 8, with 9 held until §3 produces evidence.

§2 and §3 can be done in any gap; they touch nothing else. §4 and §5 are the
spine and should not be split across a release boundary.
