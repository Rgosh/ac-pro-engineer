[← What the core does](README.md) · [Main README](../../README.md)

# For the launcher

Pro Launcher computes nothing about driving; what it needs is here, in
`games/assetto_corsa/`.

| Module | What it does |
|---|---|
| `content.rs`, `tracks.rs`, `ui_car.rs` | every car, circuit and layout installed, their specs and pictures; class and tags edited in a batch with the originals kept |
| `adding.rs`, `pack.rs`, `packs.rs`, `duplicates.rs`, `health.rs` | installing a mod from anything dropped on the window, packing one to share, copies found, broken mods checked |
| `physics.rs`, `car_data.rs` | a car's physics read from `data/` or `data.acd`, its curves, gearing and top speeds; a tune applied in place and put back exactly |
| `career.rs`, `special_events.rs` | Kunos's career and special events, the `race.ini` that starts one, and the cup or place a finished one earned — written into the game's own career file |
| `balance.rs` | a fair grid: ballast for every AI car to your car's kilograms per horsepower |
| `championship.rs`, `results.rs` | championships of your own, the game's results read, records, achievements and quests |
| `lobby.rs`, `server_details.rs`, `server_slots.rs`, `join_link.rs`, `local_servers.rs` | the server list, a server's places and drivers, invite links, servers of your own |
| `media.rs` | screenshots and replays, and the `race.ini` that opens one |
| `skin_info.rs` | a livery's driver, team, number and country |
| `csp*.rs`, `mod_config.rs`, `weather_fx.rs`, `real_weather.rs` | Custom Shaders Patch: installed, updated, every setting and preset; real weather |
| `levels.rs`, `bindings.rs`, `pedals.rs`, `assists.rs`, `video.rs` | the game's own settings, controls, pedals, assists, video |
| `backup.rs`, `preflight.rs`, `game_log.rs`, `robustness.rs` | backups of what is changed, checks before starting, the game's log read when it stops |

The model files themselves (`.kn5`) are read by the `kn5` crate beside the core:
the outline for maps and, in full, the scene the launcher's showroom draws.
