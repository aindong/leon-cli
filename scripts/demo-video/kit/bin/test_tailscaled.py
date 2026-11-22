"""test_tailscaled.py: the demo's LocalAPI stand-in answers what leon's parser reads
(crates/leon-hosts/src/localapi.rs: parse_status, parse_whois, admit), over a real Unix socket."""
import json
import os
import socket
import subprocess
import sys
import tempfile
import time

here = os.path.dirname(os.path.abspath(__file__))


def get(sock, path):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.connect(sock)
    s.sendall(f"GET {path} HTTP/1.0\r\nHost: local-tailscaled.sock\r\n\r\n".encode())
    got = b""
    while chunk := s.recv(65536):
        got += chunk
    head, body = got.split(b"\r\n\r\n", 1)
    return head.split(b"\r\n")[0].decode(), json.loads(body)


with tempfile.TemporaryDirectory() as d:
    sock = os.path.join(d, "ts", "tailscaled.sock")
    p = subprocess.Popen([sys.executable, os.path.join(here, "tailscaled.py"), sock], stdout=subprocess.DEVNULL)
    try:
        for _ in range(100):
            if os.path.exists(sock):
                break
            time.sleep(0.05)
        assert os.stat(os.path.dirname(sock)).st_mode & 0o077 == 0, "the socket's dir is private"
        line, st = get(sock, "/localapi/v0/status?peers=false")
        assert line == "HTTP/1.0 200 OK", line
        me = st["Self"]
        assert st["BackendState"] == "Running"
        assert me["ID"] and me["Tags"] is None and me["TailscaleIPs"] == ["127.0.0.1"]
        assert me["DNSName"].rstrip(".").endswith(".example-tailnet.ts.net")
        login = st["User"][str(me["UserID"])]["LoginName"]
        assert login.endswith("@example.com"), login
        line, who = get(sock, "/localapi/v0/whois?addr=127.0.0.1%3A50123")
        assert line == "HTTP/1.0 200 OK", line
        # admit: another node, untagged, the same user and login: one of the owner's other machines
        assert who["Node"]["StableID"] and who["Node"]["StableID"] != me["ID"]
        assert who["Node"]["Tags"] is None
        assert (who["UserProfile"]["ID"], who["UserProfile"]["LoginName"]) == (me["UserID"], login)
        line, _ = get(sock, "/localapi/v0/prefs")
        assert line.startswith("HTTP/1.0 404"), line
    finally:
        p.kill()
        p.wait()
print("test_tailscaled: ok")
