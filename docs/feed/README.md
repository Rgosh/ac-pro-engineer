# The feed — Pro Engineer's numbers in your own program

Pro Engineer reads the game, works out what the car is doing and what to do
about it, and **hands the result to anything that asks**. A dashboard on a
second screen, a stream overlay, a button box, a phone, a spreadsheet: ask,
and you get the same finished numbers and the same engineer's sentences the
in-game panel draws, as JSON over UDP, without reading a byte of the game's
memory yourself and without anybody editing a config file.

| | |
|---|---|
| Port | **9003**, UDP, on the machine running Pro Engineer |
| Ask | `{"magic":"acpe","subscribe":{"hz":20,"name":"my dashboard"}}` |
| Answer | `{"magic":"acpe","subscribed":true,"schema":1,"hz":20,"lease_s":10}` |
| Then | one JSON message per datagram, at the rate you asked — [schema.json](schema.json) |
| Keep it | ask again every few seconds; stop asking and it stops in 10 s |
| Leave | `{"magic":"acpe","unsubscribe":true}` |

## Thirty seconds to the first number

Ready-made readers, each one file and no dependencies:

| Language | File | Run |
|---|---|---|
| Python | [feed.py](feed.py) | `python3 feed.py` |
| JavaScript (Node) | [feed.mjs](feed.mjs) | `node feed.mjs` |
| Rust | `ac_core::broadcast::subscribe::Client` | `cargo run -p ac_core --example feed` |
| Shell | — | `echo 'acpe subscribe 5' \| nc -u -w 3 127.0.0.1 9003` |

No game to hand? `cargo run -p ac_core --example feed_demo` answers on the
same port with a made-up car lapping an imaginary circuit, so a dashboard can
be written on a train. Its driver is called `DEMO — MADE-UP DATA`, so a
screenshot of it cannot be taken for a measurement.

## What arrives

The **computed frame**, not raw telemetry: speed, gear, revs, fuel and laps of
fuel left, the delta, lap and sector times, four corners of pressures,
temperatures across the tread, wear and laps of life left, brake temperatures,
and the engineer's advice — already in the driver's language and units, most
important first, in the order to act on it. After each lap, the last laps'
debrief.

Every key is in [schema.json](schema.json), with its unit. A test in the core
walks a full message against that file both ways, so a key on the wire that
the schema does not list — or one it promises that never comes — fails the
build. Generate your types from it.

Rules a reader should keep:

- **Ignore keys you do not know.** New ones may appear; `schema` changes only
  when a key changes meaning or goes.
- **Drop a message whose `sequence` is smaller than the last.** UDP does not
  keep order.
- **Check `flags & 2` (connected).** Clear means the program is running and
  the car is not — the pit menu, a loading screen — and the numbers are held,
  not live.
- **A zero inner or outer temperature is "not published"** on a game that
  does not report it, not a cold tyre.

## Who may ask

- **This machine: always.** On by default (`overlay.subscriptions`), because
  nothing leaves the computer.
- **The local network: only if the driver turned it on**
  (`overlay.subscriptions_from_lan`). Telemetry about a person crosses a wire
  because they said so.
  While it is off the program listens on this machine's loopback only, so
  the network cannot reach the port at all.
- **The internet: never.** A public address is not answered at all — a
  reply to a forged sender is traffic aimed at somebody else.
- At most 16 subscribers at once (the seventeenth is told so); each gets its
  own rate, up to 60 a second.

## Which feed answers which question

| You want | Use |
|---|---|
| Numbers and advice for a screen of your own | **this feed** |
| Every sample — throttle, coordinates, the map — to analyse yourself | `broadcast::session`: the whole `Reading`, as another copy of Pro Engineer receives it |
| To send the frame to one fixed address, a relay | `overlay.broadcast_to` — the same JSON, pushed |

A subscriber is a sink like any other: if it stops reading, the program does
not wait for it, and the driver's own panel never notices.
