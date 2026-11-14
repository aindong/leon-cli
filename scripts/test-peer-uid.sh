#!/usr/bin/env bash
# Prove peer credentials, independently of socket permissions, including a mutation red run.
# Reuses linux-test.sh's pinned image and target/registry caches. Only the container copy mutates.
set -euo pipefail
cd "$(dirname "$0")/.."
. scripts/linux-image.sh

docker run --rm --init --platform "$PLATFORM" -v "$PWD":/src:ro \
    -v leon-cargo-registry:/usr/local/cargo/registry -v "leon-linux-target-$ARCH":/target \
    -e CARGO_TARGET_DIR=/target -w /work "$IMAGE" bash -c '
set -euo pipefail
apt-get update -qq
apt-get install -y -qq python3 >/dev/null
# Keep the repository read-only and mutate only this disposable source tree.
tar -C /src --exclude=.git --exclude=target --exclude=.agent -cf - . | tar -C /work -xf -
cargo build --locked -p leon --no-default-features
python3 scripts/peer-uid.py /target/debug/leon
python3 - <<"PY"
p = "crates/leon-server/src/serve.rs"
s = open(p).read()
old = "stream.peer_cred().is_ok_and(|c| c.uid() == unsafe { libc::geteuid() })"
assert s.count(old) == 1, "peer-check mutation must match exactly once"
open(p, "w").write(s.replace(old, "{ let _ = stream; true }"))
PY
cargo build --locked -p leon --no-default-features
set +e
python3 scripts/peer-uid.py /target/debug/leon > /tmp/peer-red.txt 2>&1
rc=$?
set -e
cat /tmp/peer-red.txt
[ "$rc" -ne 0 ]
grep -q "different uid was accepted" /tmp/peer-red.txt
echo "RED CONFIRMED: disabling same_user makes the peer-uid assertion fail"
'
