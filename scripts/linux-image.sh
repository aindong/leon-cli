# Sourced by scripts/linux-test.sh and scripts/package-linux.sh: the pinned Rust image and the
# Docker arch. Bump IMAGE (tag and its multi-arch index digest) together.
IMAGE="rust:1.100.0-bookworm@sha256:ade39a710ef85f3506cffcfec3e8a24ba090badef1dff2fb798f71d878833fea"

# `--arch arm64|amd64` (default: the Docker host's); sets ARCH and PLATFORM, leaves other args in ARGS.
ARCH=""
ARGS=()
while [ $# -gt 0 ]; do
    case "$1" in
        --arch) ARCH="${2:?--arch needs arm64 or amd64}"; shift 2 ;;
        *) ARGS+=("$1"); shift ;;
    esac
done
if [ -z "$ARCH" ]; then
    case "$(docker info --format '{{.Architecture}}')" in
        aarch64|arm64) ARCH=arm64 ;;
        x86_64|amd64) ARCH=amd64 ;;
        *) echo "unknown Docker arch; pass --arch arm64|amd64" >&2; exit 2 ;;
    esac
fi
case "$ARCH" in arm64|amd64) ;; *) echo "--arch must be arm64 or amd64, not $ARCH" >&2; exit 2 ;; esac
PLATFORM="linux/$ARCH"

# Extra `docker run` arguments (package-linux.sh mounts its output dir at /out), and Debian
# packages to install first (linux-test.sh wants zsh, so zle's paths are tested too).
DOCKER_ARGS=()
PACKAGES=

# Run "$@" in the image as uid 1000 (not root, so the private-dir and peer-uid checks are real), the
# repo read-only at /src, the cargo registry and this arch's target dir in named volumes.
in_image() {
    # --init: a real init reaps orphans; without one a killed background job stays a zombie
    local run=(docker run --rm --init --platform "$PLATFORM" -v "$PWD":/src:ro
        -v leon-cargo-registry:/usr/local/cargo/registry -v "leon-linux-target-$ARCH":/target
        -e CARGO_TARGET_DIR=/target -e CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-auto}" -w /src -e "PACKAGES=$PACKAGES" ${DOCKER_ARGS[@]+"${DOCKER_ARGS[@]}"})
    [ -t 1 ] && run+=(-t)
    "${run[@]}" "$IMAGE" bash -c '
        set -e
        useradd -m -u 1000 leon 2>/dev/null || true
        rustup component add clippy 2>/dev/null
        if [ -n "$PACKAGES" ]; then
            apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq $PACKAGES >/dev/null
        fi
        chown leon /usr/local/cargo/registry /target
        chown -R leon /usr/local/cargo/registry /target 2>/dev/null || true
        if [ -d /out ]; then chown leon /out; fi
        exec runuser -u leon -- env HOME=/home/leon USER=leon "$@"' bash "$@"
}
