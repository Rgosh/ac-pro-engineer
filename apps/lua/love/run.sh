#!/usr/bin/env bash
# Launch the overlay harness. Every flag is passed straight through to the
# LÖVE app, which is also where they are documented:
#
#   ./run.sh --help
#
# The point of this script is that it works from anywhere and says something
# useful when LÖVE is missing, rather than that it adds behaviour of its own.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if ! command -v love >/dev/null 2>&1; then
  cat >&2 <<'EOF'
love is not installed.

  Arch/CachyOS   sudo pacman -S love
  Debian/Ubuntu  sudo apt install love
  Fedora         sudo dnf install love

The harness needs LÖVE 11.x; the overlay itself does not — this is only the
thing that draws it outside the game.
EOF
  exit 1
fi

# --shm is shorthand for "read what the desktop application is publishing".
args=()
for a in "$@"; do
  case "$a" in
    --shm) args+=(--source shm) ;;
    *) args+=("$a") ;;
  esac
done

exec love "$here" ${args[@]+"${args[@]}"}
