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
# Ubuntu 20.04's glibc 2.31 unless told otherwise — see the Containerfile.
base="${PE_LINUX_BASE:-20.04}"
image="pe-linux-build:$base"
# **Without podman, zig.** `cargo zigbuild` links against the glibc named in the
# target — 2.31 for the 20.04 base, 2.35 for 22.04 — so the binary asks for
# the same symbols the container's would, with no container. Chosen by itself
# when podman is not installed (a cloud machine, a distro without it), or with
# PE_LINUX_ZIG=1; the binaries land where the container's do, under
# <target>/x86_64-unknown-linux-gnu/, and the glibc check after it is the same.
if [ "${PE_LINUX_ZIG:-}" = 1 ] || ! command -v podman >/dev/null; then
  case "$base" in 22.04) glibc=2.35 ;; *) glibc=2.31 ;; esac
  command -v cargo-zigbuild >/dev/null || {
    echo "neither podman nor cargo-zigbuild: install one (cargo install cargo-zigbuild, and zig)" >&2
    exit 1
  }
  target="${PE_LINUX_TARGET:-$HOME/.cache/pe-linux-build/target-zig-$glibc}"
  mkdir -p "$target"
  sub="$1"; shift
  args=()
  has_target=0
  for a in "$@"; do
    if [ "$a" = x86_64-unknown-linux-gnu ]; then
      a="x86_64-unknown-linux-gnu.$glibc"
      has_target=1
    fi
    args+=("$a")
  done
  [ "$has_target" = 1 ] || args+=(--target "x86_64-unknown-linux-gnu.$glibc")
  [ "$sub" = build ] && sub=zigbuild
  ( cd "$checkout" && CARGO_TARGET_DIR="$target" CARGO_INCREMENTAL=0 cargo "$sub" "${args[@]}" ) >&2
  # The caller reads <printed>/release/<binary>; with an explicit target cargo
  # puts it one folder further down, which is where this points.
  if [ "$has_target" = 1 ]; then echo "$target"; else echo "$target/x86_64-unknown-linux-gnu"; fi
  exit 0
fi
podman image exists "$image" || podman build -q --build-arg "BASE=$base" -t "$image" "$here" >&2
# A target folder per base: artefacts linked against one glibc are never
# reused by a build against another.
target="${PE_LINUX_TARGET:-$HOME/.cache/pe-linux-build/target-$base}"
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
