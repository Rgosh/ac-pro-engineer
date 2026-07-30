#!/usr/bin/env bash
set -e

VERSION="v0.2.3"
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
if rustup target list | grep -q "x86_64-pc-windows-gnu (installed)"; then
    cargo build -p shm-bridge --target x86_64-pc-windows-gnu --release
else
    echo "Notice: x86_64-pc-windows-gnu target not installed. Attempting standard build..."
    cargo build -p shm-bridge --release || true
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
echo "DONE! Release archive is ready in '${RELEASE_DIR}/${BUNDLE_NAME}_linux.tar.gz'"
echo "=========================================="
