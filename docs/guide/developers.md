[← Guide](README.md) · [Main README](../../README.md)

# For developers

### The shape of it

```
core/          ac_core — telemetry, analysis, the engineer, the overlay frame
tui/           ac_tui  — the terminal application, the key map, every screen
wineshm (its own repository)    the Windows binary that bridges /dev/shm on Linux
apps/lua/      the CSP panel, and two harnesses that run it without the game
tests_suite/   integration tests over the whole pipeline
```

The desktop application computes everything and publishes a 712-byte
`#[repr(C)]` `OverlayFrame` once per tick. The panel reads fields and calls
ImGui. **Three artefacts encode that struct** — the application, `wineshm.exe`
and `assets/frontends/csp-panel/frame_layout.lua` — and changing it means changing
all three. `core/src/overlay/frame.rs` has the rules; `CLAUDE.md` has the
procedure and the traps.

The panel itself is a tree:

```
assets/frontends/csp-panel/
  ac_pro_engineer.lua      the entry point CSP loads, and nothing else
  frame_layout.lua         GENERATED from the Rust struct
  manifest.ini             the windows CSP opens
  acpe/settings.lua        what the driver chose, and making it stick
  acpe/i18n.lua            the panel's own words, in two languages
  acpe/theme.lua           colours, accents, the editable palette
  acpe/layout.lua          text sizes, spacing, the measured window
  acpe/format.lua          numbers into strings, once per settled frame
  acpe/frame.lua           the shared block and the snapshot drawn from it
  acpe/blocks.lua          one function per thing on screen
  acpe/controls.lua        the widgets the settings window is built from
  acpe/console.lua         typed commands, for what has no widget
  acpe/windows/*.lua       one file per window
```

### Checks, cheapest first

```bash
luajit apps/lua/tests/run_overlay.lua
```

Drives every window the panel exposes under real LuaJIT with CSP stubbed, and
insists the telemetry reached the screen, the settings reached storage, they
survived a reload, and a frame with no car is distinguished from a missing
application.

```bash
love apps/lua/love --test --settings
```

The panel under LÖVE for 120 frames, non-zero on a throw. `--shot name.png` takes
a picture.

```bash
cargo run --bin simulator     # in one terminal
cargo run -p ac_core --example engineer_probe
```

Fake telemetry, then the engineer's advice printed next to the numbers that
produced it.

```bash
cargo test --workspace
```

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

```bash
cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings
```

Both targets, before pushing. `CLAUDE.md` and `AGENTS.md` describe the working
rules, including the ones learned the hard way.

### Contributing

Issues and pull requests are welcome. Conventional Commits, and a commit body
that says what was wrong, why it mattered and how it was verified. If you change
the overlay frame, regenerate the layout and rebuild the bridge — there are tests
that will fail if you do not, and they exist because forgetting has cost
evenings.

Commits need a `Signed-off-by` line — `git commit -s`. [CONTRIBUTING.md](../../CONTRIBUTING.md)
has the certificate it stands for, and the one thing asked beyond it: that a
contribution may also be licensed commercially, which is what keeps the offer
below possible.

---
