[← Guide](README.md) · [Main README](../../README.md)

# Sharing a session

Two people, two machines, one network. One drives; the other sees the
dashboard, the traces, the map, the corner table and the engineer — of the
driving happening on the other machine, in their own units and their own
language. It works between **any two** of the front ends: the terminal
watching the window, the window watching the terminal, or two of either.

**Driving, and want to be watched:** open `LAN` (`0`), press `S`. That is
sharing on — you are announced on the network, you are listening, and the port
and the rate are chosen for you. The only thing left is who to send to.

**Watching somebody:** open `LAN`, press `W`. Nothing else is asked. When the
driver appears under `ON THIS NETWORK`, put the cursor on them and press
`Enter` if you also want to send them yours.

**Off:** `O`. Every address and name is kept.

What travels is the **reading** — everything one tick of the simulator says,
about two kilobytes, thirty times a second — so the watching machine finds the
laps, builds the traces and draws the map's line itself, and its own engineer
writes the advice. A separate, much smaller announcement carries a name, a
role, a port, the car and the track, and **nothing about the driving**;
switching `A` off stops sending it without stopping you seeing anybody.

Two ports: **9001** for a session, **9002** for the group copies find each
other on (`239.255.42.99`, which routers do not forward off the local
network). Both are settable; neither has to be.

When it does not work, `cargo run -p ac_core --example lan_probe` prints which
of the four halves is at fault — this machine's address, the port, the group,
and whether anything is arriving. [docs/lan.md](../../docs/lan.md) is the whole of
it, including what to do on a network that blocks multicast.

---
