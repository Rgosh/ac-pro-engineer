[← Guide](README.md) · [Main README](../../README.md)

# Command line

### `ac_pro_engineer` — the application

```
ac_pro_engineer [OPTIONS]
```

| Flag | What it does |
|---|---|
| `-d`, `--demo` | Run against a built-in simulated session. No game needed — this is the fastest way to see what the application looks like with data in it. |
| `--export-overlay <DIR>` | Write the in-game Lua panel into `<DIR>/ac_pro_engineer` and exit. For a game folder the application may not write to, an install it cannot find, or a second copy of AC. What lands is exactly the panel this build's frame is shaped for. |
| `-l`, `--log-level <LEVEL>` | `trace`, `debug`, `info` (default), `warn`, `error`. `debug` adds the telemetry loop and the overlay writer; `trace` adds every shared-memory read. |
| `--log <FILE>` | Write the log here instead of under the config directory. |
| `-s`, `--silent` | Do not write a log at all. |
| `-h`, `--help` | Full help, with the long explanation of each flag. |
| `-V`, `--version` | Print the version and exit. |

Environment variables the Linux build reads:

| Variable | Effect |
|---|---|
| `AC_PROTON_PATH` | The launcher used to start `wineshm.exe`. Defaults to `protontricks-launch`. |
| `AC_TEST_MODE` | Pretend to start the bridge without a Proton prefix. For development. |

### `wineshm.exe` — the Linux bridge

Runs **inside** the Proton prefix.

```bash
protontricks-launch --appid 244210 wineshm.exe
```

| Flag | What it does |
|---|---|
| *(none)* | Create the mappings and stay running. Type `exit` to stop it cleanly. |
| `--verify` | Open the overlay mapping the way CSP does, print what is in it, and exit. The one check that can only be made from inside the prefix. |
| `--help`, `--version` | As usual. |

### Development binaries and examples

| Command | What it does |
|---|---|
| `cargo run --bin simulator` | Write plausible AC telemetry into shared memory, so the whole application can be exercised with no game. |
| `cargo run --bin tui_tester` | Render every terminal screen to `screenshots/` as PNG. |
| `apps/lua/love/portraits.sh` | Render every *overlay* window and settings tab to `screenshots/` as PNG. |
| `cargo run -p ac_core --example bridge_probe` | Which bridge is on disk, which is running, and whether the overlay can work. |
| `cargo run -p ac_core --example engineer_probe [samples]` | The engineer's advice printed next to the telemetry that produced it. |
| `cargo run -p ac_core --example gen_lua_layout` | Regenerate the panel's `frame_layout.lua` from the Rust struct. |
| `cargo run -p ac_core --example publish_demo_frame` | Publish one known overlay frame, for the Lua conformance check. |
| `luajit apps/lua/tests/run_overlay.lua` | Drive the whole panel under LuaJIT with CSP stubbed. `ACPE_ALL=1` prints every string it drew. |
| `love apps/lua/love` | The panel running under LÖVE, with sliders for every field. `--test` runs it headless; `--shot NAME.png` saves a picture. |

---
