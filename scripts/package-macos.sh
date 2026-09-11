#!/bin/sh
set -eu

target="${1:?target triple is required}"
archive_name="${2:?archive name is required}"
root_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
stage_dir="$root_dir/target/package-$target"
app_dir="$stage_dir/RDR2 Xbox Save Converter.app"

rm -rf "$stage_dir"
mkdir -p "$app_dir/Contents/MacOS" "$app_dir/Contents/Resources"
cp "$root_dir/target/$target/release/rdr2-xbox-save-converter" "$app_dir/Contents/MacOS/"
cp "$root_dir/README.md" "$root_dir/keys.example.toml" "$root_dir/LICENSE" "$stage_dir/"

cat > "$app_dir/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDisplayName</key><string>RDR2 Xbox Save Converter</string>
  <key>CFBundleExecutable</key><string>rdr2-xbox-save-converter</string>
  <key>CFBundleIdentifier</key><string>cc.hupose.rdr2-xbox-save-converter</string>
  <key>CFBundleName</key><string>RDR2 Xbox Save Converter</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

codesign --force --deep --sign - "$app_dir"
rm -f "$root_dir/$archive_name"
(cd "$stage_dir" && COPYFILE_DISABLE=1 zip -q -r "$root_dir/$archive_name" .)
