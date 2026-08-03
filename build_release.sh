#!/usr/bin/env bash
set -e

# Read from the workspace manifest rather than duplicated here. This was
# hardcoded and two releases out of date, so every bundle it produced was
# named after the wrong version.
VERSION="v$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
RELEASE_DIR="releases"
BUNDLE_NAME="ac_pro_engineer_${VERSION}"
BUNDLE_DIR="${RELEASE_DIR}/${BUNDLE_NAME}"
WIN_DIR="${BUNDLE_DIR}/Windows"
LIN_DIR="${BUNDLE_DIR}/Linux"

echo "=========================================="
echo "AC Pro Engineer All-in-One Builder ${VERSION}"
echo "=========================================="

echo ""
echo "[1/5] Cleaning old builds..."
rm -rf "${RELEASE_DIR}"
mkdir -p "${WIN_DIR}"
mkdir -p "${LIN_DIR}"

echo ""
echo "[2/5] Building Linux TUI (ac_pro_engineer)..."
cargo build -p ac_tui --release

echo ""
echo "[3/5] Building shm-bridge for Wine/Proton (x86_64-pc-windows-gnu)..."
# Deliberately non-fatal. The Linux binary is already built by this point, and
# the cross-build needs a mingw toolchain (`x86_64-w64-mingw32-dlltool`) that a
# Linux box will not have unless someone installed it on purpose. Under `set
# -e` an unguarded failure here aborted the whole script and left an empty
# bundle directory behind — losing the Linux build that had just succeeded.
# The official cross-platform artifacts come from the release workflow anyway;
# this script is for producing a local bundle.
BRIDGE_BUILT=0
if rustup target list | grep -q "x86_64-pc-windows-gnu (installed)"; then
    if cargo build -p shm-bridge --target x86_64-pc-windows-gnu --release; then
        BRIDGE_BUILT=1
    else
        echo ""
        echo "  WARNING: could not cross-build shm-bridge.exe."
        echo "  A mingw-w64 toolchain is required for the windows-gnu target:"
        echo "    Arch:   sudo pacman -S mingw-w64-gcc"
        echo "    Debian: sudo apt install gcc-mingw-w64-x86-64"
        echo "  Continuing without it; the Linux bundle will have no bridge."
    fi
else
    echo "Notice: x86_64-pc-windows-gnu target not installed."
    echo "  Install it with: rustup target add x86_64-pc-windows-gnu"
    cargo build -p shm-bridge --release && BRIDGE_BUILT=1 || true
fi

echo ""
echo "[4/5] Checking Windows binaries..."
if [ -f "target/x86_64-pc-windows-gnu/release/shm-bridge.exe" ]; then
    cp "target/x86_64-pc-windows-gnu/release/shm-bridge.exe" "${LIN_DIR}/"
    echo "  - shm-bridge.exe copied to Linux folder."
elif [ -f "target/release/shm-bridge.exe" ]; then
    cp "target/release/shm-bridge.exe" "${LIN_DIR}/"
    echo "  - shm-bridge.exe copied to Linux folder."
fi

if [ -f "target/release/ac_pro_engineer" ]; then
    cp "target/release/ac_pro_engineer" "${LIN_DIR}/"
    echo "  - Linux ac_pro_engineer binary copied."
fi

if [ -f "target/release/ac_pro_engineer.exe" ]; then
    cp "target/release/ac_pro_engineer.exe" "${WIN_DIR}/"
    echo "  - Windows ac_pro_engineer.exe binary copied."
fi

if [ -f "README.txt" ]; then
    cp "README.txt" "${BUNDLE_DIR}/"
    cp "README.txt" "${WIN_DIR}/"
    cp "README.txt" "${LIN_DIR}/"
    echo "  - README.txt copied to all release folders."
fi

echo ""
echo "[5/5] Packaging tar.gz release archive..."
cd "${RELEASE_DIR}"
tar -czf "${BUNDLE_NAME}_linux.tar.gz" "${BUNDLE_NAME}"
cd ..

echo ""
echo "=========================================="
echo "Bundle contents:"
find "${BUNDLE_DIR}" -type f | sed "s|^|  |"
echo ""
if [ "${BRIDGE_BUILT}" -eq 0 ]; then
    echo "INCOMPLETE: shm-bridge.exe is missing, so this bundle cannot read"
    echo "telemetry under Wine/Proton. Install a mingw-w64 toolchain and re-run,"
    echo "or take the artifacts from the release workflow instead."
else
    echo "COMPLETE: Linux binary and Wine/Proton bridge are both present."
fi
echo ""
echo "Archive: ${RELEASE_DIR}/${BUNDLE_NAME}_linux.tar.gz"
echo "=========================================="
