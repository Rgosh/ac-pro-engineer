[← Guide](README.md) · [Main README](../../README.md)

# The UDP feed — writing your own front end

The application computes everything and publishes it. The in-game panel reads a
shared-memory mapping, which is the right transport inside the game and no use
outside it: it needs a bridge under Proton and it cannot cross a machine. So
everything else reads UDP.

**Ask for it, and it comes.** While Pro Engineer runs it answers on UDP port
9003; a program sends one line and gets the feed at the rate it wanted, for as
long as it keeps asking — nothing to configure on either side:

```json
{"magic":"acpe","subscribe":{"hz":20,"name":"my dashboard"}}
```

Ready-made readers in Python, JavaScript and Rust, the JSON schema of every
key, and a demo server that sends a made-up car for writing a dashboard with
no game running are in [`docs/feed/`](../../docs/feed/README.md):

```bash
python3 docs/feed/feed.py
```

This machine may always ask, because nothing leaves it. Another machine on
the network only when `overlay.subscriptions_from_lan` is on, and the internet
never.

Or push it to one fixed address, for a relay:

```json
"overlay": { "broadcast_to": "127.0.0.1:9001", "broadcast_hz": 10 }
```

Off unless you set it. This is telemetry about you, and it leaves the machine
because you said so.

One JSON object per datagram, ten a second by default, carrying **the computed
frame** — not raw telemetry. Speed, gear, four corners, lap times, the
engineer's advice and the last three laps of debrief, already analysed. A
receiver draws and needs to know nothing about which simulator produced it.

```json
{
  "magic": "acpe", "schema": 1, "app_version": "0.3.6",
  "game": "assetto_corsa", "driver": "", "sequence": 12043,
  "speed_kmh": 214.0, "gear": 5, "rpm": 7400, "max_rpm": 8500,
  "fuel_litres": 41.2, "fuel_laps_remaining": 13.3, "delta_seconds": -0.284,
  "lap_count": 7, "best_lap_ms": 91380, "last_lap_ms": 92450, "stint_laps": 7,
  "corners": [ { "pressure_psi": 26.8, "temp_c": 88.0, "temp_inner_c": 92.0,
                 "temp_outer_c": 84.0, "wear_percent": 98.0,
                 "brake_temp_c": 420.0, "laps_remaining": 10.5 } ],
  "advice": [ { "severity": 1, "text": "Fronts over 28.4 psi (target 27.5)" } ],
  "debrief": [ { "lap_number": 12, "lap_time_ms": 91234,
                 "sectors_ms": [28540, 31120, 31574],
                 "lines": [ { "severity": 1, "text": "…" } ] } ]
}
```

`magic` is always `"acpe"`, so a receiver on a shared port can tell these from
somebody else's datagrams. `schema` changes when a key changes meaning or
disappears — not when the panel's own wire format moves, which is a different
number and none of your business. `severity` is 0 info, 1 warning, 2 critical.

Reading it is about fifteen lines:

```python
import json, socket
sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
sock.bind(("127.0.0.1", 9001))
while True:
    frame = json.loads(sock.recv(65535))
    print(frame["speed_kmh"], [line["text"] for line in frame["advice"]])
```

UDP because a lost frame costs nothing — another arrives in a tenth of a second
— and because a subscriber that stops reading must not be able to stall the loop
feeding the driver's own overlay.

### Watching someone else drive, on the same network

The other end of the feed now exists. The driver sets `broadcast_to`; the
viewer sets `receive_from` and gets the driver's telemetry and the driver's
engineer in their own panel.

```json
"overlay": { "broadcast_to": "192.168.1.42:9001" }
```

```json
"overlay": { "receive_from": "0.0.0.0:9001" }
```

What crosses is the **finished frame**, so the viewer sees exactly what the
driver's engineer is saying — which is the point when the reason you are
watching is to help the person driving. The cost is that the sentences were
written on the other machine, so the viewer's own language and units do not
apply to them. Which blocks to draw stays the viewer's choice: one person's
panel settings never reach another's screen.

While a remote frame is arriving it **takes the panel over** and the flag that
says so is set, so a lap counter about a car you are not sitting in cannot be
mistaken for your own telemetry gone wrong.

**Limits, plainly.** This is one network — a flat, a LAN party, a machine in the
next room. Two houses behind two routers is NAT, and that needs a forwarded port
or a relay in the middle, which is infrastructure rather than code and is not
built. There is no discovery, no relay and no championship mode: the viewer
types an address. `docs/ARCHITECTURE.md` is where the rest is meant to go.
