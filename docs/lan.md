# Sharing a session, and watching one

Two people, two machines, one network. One drives; the other sees the
dashboard, the traces, the map, the corner table and the engineer — of the
driving happening on the other machine, in their own units and their own
language.

It works between **any two** of the three: the terminal (`ac_pro_engineer`),
the window (Pro Engineer), and either of them watching the other. They speak
one protocol as of v0.4.5.

## The short version

**Driving, and want to be watched.** Open `LAN` — `0` in the terminal, the LAN
button in the window. Press `S`. That is sharing on: you are announced on the
network, you are listening, and the rate and the port are chosen for you. The
only thing left is *who to send to*, and it is the line the screen says is
missing.

**Watching somebody.** Open `LAN`. Press `W`. Nothing else is asked. When the
driver's copy appears in `ON THIS NETWORK`, that is them; your screens fill as
soon as they start sending to you.

**Which of you picks whom does not matter.** Either end can aim at the other:
put the cursor on a name and press `Enter`, and that machine becomes the one
your session is sent to. Two people trying it out usually both press `S` and
`W` and pick each other, which is `BOTH` — you send yours and watch theirs.

**Off.** `O`. Every address and name is kept, so switching back on tomorrow is
one key rather than one form.

## What each key does

| Key | What |
|---|---|
| `0` | the LAN screen |
| `S` | share this session, or stop |
| `W` | watch, or stop |
| `↑` `↓` | move through the list of machines |
| `Enter` | send my session to the one the cursor is on |
| `O` | everything off, keeping the addresses |
| `A` | be findable on the network, or stop being |

They are bindings like every other key in this program: `config.json`, under
`keys`, and the Settings screen lists them.

## What travels

The **reading** — everything one tick of the simulator says, about two
kilobytes, thirty times a second. The watching machine runs its own analysis
on it: it finds the laps, builds the traces, draws the map's line, and its own
engineer writes the advice. That is why a watcher gets every screen rather
than a summary, and why the advice is in the watcher's language.

An **announcement** is separate and much smaller: a name, whether you are
driving or watching, the port you listen on, the car, the track and the
release. Nothing about the driving is in it — a machine with no business here
can read every packet on the group and learn a name and a port. Turning `A`
off stops sending it; it does not stop you *seeing* anybody, because listening
says nothing.

Two numbers, if you need them:

* **9001** — where a session arrives. One machine owns it.
* **9002** — the group everybody joins to find each other: `239.255.42.99`,
  which routers do not forward off the local network.

## When it does not work

Run the probe. It is the same four things the program does, printed:

```bash
cargo run -p ac_core --example lan_probe
```

and on the other machine, aimed back at the first one:

```bash
cargo run -p ac_core --example lan_probe -- send 192.168.1.42:9001
```

It says this machine's address, whether the port could be listened on, whether
the group could be joined, who was found, and whether anything arrived. A real
run between two processes looks like this:

```text
── this machine ─────────────────────────────────────────
  192.168.1.167  — this is what a friend on the LAN sends to

── the port a session arrives on ────────────────────────
  listening on 0.0.0.0:9001

── the group copies find each other on ──────────────────
  joined — announcing as "lan_probe" every two seconds

── twenty seconds ──────────────────────────────────────
  1 copies on this network:
    lan_probe        not listening          driving   probe 0.4.5
  a session is arriving from lan_probe
    300 readings, 29 a second, 0 ms old, 0 lost — 122 km/h
```

The three things that actually go wrong:

* **"cannot listen: address already in use."** The program is already running
  and holding the port. Watch from the program, not from the probe.
* **The list stays empty.** The network is not carrying multicast — a guest
  network, a VPN, some office switches. Everything still works: type the other
  machine's address into `sending to` by hand. The probe prints the address to
  type.
* **A name appears but says `not listening`.** They have not pressed `W`, so
  there is nowhere to send. That is a state, not a fault, and the list says so
  rather than offering a link that would carry nothing.
* **"that copy is older than v0.4.5 — both sides need updating."** A window
  from v0.4.2 speaks a shape this cannot read. It is recognised by name so the
  screen can say so; there is no half-working mode, and both machines have to
  be on 0.4.5.

## Watching somebody in another city

Everything above is one network. Two houses behind two routers cannot dial
each other — that is NAT, not a limitation of this program — so the usual
answers are a forwarded port or a relay somebody has to run. There is a third,
and it needs neither.

**A mesh VPN puts both machines on one private network wherever they are.**
[Tailscale](https://tailscale.com) and [ZeroTier](https://www.zerotier.com)
are both free for a handful of machines, and both give every computer a
permanent address it keeps when it moves.

1. Install it on both machines and sign both into the same network. Tailscale
   calls it a tailnet; ZeroTier calls it a network you join by its id.
2. Read the address it gives each machine — Tailscale's are `100.x.y.z`,
   ZeroTier's are usually `10.x.y.z` or `192.168.x.y` on the range you chose.
3. **The watcher** presses `W`, and leaves the listening address as
   `0.0.0.0:9001`. That means every interface this machine has, which includes
   the mesh one. There is nothing else to set.
4. **The driver** presses `S`, and types the watcher's mesh address into
   `sending to` — `100.64.0.2:9001`. It will not appear in the list, and that
   is expected: see below.

Nothing else about the program changes, and there is no server of ours in the
middle because there is no server at all.

### The list stays empty, and that is not a fault

Discovery is multicast, and **a mesh VPN does not carry multicast**. Neither
does most guest Wi-Fi, and neither do some office switches. So across a mesh
the two copies never see each other in `ON THIS NETWORK`, and the address has
to be typed — which is the case that box has always existed for.

Everything else works exactly as it does on a LAN. Announcing is only how you
are *found*; it has nothing to do with how a session travels.

### What it costs

A full reading is **1917 bytes** — measured, and a test keeps it that way —
and they go thirty times a second: **roughly half a megabit per second**, one
way, per watcher. That is less than a video call and far less than streaming
your screen, which is the alternative people reach for and which sends a
picture of your telemetry rather than the telemetry.

One thing to know about that number: it is larger than a single Ethernet
frame, so IP splits each reading into two and losing either half discards the
whole one. On a switched LAN that is nothing. Over a mesh VPN, with real loss
on the path, it is why the `lost` count on the LAN screen is worth a glance and
why the rate is a setting.

If the link is poor, `Settings → SHARING` lowers the rate. Ten a second is
still a map drawn from a sample every five metres at racing speed. The LAN
screen reports what is actually arriving — the rate, how old the picture is,
the worst it has been, and how many readings never came — so the answer to "is
it the link" is on the screen rather than a guess.

### Check it before anybody is waiting

The probe takes an address, so it tests the real path:

```bash
# on the watching machine
cargo run -p ac_core --example lan_probe
```

```bash
# on the driving machine, aimed at the watcher's mesh address
cargo run -p ac_core --example lan_probe -- send 100.64.0.2:9001
```

If readings arrive there and not in the program, the difference is a setting.
If they do not arrive at all, it is the mesh — check both machines are up in
its own admin page, and that the watcher's firewall lets UDP 9001 in on the
mesh interface.

## Two copies on one machine

Supported on purpose, and the way to try this before trusting it at a LAN
party: the group is joined on loopback as well as on the network, so a second
copy on the same desk finds the first. Give the second one a different
listening port if you want both to receive.

## What a watcher never does

Write a record. Somebody else's lap is not your personal best, and the file
that outlives the session is the one place that mistake would be permanent —
the same rule the demo obeys.
