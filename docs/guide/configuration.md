[← Guide](README.md) · [Main README](../../README.md)

# Configuration file

`config.json`, written as you change things and on exit.

| Platform | Path |
|---|---|
| Linux | `~/.config/raceengineer/config.json` |
| Windows | `%APPDATA%\RaceEngineer\RaceEngineer\config\config.json` |

Delete it to start from defaults; the application writes a fresh one. Keys it
does not recognise are ignored, and keys that are missing take their default, so
a config from an older version keeps working.

| Key | Default | What it is |
|---|---|---|
| `language` | `"English"` | `"English"` or `"Russian"` |
| `update_rate` | `16` | Milliseconds between telemetry ticks. Lower is smoother and costs more CPU. |
| `history_size` | `300` | Points kept for the graphs. |
| `auto_save` | `true` | Write settings on exit as well as on change. |
| `pressure_unit` | `"Psi"` | `"Psi"`, `"Bar"` or `"Kpa"` |
| `temp_unit` | `"Celsius"` | `"Celsius"` or `"Fahrenheit"` |
| `shift_point_offset` | `200` | RPM before the limiter that the shift light comes on. |
| `fuel_safety_margin` | `1.0` | Litres kept back in the strategy calculation. |
| `target_tyre_pressure` | `27.5` | The pressure the engineer measures against. |
| `target_hot_pressure_front` / `_rear` | `27.5` / `27.0` | Published to the overlay, which shows your distance from them. |
| `show_ghost_delta` | `true` | Measure the delta against your own best lap rather than AC's meter. |
| `alerts.tyre_pressure_min` / `_max` | `26.0` / `28.5` | Outside this is worth saying. |
| `alerts.tyre_temp_min` / `_max` | `70` / `105` | Cold and overheating, in °C. |
| `alerts.brake_temp_max` | `800` | Above this the brakes are cooking. |
| `alerts.fuel_warning_laps` | `3.0` | Laps of fuel left that triggers the warning. |
| `alerts.wear_warning` | `96` | Tyre life below which it is a warning, as a percentage. |
| `alerts.wear_critical` | `85` | And below which it is critical. |
| `overlay.show_telemetry` / `_engineer` / `_session` / `_timing` / `_fuel` | `true` | Which blocks the overlay is allowed to draw. |
| `overlay.engineer_lines` | `4` | How many advice lines reach the overlay, 0 to 8. |
| `overlay.debrief_lines` | `8` | Lines of each finished lap's debrief that reach the overlay, 0 to 8. Zero stops publishing one. |
| `overlay.broadcast_to` | `""` | Where a session is sent, `host:port`. Empty is off. Set from the LAN tab. |
| `overlay.broadcast_hz` | `10` | How many times a second the *summary* frame is sent — the one for the panel and for anything outside this project. A whole session goes at `lan.share_hz`. |
| `overlay.broadcast_name` | `""` | The name that travels with it, so somebody watching several drivers can tell them apart. |
| `overlay.receive_from` | `""` | Where a session is listened for, `ip:port`. `0.0.0.0:9001` is the whole network, `127.0.0.1:9001` only this machine. |
| `lan.mode` | `"Off"` | `Off`, `Share`, `OnAir`, `Watch` or `Both`. The LAN tab writes it. |
| `lan.share_hz` | `30` | Readings a second sent to another copy of this program. Their map and traces are drawn from these, so it is how finely their picture is drawn. |
| `lan.only_on_track` | `false` | Only send while the car is on track, not in the menus. |
| `lan.quiet_after_s` | `3` | How long a link may be silent before the screen says so. |
| `lan.announce` | `true` | Say on the network that this copy is here, so others can find it without an address being read out. |
| `overlay.startup_card` | `true` | Show the install card when the application starts. |
| `keys.*` | see [Keyboard](keyboard.md#keyboard) | One key per action, as text. |
| `data_path` | config directory | Where laps, exports, screenshots and records go. |
| `ac_install_path` | `""` | Force the Assetto Corsa folder. Empty means auto-detect. |
| `ac_documents_path` | `""` | Force the Documents folder AC reads setups from. Under Proton this is inside the prefix. |

The panel's own settings are **not** here — CSP keeps them in its own storage, so
uninstalling and reinstalling the overlay does not lose them.

---
