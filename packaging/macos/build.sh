#!/usr/bin/env bash
# Builds the macOS release into target/macos/: Katch.app (universal: Apple silicon and Intel),
# Katch-macOS.zip (what Katch's updater installs) and Katch-macOS.dmg (drag Katch to Applications),
# plus Snag-macOS.zip, a copy for Snag 1.1.3 and older to update from (RELEASE-ALIASES.md).
# Run on a Mac with Xcode's command line tools (xcode-select --install) and both Rust targets:
#   rustup target add aarch64-apple-darwin x86_64-apple-darwin
# SNAG_TOOLS_DIR=<folder> also puts the yt-dlp, gallery-dl, ffmpeg and ffprobe found there into
# Katch.app/Contents/Resources/bin (Katch uses those before downloading its own).
set -euo pipefail
cd "$(dirname "$0")/../.."

version=$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"(.*)".*/\1/')
out=target/macos
app="$out/Katch.app"
rm -rf "$out"
mkdir -p "$out"

# Info.plist's LSMinimumSystemVersion.
export MACOSX_DEPLOYMENT_TARGET=11.0
for target in aarch64-apple-darwin x86_64-apple-darwin; do
  cargo build --release -p rdm-app --target "$target"
done

mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
lipo -create -output "$app/Contents/MacOS/snag" \
  target/aarch64-apple-darwin/release/snag target/x86_64-apple-darwin/release/snag
sed "s/@VERSION@/$version/g" packaging/macos/Info.plist > "$app/Contents/Info.plist"
# Only macOS's own libraries may be linked: a library from the build machine (Homebrew's
# liblzma, say) is missing on most Macs and Katch would quit at launch.
if otool -L "$app/Contents/MacOS/snag" | grep -E "^\s" | grep -vE "^\s+(/usr/lib/|/System/Library/)"; then
  echo "Katch links libraries outside macOS (above): build them in instead" >&2
  exit 1
fi
printf 'APPL????' > "$app/Contents/PkgInfo"

# The icon: the logo on macOS's icon grid (packaging/macos/icon-*.png, from docs/logo), the bold
# mark at the two smallest sizes where the engraving's fine lines blur.
iconset="$out/Katch.iconset"
mkdir -p "$iconset"
icon() { # <pixels> <file name>
  local src=packaging/macos/icon-1024.png
  [ "$1" -le 32 ] && src=packaging/macos/icon-small-1024.png
  sips -z "$1" "$1" "$src" --out "$iconset/$2" >/dev/null
}
for size in 16 32 128 256 512; do
  icon "$size" "icon_${size}x${size}.png"
  icon $((size * 2)) "icon_${size}x${size}@2x.png"
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/Snag.icns"
rm -rf "$iconset"

if [ -n "${SNAG_TOOLS_DIR:-}" ]; then
  mkdir -p "$app/Contents/Resources/bin"
  for tool in yt-dlp gallery-dl ffmpeg ffprobe; do
    if [ -f "$SNAG_TOOLS_DIR/$tool" ]; then
      cp "$SNAG_TOOLS_DIR/$tool" "$app/Contents/Resources/bin/"
      chmod 755 "$app/Contents/Resources/bin/$tool"
    fi
  done
fi

# Ad-hoc signature: Apple silicon runs only signed code. Not notarized, so the first launch is
# right-click → Open (see the README).
xattr -cr "$app"
codesign --force --deep --sign - "$app"
codesign --verify --deep --strict "$app"

# The zip keeps the bundle as it is (ditto: permissions, symlinks, the signature).
ditto -c -k --sequesterRsrc --keepParent "$app" "$out/Katch-macOS.zip"
# The name the updater of Snag 1.1.3 and older looks for (crates/core/src/selfupdate.rs: OLD_MAC_ZIP).
# It takes the one .app inside, whatever its name, and puts it in place of its Snag.app.
cp "$out/Katch-macOS.zip" "$out/Snag-macOS.zip"

# The disk image: Katch.app next to a link to Applications.
stage="$out/dmg"
mkdir -p "$stage"
ditto "$app" "$stage/Katch.app"
ln -s /Applications "$stage/Applications"
hdiutil create -volname Katch -srcfolder "$stage" -fs HFS+ -format UDZO -ov "$out/Katch-macOS.dmg"
rm -rf "$stage"

(cd "$out" && shasum -a 256 Katch-macOS.zip Katch-macOS.dmg Snag-macOS.zip | tee "SHA256SUMS-macos-$version.txt")
ls -la "$out/Katch-macOS.zip" "$out/Katch-macOS.dmg" "$out/Snag-macOS.zip"
