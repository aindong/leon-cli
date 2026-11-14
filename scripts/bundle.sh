#!/usr/bin/env bash
# bundle.sh: build Leon.app, the macOS app bundle around the leon binary.
#
#   scripts/bundle.sh [--bin PATH] [--out PATH]
#
# --bin PATH  bundle this leon binary instead of running `cargo build --release -p leon`
# --out PATH  where the app goes [default: target/release/Leon.app]
#
# It only ever replaces a Leon.app it made (bundle id dev.leon.app); anything else at --out is
# left alone and the script exits 1. The version comes from `leon --version` (Cargo's version).
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
bin=""
out="$repo/target/release/Leon.app"
while [ $# -gt 0 ]; do
  case "$1" in
    --bin) bin=${2:?--bin needs a path}; shift 2 ;;
    --out) out=${2:?--out needs a path}; shift 2 ;;
    -h|--help) sed -n '2,10s/^# \{0,1\}//p' "$0"; exit 0 ;;
    *) echo "bundle.sh: unknown argument: $1 (try --help)" >&2; exit 2 ;;
  esac
done

# "x.app/" would follow a symlink in the checks below and rm -rf its target
while [ "$out" != / ] && [ "${out%/}" != "$out" ]; do out=${out%/}; done
# an absolute --bin, so the version we read and the binary we copy are the same file
if [ -n "$bin" ]; then bin="$(cd "$(dirname "$bin")" && pwd)/$(basename "$bin")"; fi

bundle_id() { plutil -extract CFBundleIdentifier raw -o - "$1/Contents/Info.plist" 2>/dev/null || true; }

check_out() {
  if [ -e "$out" ] || [ -L "$out" ]; then
    if [ -L "$out" ] || [ ! -d "$out" ] || [ "$(bundle_id "$out")" != dev.leon.app ]; then
      echo "bundle.sh: $out exists and is not a Leon.app (bundle id dev.leon.app); leaving it alone" >&2
      exit 1
    fi
  fi
}
check_out

if [ -z "$bin" ]; then
  echo "bundle.sh: cargo build --release -p leon"
  (cd "$repo" && cargo build --release -p leon)
  bin="$repo/target/release/leon"
fi
version=$(LEON_AUTOSTART=0 "$bin" --version)
version=${version#leon }
if ! printf '%s' "$version" | grep -Eq '^[0-9A-Za-z.+-]+$'; then
  echo "bundle.sh: $bin --version printed an odd version: $version" >&2
  exit 1
fi

mkdir -p "$(dirname "$out")"
# build next to the target, then swap it in, so a failed build never leaves half an app
stage=$(mktemp -d "$(dirname "$out")/.Leon.app.XXXXXX")
trap 'rm -rf "$stage"' EXIT
app="$stage/Leon.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin" "$app/Contents/MacOS/leon"
swift "$repo/scripts/icon.swift" "$stage/Leon.iconset"
iconutil -c icns -o "$app/Contents/Resources/Leon.icns" "$stage/Leon.iconset"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleDisplayName</key><string>Leon</string>
  <key>CFBundleExecutable</key><string>leon</string>
  <key>CFBundleIconFile</key><string>Leon</string>
  <key>CFBundleIdentifier</key><string>dev.leon.app</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>Leon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
plutil -lint -s "$app/Contents/Info.plist"
# ad-hoc signature: Apple Silicon won't launch an unsigned bundle
codesign --force --sign - "$app" 2>/dev/null || { echo "bundle.sh: codesign failed on $app" >&2; exit 1; }

check_out # again: the build can take minutes
rm -rf "$out"
mv "$app" "$out"
echo "bundle.sh: built $out (leon $version)"
