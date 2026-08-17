#!/usr/bin/env bash
# Run one game's tests, and the game-neutral core beside them.
#
#   ./tools/test-game.sh acc      # Competizione, then the core
#   ./tools/test-game.sh ac       # Assetto Corsa, then the core
#   ./tools/test-game.sh core     # only the core — nothing that names a game
#   ./tools/test-game.sh all      # the whole workspace, the way CI runs it
#
# Why this exists: `cargo test --workspace` runs everything, and while working
# on one simulator most of that is noise you have already read. The two
# questions that actually matter while changing a game are **"does this game
# still parse and convert its own pages"** and **"is the core still what it
# was"** — and the second one is the important half, because a game folder is
# meant to be additive and a change there that moves a neutral number is the
# bug worth catching early.
#
# What it does *not* do is replace the full run. A game's own suite passing
# says nothing about the other game, the terminal, the translations or the
# boundary tests, so this prints the command to run before pushing and does not
# pretend to be it.
#
# The filters are module paths, and they stay right as long as a game keeps to
# the two places it lives: `core/src/games/<id>/` for the code and
# `tests_suite/src/<id>_tests.rs` for the tests that need its bytes. Adding a
# simulator is a `case` arm here with those two names in it.
set -euo pipefail

game="${1:-}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

case "$game" in
  ac | assetto_corsa)
    module="games::assetto_corsa::"
    layout="assetto_corsa_tests"
    name="Assetto Corsa"
    ;;
  acc | assetto_corsa_competizione)
    module="games::assetto_corsa_competizione::"
    layout="assetto_corsa_competizione_tests"
    name="Assetto Corsa Competizione"
    ;;
  core)
    module=""
    layout=""
    name=""
    ;;
  all)
    exec cargo test --workspace
    ;;
  *)
    echo "usage: ${BASH_SOURCE[0]##*/} <ac|acc|core|all>" >&2
    echo >&2
    echo "  ac    Assetto Corsa's folder and its layout tests, then the core" >&2
    echo "  acc   Competizione's, then the core" >&2
    echo "  core  everything that does not name a simulator" >&2
    echo "  all   the whole workspace" >&2
    exit 2
    ;;
esac

if [ -n "$module" ]; then
  echo "── $name ─────────────────────────────────────────────"
  # The game's own folder: its structs, its conversion, its paths, its reader.
  cargo test -p ac_core --lib "$module"
  # ...and the layout tests, which are the ones that hold it against bytes the
  # game actually published. These live in `tests_suite` because they carry a
  # capture rather than a fixture.
  cargo test -p tests_suite "$layout"
fi

echo
echo "── the core, which no game may change ────────────────"
# Everything in the core except the game folders: the engineer, the analyser,
# the corner detection, the confidence model, the config, the broadcast and the
# overlay frame. A game folder is meant to be additive, so a change inside one
# that moves a number here is the finding.
cargo test -p ac_core --lib -- --skip games::
# And the rule that keeps a game's layout inside its own folder, which is the
# one test a game change is most likely to break.
cargo test -p tests_suite boundary_tests

echo
echo "Not the whole suite. Before pushing:"
echo "  cargo test --workspace"
echo "  cargo clippy --workspace --all-targets -- -D warnings"
echo "  cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings"
