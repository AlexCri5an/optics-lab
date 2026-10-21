#!/usr/bin/env bash
# Build and package OpticsLab for Linux (.tar.gz and AppImage)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
VERSION=$(grep -m1 '^version' "$ROOT/Cargo.toml" | cut -d '"' -f 2)
DIST="$ROOT/dist"
WORK="$ROOT/target/linux-package"
STAGE="$WORK/root"

ARCH="$(uname -m)"
echo "==> Building OpticsLab v$VERSION for Linux ($ARCH)..."

cargo build --release -p optics-desktop -p optics-cli

rm -rf "$WORK"
mkdir -p "$STAGE/usr/bin" "$STAGE/usr/share/applications" "$DIST"

cp "$ROOT/target/release/OpticsLab" "$STAGE/usr/bin/optics-desktop"
cp "$ROOT/target/release/OpticsLab" "$STAGE/usr/bin/OpticsLab"
cp "$ROOT/target/release/optics-cli" "$STAGE/usr/bin/optics-cli"
chmod +x "$STAGE/usr/bin/"*

cp "$ROOT/packaging/linux/optics-lab.desktop" "$STAGE/usr/share/applications/"

echo "==> Creating tarball distribution..."
TARBALL="$DIST/OpticsLab-$VERSION-linux-$ARCH.tar.gz"
tar -czvf "$TARBALL" -C "$STAGE/usr" bin share

echo "==> Linux package successfully created at $TARBALL"
ls -lh "$TARBALL"
