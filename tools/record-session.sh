#!/usr/bin/env bash
# Record a game's shared memory for a whole session, in one command.
#
#   ./tools/record-session.sh                             # Competizione
#   ./tools/record-session.sh "Assetto Corsa"             # another game
#   ./tools/record-session.sh "Assetto Corsa" my-run.txt  # and a file name
#
# Run this *before* starting the game, drive, then close this window. The
# report is rewritten every few seconds and is always complete, so there is
# nothing to do at the end.
#
# What it does, and why each step is here:
#
#   1. Finds the game's Steam appid by reading the manifests on this machine.
#      Nothing is hardcoded: a guessed appid is the kind of thing that costs an
#      evening the day it turns out to be wrong.
#   2. Starts `shm-bridge.exe` inside *that game's* Proton prefix. The game is a
#      Windows process under Proton and publishes into the prefix; the bridge
#      pre-creates the mappings as files in /dev/shm so the writes land where
#      Linux can read them. **This has to happen before the game starts**, which
#      is the whole reason this script exists rather than a list of commands.
#   3. Runs the recorder until you stop it, and shuts the bridge down after.
set -euo pipefail

GAME="${1:-Assetto Corsa Competizione}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STAMP="$(date +%Y%m%d-%H%M)"
SLUG="$(printf '%s' "$GAME" | tr '[:upper:] ' '[:lower:]-')"
OUT="${2:-$ROOT/$SLUG-$STAMP.txt}"

say() { printf '\n\033[36m==\033[0m %s\n' "$*"; }
die() { printf '\n\033[31m!!\033[0m %s\n' "$*" >&2; exit 1; }

# ── 1. which game, and which prefix ──────────────────────────────────────
# Steam records extra libraries in libraryfolders.vdf; a game may live in any
# of them, and its manifest is what names it.
say "Looking for \"$GAME\" in your Steam libraries"

libraries=(~/.steam/steam ~/.local/share/Steam ~/.steam/root)
vdf="$HOME/.steam/steam/steamapps/libraryfolders.vdf"
[ -f "$vdf" ] || vdf="$HOME/.local/share/Steam/steamapps/libraryfolders.vdf"
if [ -f "$vdf" ]; then
  while read -r extra; do
    libraries+=("$extra")
  done < <(grep -oP '(?<="path")\s*"\K[^"]+' "$vdf" 2>/dev/null || true)
fi

appid=""
found_manifest=""
found_lib=""
for lib in "${libraries[@]}"; do
  [ -d "$lib/steamapps" ] || continue
  for manifest in "$lib"/steamapps/appmanifest_*.acf; do
    [ -f "$manifest" ] || continue
    name=$(grep -m1 '"name"' "$manifest" | cut -d'"' -f4)
    if [ "$name" = "$GAME" ]; then
      appid=$(basename "$manifest" | tr -dc '0-9')
      found_manifest="$manifest"
      found_lib="$lib"
      break 2
    fi
  done
done

[ -n "$appid" ] || die "No Steam manifest names \"$GAME\".
   Installed games are listed by:  grep -h '\"name\"' ~/.steam/steam/steamapps/appmanifest_*.acf
   Pass the name exactly as Steam spells it."

echo "   $GAME is appid $appid"

# A game that is still downloading has a manifest already, so finding one
# proves nothing about whether it can run. StateFlags 4 is "fully installed";
# anything else means Steam is still working. Starting the bridge against a
# half-installed game wastes a session and looks like a bug in the bridge.
state=$(grep -m1 '"StateFlags"' "$found_manifest" | cut -d'"' -f4)
if [ "$state" != "4" ]; then
  done_bytes=$(grep -m1 '"BytesDownloaded"' "$found_manifest" | cut -d'"' -f4 || echo 0)
  todo_bytes=$(grep -m1 '"BytesToDownload"' "$found_manifest" | cut -d'"' -f4 || echo 0)
  if [ "${todo_bytes:-0}" -gt 0 ] 2>/dev/null; then
    pct=$(( done_bytes * 100 / todo_bytes ))
    gb() { awk "BEGIN{printf \"%.1f\", $1/1073741824}"; }
    die "$GAME is still downloading — $(gb "$done_bytes") of $(gb "$todo_bytes") GB ($pct%).
   Let Steam finish, then run this again."
  fi
  die "$GAME is not fully installed (Steam StateFlags $state).
   Let Steam finish, then run this again."
fi

# The Proton prefix is created the first time the game is launched. Without it
# there is nowhere for the bridge to run, and protontricks says so in a way
# that is much harder to read than this.
prefix="$found_lib/steamapps/compatdata/$appid"
[ -d "$prefix" ] || die "$GAME has no Proton prefix yet ($prefix is missing).
   Launch the game once from Steam and quit it, then run this again — that is
   what creates the prefix the bridge has to run inside."

command -v protontricks-launch >/dev/null \
  || die "protontricks-launch is not installed, and the bridge has to run inside
   the game's Proton prefix. Install protontricks and run this again."

# ── 2. the bridge ────────────────────────────────────────────────────────
# The same search the launcher and bridge_probe use, so the bridge this starts
# is the bridge the application would have started.
bridge=""
for candidate in \
  "$ROOT/target/x86_64-pc-windows-gnu/release/shm-bridge.exe" \
  "$ROOT/shm-bridge.exe"; do
  [ -f "$candidate" ] && { bridge="$candidate"; break; }
done

if [ -z "$bridge" ]; then
  say "No shm-bridge.exe yet — building it"
  cargo build --release -p shm-bridge --target x86_64-pc-windows-gnu \
    || die "could not build the bridge. Is the x86_64-pc-windows-gnu target installed?
   rustup target add x86_64-pc-windows-gnu"
  bridge="$ROOT/target/x86_64-pc-windows-gnu/release/shm-bridge.exe"
fi
echo "   using $bridge"

say "Building the recorder"
cargo build --release -p ac_core --example record_pages >/dev/null

# ── 3. run ───────────────────────────────────────────────────────────────
# Whatever happens after this point, the bridge is stopped: leaving one running
# in a prefix means the next session opens mappings somebody else already owns.
bridge_pid=""
cleanup() {
  if [ -n "$bridge_pid" ] && kill -0 "$bridge_pid" 2>/dev/null; then
    printf '\n== stopping the bridge\n'
    kill "$bridge_pid" 2>/dev/null || true
    wait "$bridge_pid" 2>/dev/null || true
  fi
  printf '\nThe recording is in:\n  %s\n\n' "$OUT"
}
trap cleanup EXIT INT TERM

say "Starting the bridge inside $GAME's prefix"
protontricks-launch --appid "$appid" "$bridge" >/dev/null 2>&1 &
bridge_pid=$!

# The bridge has to create the mappings before the game asks for them, so give
# it a moment and say plainly whether it managed.
for _ in $(seq 1 20); do
  [ -f /dev/shm/acpmf_physics ] && break
  sleep 0.5
done

if [ -f /dev/shm/acpmf_physics ]; then
  echo "   mappings are up: $(ls -s --block-size=1 /dev/shm/acpmf_physics | cut -d' ' -f1) bytes"
else
  printf '\n\033[33m??\033[0m The bridge has not created /dev/shm/acpmf_physics yet.\n'
  printf '   Recording anyway — it will start reading as soon as they appear.\n'
fi

say "Recording. Start $GAME now."
cat <<'ADVICE'
   A field only gives itself away when it moves, so during the session:
     * complete at least two laps — some fields are zero until the first one
     * use the whole speed range, and brake hard
     * change TC and ABS, turn the lights on, use the pit limiter
     * drive through the pit lane once

   Close this window or press Ctrl-C when you are done.
ADVICE

"$ROOT/target/release/examples/record_pages" "$OUT"
