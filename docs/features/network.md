[← What the core does](README.md) · [Main README](../../README.md)

# The network

`broadcast/`, `lan.rs`, `net.rs` — and [`../lan.md`](../lan.md) for the details.

- **The frame over UDP** (`broadcast::udp`): a panel's worth of finished numbers
  and sentences, about a kilobyte, for a second screen, a relay or a script of
  your own — [the format](../guide/udp-feed.md).
- **The whole session** (`broadcast::session`): every reading, for another copy
  of the program, which runs its own analysis — so a watcher gets every screen,
  in their own units and language.
- **Finding each other** (`broadcast::discovery`): multicast to find copies on
  the network, never to stream.
- **A sink that fails never slows the tick**: a slow or vanished receiver is
  dropped, not waited for.
