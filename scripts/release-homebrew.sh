#!/usr/bin/env bash
# release-homebrew.sh: write the Homebrew tap's cask and formula from a release's manifest.
#
#   scripts/release-homebrew.sh [--url-base URL] DIR TAP
#
# DIR holds a release's latest.json; TAP is a checkout of the tap (aindong/homebrew-leon). Writes
# TAP/Casks/leon.rb (Leon.app, with the CLI inside it linked as `leon`) and TAP/Formula/leon.rb
# (the CLI alone, macOS and Linux) with the release's version, URLs and sha256 sums. It commits
# and pushes nothing. A release without both arches of the app and of each CLI tarball is refused.
#
# --url-base URL  fetch the files from URL/<name> instead of the release's download URLs: a
#                 file:// directory, to try a draft release from a local tap
#
# Needs jq.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
. "$repo/scripts/release-files.sh"
url_base=""
paths=()
while [ $# -gt 0 ]; do
  case "$1" in
    --url-base) url_base=${2:?--url-base needs a URL}; shift 2 ;;
    -h|--help) sed -n '2,14s/^# \{0,1\}//p' "$0"; exit 0 ;;
    -*) echo "release-homebrew.sh: unknown argument: $1 (try --help)" >&2; exit 2 ;;
    *) paths+=("$1"); shift ;;
  esac
done
[ ${#paths[@]} -eq 2 ] && [ -f "${paths[0]}/latest.json" ] && [ -d "${paths[1]}" ] \
  || { echo "release-homebrew.sh: give a directory with latest.json and the tap's checkout (try --help)" >&2; exit 2; }
need jq
manifest="${paths[0]}/latest.json" tap=${paths[1]}

# These values are written into Ruby strings that Homebrew runs on every user's machine: nothing
# that could close a string or interpolate (a quote, #{}, a backslash, a newline) gets through.
jq -e 'def plain(re): type == "string" and test(re) and (contains("\n") | not); # $ matches before a last newline
  (.version | plain("^[0-9A-Za-z.+-]+$"))
  and (.files | type == "array") and all(.files[];
    (.name | plain("^[A-Za-z0-9._-]+$")) and (.sha256 | plain("^[0-9a-f]{64}$"))
    and (.url | plain("^https://[A-Za-z0-9._/-]+$")))' "$manifest" >/dev/null \
  || { echo "release-homebrew.sh: latest.json has a version, name, sha256 or URL that is not plainly one" >&2; exit 1; }
plain_url='^(https|file)://[A-Za-z0-9._/-]+$' # [[ =~ ]] matches the whole value; grep would match any one line of it
if [ -n "$url_base" ] && ! [[ $url_base =~ $plain_url ]]; then
  echo "release-homebrew.sh: --url-base takes a plain https:// or file:// URL, not $url_base" >&2
  exit 1
fi
version=$(jq -r .version "$manifest")
# `field KIND OS ARCH FIELD`: one field of the one file that matches, or fail
field() {
  jq -er --arg k "$1" --arg o "$2" --arg a "$3" --arg f "$4" \
    '[.files[] | select(.kind == $k and .os == $o and .arch == $a)] | if length == 1 then .[0][$f] else empty end' "$manifest" \
    || { echo "release-homebrew.sh: the release has no $1 for $2 $3: the tap needs both arches" >&2; exit 1; }
}
# `url KIND OS ARCH`: where Homebrew downloads that file
url() {
  if [ -n "$url_base" ]; then echo "$url_base/$(field "$1" "$2" "$3" name)"; else field "$1" "$2" "$3" url; fi
}
app_arm=$(field app macos arm64 sha256)
app_intel=$(field app macos x86_64 sha256)
# every lookup runs before a file is written
block=""
for os in macos linux; do
  block+="  on_$os do"$'\n'
  for pair in arm:arm64 intel:x86_64; do
    block+="    on_${pair%%:*} do"$'\n'
    block+="      url \"$(url cli "$os" "${pair##*:}")\""$'\n'
    block+="      sha256 \"$(field cli "$os" "${pair##*:}" sha256)\""$'\n'
    block+="    end"$'\n'
  done
  block+="  end"$'\n'
  [ "$os" = linux ] || block+=$'\n'
done
# the cask spells one URL for both arches; the release has to be named that way
cask_base=${url_base:-"$RELEASE_REPO_URL/releases/download/v#{version}"}
for arch in arm64 x86_64; do
  name="Leon-$version-macos-$arch.zip"
  [ "$(field app macos "$arch" name)" = "$name" ] \
    || { echo "release-homebrew.sh: the app for $arch is not named $name" >&2; exit 1; }
  [ -n "$url_base" ] || [ "$(field app macos "$arch" url)" = "$RELEASE_REPO_URL/releases/download/v$version/$name" ] \
    || { echo "release-homebrew.sh: the manifest's URL for $name is not the one the cask would download" >&2; exit 1; }
done

mkdir -p "$tap/Casks" "$tap/Formula"
cat > "$tap/Casks/leon.rb" <<CASK
cask "leon" do
  arch arm: "arm64", intel: "x86_64"

  version "$version"
  sha256 arm:   "$app_arm",
         intel: "$app_intel"

  url "$cask_base/Leon-#{version}-macos-#{arch}.zip"
  name "Leon"
  desc "Terminal multiplexer with a native app, a CLI and a socket API"
  homepage "$RELEASE_REPO_URL"

  depends_on :macos

  app "Leon.app"
  binary "#{appdir}/Leon.app/Contents/MacOS/leon"

  caveats <<~EOS
    Leon is signed ad hoc and not notarized, so macOS will not open it until
    you remove the quarantine flag that Homebrew puts on every download:

      xattr -dr com.apple.quarantine "#{appdir}/Leon.app"

    The leon command is the binary inside the app, so it needs that too.
  EOS
end
CASK
cat > "$tap/Formula/leon.rb" <<FORMULA
class Leon < Formula
  desc "Fast, scriptable terminal multiplexer (server and CLI)"
  homepage "$RELEASE_REPO_URL"
  license "MIT"

$block
  def install
    bin.install "leon"
    doc.install "docs"
    prefix.install "THIRD-PARTY-NOTICES.txt"
  end

  test do
    assert_match "leon #{version}", shell_output("LEON_AUTOSTART=0 #{bin}/leon --version")
  end
end
FORMULA
echo "release-homebrew.sh: wrote Casks/leon.rb and Formula/leon.rb for leon $version in $tap"
