[← What the core does](README.md) · [Main README](../../README.md)

# Games

`games/assetto_corsa/`, `games/assetto_corsa_competizione/`, `games/registry.rs`.

One folder per game, each a `Source` that reads the game's shared memory into
the same `Reading`, and says honestly what it can report in `Capabilities` —
**"not measured" and "measured as zero" are different answers.**

| | Assetto Corsa | Competizione |
|---|---|---|
| Tyre temperatures across the tread | yes | no (core only) |
| Tyre wear | yes | no |
| Brake pad wear | no | yes |
| Ride height | yes | no |
| Wind | yes | no |
| Track grip as a number | yes | no (named states only) |
| Sector times | yes | yes |

`cargo run -p ac_core --example capability_matrix` prints the full table from
the registry. Which game is read is a choice on the launcher (`config.game`),
never a guess: both publish under the same shared-memory names.

On Linux the game runs under Proton and the bridge (`wineshm.exe`) mirrors its
shared memory into `/dev/shm` — see the [guide](../guide/linux.md).
