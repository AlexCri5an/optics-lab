#!/usr/bin/env bash
# Build and package OpticsLab into a drag-and-drop macOS DMG installer
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
VERSION=$(grep -m1 '^version' "$ROOT/Cargo.toml" | cut -d '"' -f 2)
DIST="$ROOT/dist"
WORK="$ROOT/target/macos-package"
APP="$WORK/OpticsLab.app"
DMG="$DIST/OpticsLab-$VERSION-macos.dmg"

echo "==> Building OpticsLab v$VERSION for macOS..."
cargo build --release -p optics-desktop -p optics-cli

rm -rf "$WORK"
mkdir -p "$WORK/bin" "$DIST"
cp "$ROOT/target/release/OpticsLab" "$WORK/bin/OpticsLab"
cp "$ROOT/target/release/optics-cli" "$WORK/bin/optics-cli"

echo "==> Assembling $APP..."
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$WORK/bin/OpticsLab" "$APP/Contents/MacOS/OpticsLab"
cp "$ROOT/packaging/macos/OpticsLab.icns" "$APP/Contents/Resources/OpticsLab.icns"
chmod +x "$APP/Contents/MacOS/OpticsLab"

sed -e "s/@VERSION@/$VERSION/g" "$ROOT/packaging/macos/Info.plist.in" > "$APP/Contents/Info.plist"
printf 'APPL????' > "$APP/Contents/PkgInfo"

# Ad-hoc code sign so macOS gatekeeper allows local execution
codesign --force --deep --sign - "$APP"

echo "==> Creating DMG staging folder..."
STAGE="$WORK/dmg"
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/OpticsLab.app"
ln -s /Applications "$STAGE/Applications"

echo "==> Generating $DMG..."
rm -f "$DMG"
hdiutil create -srcfolder "$STAGE" -volname "OpticsLab" -fs HFS+ -format UDZO -imagekey zlib-level=9 -ov "$DMG"

codesign --force --sign - "$DMG" || true

echo "==> Successfully created $DMG"
ls -lh "$DMG"
