"""tailscaled.py SOCKET: a stand-in for tailscaled's LocalAPI, for the web demo (kit/web.sh).

It answers the two calls leon makes, `status?peers=false` and `whois`, on a Unix socket, the way
leon's own tests' fake does (crates/leon/tests/support/localapi.rs). leon finds it only through
the debug-only seam LEON_TEST_TAILNET, which the demo's own build has
(scripts/demo-video/run-demo.sh webapp); a release build has no code that reads it. The tailnet
is made up: this machine is leon-demo.example-tailnet.ts.net, signed in as demo@example.com,
and every caller is "laptop", another of that owner's machines. So the page in the viewer counts
as another machine, as it would over a real tailnet, while leon's admit, Host, Origin and token
checks run unchanged.
"""
import json
import os
import socket
import sys
import threading

OWNER, USER = "demo@example.com", 7
STATUS = {
    "BackendState": "Running",
    "Self": {"ID": "nleondemo", "UserID": USER, "TailscaleIPs": ["127.0.0.1"], "Tags": None,
             "DNSName": "leon-demo.example-tailnet.ts.net.", "OS": "macOS"},
    "User": {str(USER): {"ID": USER, "LoginName": OWNER}},
}
WHOIS = {
    "Node": {"ID": 2, "StableID": "nlaptop", "Name": "laptop.example-tailnet.ts.net.", "User": USER, "Tags": None},
    "UserProfile": {"ID": USER, "LoginName": OWNER, "DisplayName": "Demo"},
}


def answer(path):
    """(code, body) for a request path"""
    if path == "/localapi/v0/status?peers=false":
        return 200, STATUS
    if path.startswith("/localapi/v0/whois?addr="):
        return 200, WHOIS
    return 404, {"error": "the demo's stand-in answers status and whois only"}


def serve_one(conn):
    with conn:
        conn.settimeout(5)
        head = b""
        while b"\r\n\r\n" not in head and len(head) < 8192:
            got = conn.recv(4096)
            if not got:
                return
            head += got
        parts = head.split(b"\r\n", 1)[0].decode("latin-1").split(" ")
        code, body = answer(parts[1] if len(parts) == 3 and parts[0] == "GET" else "")
        data = json.dumps(body).encode()
        conn.sendall(b"HTTP/1.0 %d %s\r\nContent-Type: application/json\r\nContent-Length: %d\r\n\r\n%s"
                     % (code, b"OK" if code == 200 else b"Not Found", len(data), data))


def main(path):
    # leon checks the socket's peer is this user (the seam's rule); the dir keeps others out
    os.makedirs(os.path.dirname(path), mode=0o700, exist_ok=True)
    os.chmod(os.path.dirname(path), 0o700)
    if os.path.exists(path):
        os.unlink(path)
    srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    srv.bind(path)
    srv.listen(16)
    print(f"tailscaled.py: listening on {path}", flush=True)
    while True:
        conn, _ = srv.accept()
        threading.Thread(target=serve_one, args=(conn,), daemon=True).start()


if __name__ == "__main__":
    main(sys.argv[1])
