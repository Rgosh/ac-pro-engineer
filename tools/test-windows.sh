#!/usr/bin/env bash
# Run the test suite as a *Windows* binary, on Linux, through Wine.
#
#   ./tools/test-windows.sh              # the whole workspace
#   ./tools/test-windows.sh -p ac_core   # anything else is passed to cargo
#
# Why this exists: the Windows half of this program has been compiled and
# linted for a long time and never *run*. `cargo clippy --target
# x86_64-pc-windows-gnu` proves it builds; it says nothing about whether
# `CreateToolhelp32Snapshot` finds a process, whether a named mapping opens, or
# whether a path with a drive letter resolves. Those are the parts that only
# exist on that platform, and they were the parts nothing exercised.
#
# Wine is not Windows and this is not a substitute for running it there. It is
# the difference between "compiles" and "the Win32 calls in this build return
# what the code expects", which is most of the distance.
#
# The prefix is a scratch one and is created on first use, so this never
# touches the Proton prefixes the games live in — starting a Wine process in
# one of those is how the launcher used to stop Steam from starting a game.
set -euo pipefail

TARGET=x86_64-pc-windows-gnu
# Under the cache directory rather than /tmp: Wine refuses to create a
# configuration directory in a path it does not own, and a shared /tmp is
# exactly that on several distributions — "'/tmp' is not owned by you".
PREFIX="${ACPE_TEST_WINEPREFIX:-${XDG_CACHE_HOME:-$HOME/.cache}/acpe-test-wineprefix}"

if ! command -v wine >/dev/null 2>&1; then
    echo "wine is not installed — this script needs it to run the Windows binaries." >&2
    echo "The build and the lints still work without it:" >&2
    echo "  cargo clippy --workspace --all-targets --target $TARGET -- -D warnings" >&2
    exit 127
fi

if ! rustup target list --installed 2>/dev/null | grep -qx "$TARGET"; then
    echo "The $TARGET target is not installed. Add it with:" >&2
    echo "  rustup target add $TARGET" >&2
    exit 127
fi

echo "Wine:   $(wine --version)"
echo "Prefix: $PREFIX"
echo "Target: $TARGET"
echo

# WINEDEBUG silences the fixme chatter that would otherwise bury the test
# output; the runner is what makes cargo hand each test binary to Wine.
export WINEPREFIX="$PREFIX"
export WINEDEBUG="${WINEDEBUG:--all}"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER=wine

if [ "$#" -eq 0 ]; then
    set -- --workspace
fi

cargo test "$@" --target "$TARGET"

echo
echo "Fewer tests ran than on Linux, and that is correct: the bridge and the"
echo "/dev/shm paths are #[cfg(not(target_os = \"windows\"))] and do not exist here."
