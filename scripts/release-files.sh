# shellcheck shell=bash
# Sourced by the other scripts/release-*.sh: where releases are published, the files a full release
# has, a sha256 that runs on macOS and Linux, and a check for the tools a script needs.
# shellcheck disable=SC2034 # read by the scripts that source this file
RELEASE_REPO_URL="https://github.com/aindong/leon-cli"

# The eight files of a full release of version $1, one per line: name kind os arch.
release_files() {
  cat <<LIST
Leon-$1-macos-arm64.zip app macos arm64
Leon-$1-macos-x86_64.zip app macos x86_64
leon-$1-arm64-macos.tar.gz cli macos arm64
leon-$1-x86_64-macos.tar.gz cli macos x86_64
leon-$1-aarch64-linux.tar.gz cli linux arm64
leon-$1-x86_64-linux.tar.gz cli linux x86_64
leon_$1_arm64.deb deb linux arm64
leon_$1_amd64.deb deb linux x86_64
LIST
}

# `sha256 FILE…` prints "<hash>  <name>" lines.
sha256() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$@"; else shasum -a 256 "$@"; fi
}

need() {
  local tool
  for tool in "$@"; do
    command -v "$tool" >/dev/null 2>&1 || { echo "$(basename "$0"): $tool is needed (brew install $tool, or apt install $tool)" >&2; exit 1; }
  done
}
