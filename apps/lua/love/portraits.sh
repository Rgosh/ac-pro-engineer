#!/usr/bin/env bash
# Every picture of the overlay that the README shows, regenerated.
#
# `tui_tester` does this for the terminal application; there was no equivalent
# for the panel, so the one overlay picture in the README was a screenshot of
# the *terminal's* overlay control centre — a window that no longer exists.
#
# Each run draws one window, alone, in a LÖVE window sized exactly to it, waits
# three seconds for the simulated lap to reach interesting numbers, and writes a
# PNG. LÖVE can only save into its own directory, so they are copied out here.
#
#   ./portraits.sh              # into ../../../screenshots
#   ./portraits.sh /some/dir
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
out="${1:-$here/../../../screenshots}"
save="${XDG_DATA_HOME:-$HOME/.local/share}/love/acpe-harness"

if ! command -v love >/dev/null 2>&1; then
  echo "love is not installed; see ./run.sh for how to get it." >&2
  exit 1
fi

mkdir -p "$out"

# name  window  size  [extra flags...]
shot() {
  local name="$1" window="$2" size="$3"
  shift 3
  rm -f "$save/$name.png"
  love "$here" --portrait "$window" --size "$size" --shot "$name.png" "$@" >/dev/null
  if [[ ! -f "$save/$name.png" ]]; then
    echo "  [!!] $name.png was not written" >&2
    return 1
  fi
  cp "$save/$name.png" "$out/$name.png"
  echo "  [OK] $name.png"
}

echo "Rendering the overlay's windows to PNG..."

# The windows the manifest declares, at the sizes it declares them at.
shot Overlay_Main      main      360x470
shot Overlay_Engineer  engineer  380x180
shot Overlay_Telemetry telemetry 400x700
shot Overlay_Status    status    380x330

# The settings window, one picture per tab: it is the one window a driver reads
# through rather than glances at, and a single shot of it says nothing about the
# five tabs it is not showing.
shot Overlay_Settings_Panel   settings 460x560 --app-tab Panel/Blocks
shot Overlay_Settings_Advice  settings 460x560 --app-tab Advice
shot Overlay_Settings_Look    settings 460x560 --app-tab Look/Colour
shot Overlay_Settings_Units   settings 460x400 --app-tab Units
shot Overlay_Settings_Console settings 460x330 --app-tab Console
# The Dev tab is only in the window when the panel's own developer mode is on.
shot Overlay_Settings_Dev     settings 460x560 --app-dev --app-tab Dev/Switches

echo
echo "Done. $out"
