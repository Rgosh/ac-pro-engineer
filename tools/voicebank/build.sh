#!/usr/bin/env bash
# Make the race engineer's voices. One command, nobody records anything.
#
#   tools/voicebank/build.sh [--lang en] [--voice male] [--demo]
#
# Its own Python environment under ~/.cache/pe-voicebank, the models there
# too — nothing is installed system-wide and nothing large lands in the
# repository. Needs python3, ffmpeg with libvorbis and libmp3lame, and
# huggingface.co reachable. See voicebank.py for what each step does.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
venv="$HOME/.cache/pe-voicebank/venv"
command -v ffmpeg >/dev/null || { echo "ffmpeg is needed (with libvorbis)" >&2; exit 1; }
if [ ! -x "$venv/bin/python" ]; then
  python3 -m venv "$venv"
fi
# Installed again only when the list changed.
stamp="$venv/.requirements.sha256"
want="$(sha256sum "$here/requirements.txt" | cut -d' ' -f1)"
if [ "$(cat "$stamp" 2>/dev/null)" != "$want" ]; then
  "$venv/bin/pip" install -q --upgrade pip
  "$venv/bin/pip" install -q -r "$here/requirements.txt"
  echo "$want" > "$stamp"
fi
exec "$venv/bin/python" -I "$here/voicebank.py" "$@"
