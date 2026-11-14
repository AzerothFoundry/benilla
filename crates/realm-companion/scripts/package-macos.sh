#!/bin/bash
set -euo pipefail
root="$(cd "$(dirname "$0")/../../.." && pwd)"
out="${1:-$root/dist/Realm Companion.app}"
cd "$root"
cargo build --release -p realm-companion
build_dir="${CARGO_TARGET_DIR:-$root/target}"
mkdir -p "$out/Contents/MacOS" "$out/Contents/Resources"
cp "$build_dir/release/realm-companion" "$out/Contents/MacOS/realm-companion"
cat > "$out/Contents/MacOS/RealmCompanion" <<'LAUNCHER'
#!/bin/bash
set -e
companion_home="$HOME/Library/Application Support/AzerothFoundry/companion"
mkdir -p "$companion_home/run"
exec "$(dirname "$0")/realm-companion" "$@" >> "$companion_home/run/app.log" 2>&1
LAUNCHER
chmod +x "$out/Contents/MacOS/RealmCompanion"
cat > "$out/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>RealmCompanion</string>
<key>CFBundleIdentifier</key><string>org.azerothfoundry.companion</string>
<key>CFBundleName</key><string>Realm Companion</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleIconFile</key><string>RealmCompanion.icns</string>
<key>CFBundleShortVersionString</key><string>0.4.12</string>
<key>CFBundleVersion</key><string>20</string>
<key>LSUIElement</key><true/>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
# Local-only icon derived from the operator's install; CI without game data omits it.
icon_png="${COMPANION_ICON_PNG:-$HOME/Library/Application Support/AzerothFoundry/companion/generated/wow-app-icon.png}"
if [[ -z "${COMPANION_ICON_PNG:-}" ]]; then
  "$build_dir/release/realm-companion" --export-app-icon "$icon_png" || true
fi
if [[ -f "$icon_png" ]]; then
  icon_work="$(mktemp -d)/RealmCompanion.iconset"
  mkdir -p "$icon_work"
  for size in 16 32 128 256 512; do
    sips -z "$size" "$size" "$icon_png" --out "$icon_work/icon_${size}x${size}.png" >/dev/null
    double=$((size * 2))
    sips -z "$double" "$double" "$icon_png" --out "$icon_work/icon_${size}x${size}@2x.png" >/dev/null
  done
  iconutil -c icns "$icon_work" -o "$out/Contents/Resources/RealmCompanion.icns"
  rm -rf "$(dirname "$icon_work")"
fi
codesign --force --deep --sign - "$out"
printf '%s\n' "$out"
