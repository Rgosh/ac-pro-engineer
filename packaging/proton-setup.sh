#!/usr/bin/env bash
# Prepare Assetto Corsa's Proton prefix so CSP — and therefore the in-game
# panel — can load at all.
#
# CSP loads through Windows libraries Proton ships only as stubs. Without them
# the launcher opens on a black screen and the game crashes as soon as a Lua
# script runs, which reads as "the overlay broke my game" and is nothing to do
# with the overlay.
#
# This is the whole of the crib sheet from the README, in one file, because
# "run these four commands in this order" is not something to leave in a
# document nobody opens until the game is already broken.
#
# Fonts are not shipped with this application and cannot be: the desktop side
# is a terminal program that uses the terminal's own font, and the in-game
# panel draws through CSP's DirectWrite. `corefonts` below is the font step,
# and it has to go into the prefix rather than into a release archive.

set -euo pipefail

APPID="${ACPE_APPID:-244210}"

say() { printf '\n\033[1m%s\033[0m\n' "$*"; }
note() { printf '  %s\n' "$*"; }

say "AC Pro Engineer — Proton prefix setup (app id ${APPID})"

if ! command -v protontricks >/dev/null 2>&1; then
    cat <<'EOF'

protontricks is not installed, and every step below needs it.

  Arch:     sudo pacman -S protontricks
  Debian:   sudo apt install protontricks
  Flatpak:  flatpak install com.github.Matoking.protontricks

Install it and run this again.
EOF
    exit 1
fi

# Run Assetto Corsa once before this: the prefix does not exist until Steam has
# created it, and every command here would fail with a message about a missing
# app id that means "you have not launched the game yet".
if ! protontricks -l 2>/dev/null | grep -q "${APPID}"; then
    cat <<EOF

protontricks does not know about app id ${APPID}.

Launch Assetto Corsa from Steam once and quit it. That is what creates the
Proton prefix; until it exists there is nothing here to set up.
EOF
    exit 1
fi

say "[1/4] vcrun2019 and corefonts"
note "The Visual C++ runtime CSP is built against, and the fonts it draws with."
protontricks "${APPID}" --force vcrun2019 corefonts

say "[2/4] d3dcompiler_47"
note "CSP compiles its own shaders at load; Proton's stub cannot."
protontricks "${APPID}" d3dcompiler_47

say "[3/4] dwrite"
note "DirectWrite. The panel draws every string through it, at its own size —"
note "CSP's five font tiers cannot be scaled, and a 4K screen needs more than"
note "the largest of them."
protontricks "${APPID}" dwrite

say "[4/4] Checking for the bridge"
BRIDGE="$(dirname "$0")/shm-bridge.exe"
if [ -f "${BRIDGE}" ]; then
    note "Found ${BRIDGE}"
    note "Start it before the game, and leave it running:"
    note "    protontricks-launch --appid ${APPID} ${BRIDGE}"
else
    note "shm-bridge.exe is not next to this script."
    note "Without it the desktop application cannot read the game's telemetry"
    note "and the in-game panel never receives a frame. It ships in the same"
    note "archive as this script."
fi

cat <<EOF

Done. In order, every time:

  1. protontricks-launch --appid ${APPID} shm-bridge.exe   (leave it running)
  2. ./ac_pro_engineer                                     (leave it running)
  3. Assetto Corsa

The panel appears in CSP's app sidebar, on the right, as "AC Pro Engineer".
If it says it is waiting for the application while the application is running,
the bridge is the piece to check:

  ./ac_pro_engineer   then look at the IN-GAME OVERLAY card

EOF
