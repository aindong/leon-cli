#!/usr/bin/env python3
"""Linux Docker fixture: a uid-1000 server, uid-1000 positive control, uid-1001 rejection."""
import json
import os
import socket
import subprocess
import sys
import tempfile
import time


def identity(uid):
    os.setgroups([])
    os.setgid(uid)
    os.setuid(uid)


def probe(path, uid):
    # Fork so each probe changes identity for real without changing the controller.
    reader, writer = os.pipe()
    pid = os.fork()
    if pid == 0:
        os.close(reader)
        identity(uid)
        try:
            with socket.socket(socket.AF_UNIX) as s:
                s.settimeout(3)
                s.connect(path)
                s.sendall(b'{"id":1,"method":"server.version","params":{}}\n')
                response = s.recv(65536)
                accepted = bool(response and "result" in json.loads(response))
                os.write(writer, b"yes" if accepted else b"no")
        except (ConnectionResetError, BrokenPipeError):
            os.write(writer, b"no")
        except Exception as error:
            os.write(writer, repr(error).encode())
        os._exit(0)
    os.close(writer)
    result = os.read(reader, 65536)
    os.close(reader)
    os.waitpid(pid, 0)
    assert result in (b"yes", b"no"), f"probe could not reach server: {result!r}"
    return result == b"yes"


assert os.geteuid() == 0, "run only inside the Docker fixture"
with tempfile.TemporaryDirectory(prefix="leon-peer-") as root:
    os.chmod(root, 0o755)
    runtime = os.path.join(root, "runtime")
    state = os.path.join(root, "state")
    for directory in (runtime, state):
        os.mkdir(directory, 0o700)
        os.chown(directory, 1000, 1000)
    env = dict(os.environ, LEON_RUNTIME_DIR=runtime, LEON_STATE_DIR=state,
               LEON_CONFIG=os.path.join(root, "no-init.lua"), SHELL="/bin/sh", HOME=root)
    for key in ("LEON_SERVER", "LEON_SESSION", "LEON_BLOCK", "LEON_IDLE_EXIT_SECS"):
        env.pop(key, None)
    with open(os.path.join(root, "server.log"), "w+") as log:
        process = subprocess.Popen([sys.argv[1], "server", "run"], env=env,
                                   preexec_fn=lambda: identity(1000), stdout=log, stderr=log)
        try:
            path = os.path.join(runtime, "server.sock")
            deadline = time.monotonic() + 10
            while not os.path.exists(path):
                assert process.poll() is None, "server exited"
                assert time.monotonic() < deadline, "server readiness deadline"
                time.sleep(0.02)
            # Deliberately let a second uid connect: otherwise filesystem permissions mask
            # a missing SO_PEERCRED check. No production permission policy is altered.
            os.chmod(runtime, 0o755)
            os.chmod(path, 0o666)
            assert probe(path, 1000), "same uid was rejected"
            assert not probe(path, 1001), "different uid was accepted"
            print("GREEN: same uid accepted; different uid refused despite socket access", flush=True)
        finally:
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=3)
