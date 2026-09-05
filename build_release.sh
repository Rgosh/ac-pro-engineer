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
echo "Pro Engineer All-in-One Builder ${VERSION}"
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
echo "[3/5] Building the Windows binaries (x86_64-pc-windows-gnu)..."
# Deliberately non-fatal. The Linux binary is already built by this point, and
# the cross-build needs a mingw toolchain (`x86_64-w64-mingw32-dlltool`) that a
# Linux box will not have unless someone installed it on purpose. Under `set
# -e` an unguarded failure here aborted the whole script and left an empty
# bundle directory behind — losing the Linux build that had just succeeded.
# The official cross-platform artifacts come from the release workflow anyway;
# this script is for producing a local bundle.
BRIDGE_BUILT=0
if rustup target list | grep -q "x86_64-pc-windows-gnu (installed)"; then
    # **The terminal itself, not only the bridge.** This bundle's own README
    # says it holds both builds, and for three releases it did not: nothing
    # here ever cross-built `ac_tui`, so `Windows/` went out with a README in
    # it and nothing to run. The bridge needs the same toolchain, so a machine
    # that can produce one can produce the other.
    if cargo build -p ac_tui --target x86_64-pc-windows-gnu --release; then
        echo "  - ac_pro_engineer.exe built."
    else
        echo ""
        echo "  WARNING: could not cross-build ac_pro_engineer.exe."
        echo "  The Windows folder will hold its README and nothing else."
    fi
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

WINDOWS_BUILT=0
if [ -f "target/x86_64-pc-windows-gnu/release/ac_pro_engineer.exe" ]; then
    cp "target/x86_64-pc-windows-gnu/release/ac_pro_engineer.exe" "${WIN_DIR}/"
    WINDOWS_BUILT=1
    echo "  - Windows ac_pro_engineer.exe binary copied."
elif [ -f "target/release/ac_pro_engineer.exe" ]; then
    cp "target/release/ac_pro_engineer.exe" "${WIN_DIR}/"
    WINDOWS_BUILT=1
    echo "  - Windows ac_pro_engineer.exe binary copied."
fi

if [ -f "README.txt" ]; then
    cp "README.txt" "${BUNDLE_DIR}/"
    cp "README.txt" "${WIN_DIR}/"
    cp "README.txt" "${LIN_DIR}/"
    echo "  - README.txt copied to all release folders."
fi

# Everything needed when the automatic path does not work, in the bundle rather
# than in a document nobody opens until the game is already broken.
for doc in README.md CHANGELOG.md LICENSE LICENSE-MIT-HISTORICAL NOTICE LICENSING.md; do
    if [ -f "${doc}" ]; then
        cp "${doc}" "${BUNDLE_DIR}/"
    fi
done

# The Lua panel, loose. It is embedded in the binary and installed at startup,
# so this copy is for the case that fails: an unwritable game folder, an install
# Steam put somewhere unusual, a second copy of AC. Dropping the folder into
# assettocorsa/apps/lua/ by hand is then the whole remedy.
#
# Copied *to a different name than it has in the tree*: CSP finds an app's entry
# point by folder name, so this has to land as `ac_pro_engineer` however the
# sources are organised. The rename that moved the panel under assets/ would
# otherwise have shipped a folder CSP ignores, silently.
if [ -d "assets/frontends/csp-panel" ]; then
    mkdir -p "${BUNDLE_DIR}/overlay"
    cp -r "assets/frontends/csp-panel" "${BUNDLE_DIR}/overlay/ac_pro_engineer"
    echo "  - Lua overlay copied for manual installation."
fi

# The prefix setup, next to the bridge it tells people to launch. CSP loads
# through Windows libraries Proton ships only as stubs — including the fonts,
# which is why there are no font files in this bundle and cannot be: they go
# into the prefix (corefonts), not into an archive.
if [ -f "packaging/proton-setup.sh" ]; then
    cp "packaging/proton-setup.sh" "${LIN_DIR}/"
    chmod +x "${LIN_DIR}/proton-setup.sh"
    echo "  - proton-setup.sh copied to the Linux folder."
fi

if [ -f "packaging/ac-pro-engineer.desktop" ]; then
    cp "packaging/ac-pro-engineer.desktop" "${LIN_DIR}/"
    echo "  - desktop entry copied to the Linux folder."
fi

echo ""
echo "[5/5] Packaging the release archives..."
cd "${RELEASE_DIR}"
tar -czf "${BUNDLE_NAME}_linux.tar.gz" "${BUNDLE_NAME}"
# **A zip as well, when both builds are in it.** The forum listing takes one
# attachment and its readers are mostly on Windows, where a `.tar.gz` is a
# second program to install before the first one can be run. Same tree, same
# name the v0.3.5 bundle used.
if [ "${WINDOWS_BUILT}" -eq 1 ]; then
    # `zip` is not on a stock Arch install and this script is otherwise
    # dependency-free, so python's zipfile is the fallback — with the mode
    # bits written by hand, because the module drops them and a Linux binary
    # that arrives without its execute bit is a bug report.
    if command -v zip >/dev/null; then
        zip -qr "${BUNDLE_NAME}_lin_win.zip" "${BUNDLE_NAME}"
    else
        python3 - "${BUNDLE_NAME}" <<'ZIP'
import os, sys, zipfile
root = sys.argv[1]
with zipfile.ZipFile(f"{root}_lin_win.zip", "w", zipfile.ZIP_DEFLATED) as archive:
    for base, _, names in os.walk(root):
        for name in sorted(names):
            path = os.path.join(base, name)
            info = zipfile.ZipInfo.from_file(path, path)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = (os.stat(path).st_mode & 0xFFFF) << 16
            with open(path, "rb") as handle:
                archive.writestr(info, handle.read())
ZIP
    fi
    echo "  - ${BUNDLE_NAME}_lin_win.zip"
fi
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
if [ "${WINDOWS_BUILT}" -eq 1 ]; then
    echo "Archive: ${RELEASE_DIR}/${BUNDLE_NAME}_lin_win.zip"
else
    echo "No Windows binary: this bundle is Linux only, and its README says"
    echo "otherwise. Install a mingw-w64 toolchain and re-run before shipping it."
fi
echo "=========================================="
