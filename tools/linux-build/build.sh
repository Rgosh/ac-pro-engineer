#!/usr/bin/env bash
# Build one of this family's programs for Linux inside the old-glibc container.
#
#   tools/linux-build/build.sh <checkout> <cargo args…>
#
# The checkouts sit side by side and depend on each other by path, so the
# whole parent folder is mounted at the same place. The crate registry is the
# host's, so nothing is downloaded twice; the build folder is the container's
# own, so a host build and this one never share artefacts linked against
# different glibcs. Prints the target folder the binaries are in.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
checkout="$(cd "$1" && pwd)"; shift
parent="$(dirname "$checkout")"
image=pe-linux-build:22.04
podman image exists "$image" || podman build -q -t "$image" "$here" >&2
target="${PE_LINUX_TARGET:-$HOME/.cache/pe-linux-build/target}"
mkdir -p "$target" "$HOME/.cargo/registry" "$HOME/.cargo/git"
# Variables a build reads, passed through by name: PE_ENV="RGPE_EMBED_HOOK".
pass=()
for name in ${PE_ENV:-}; do
  [ -n "${!name:-}" ] && pass+=(-e "$name=${!name}")
done
podman run --rm --userns=keep-id "${pass[@]}" \
  -v "$parent:$parent" \
  -v "$HOME/.cargo/registry:/usr/local/cargo/registry" \
  -v "$HOME/.cargo/git:/usr/local/cargo/git" \
  -v "$target:$target" \
  -e CARGO_TARGET_DIR="$target" -e CARGO_INCREMENTAL=0 \
  -w "$checkout" "$image" cargo "$@" >&2
echo "$target"
