"""test_proxy.py: the web demo's proxy (proxy.py) reaches only leon's port for the made-up name,
and passes each request on unchanged, so leon checks the page's own Host and Origin."""
import os
import socket
import subprocess
import sys
import threading
import time

here = os.path.dirname(os.path.abspath(__file__))
NAME = "studio-mac.example-tailnet.ts.net"


def free_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


def echo_server():
    """a stand-in for leon: answers each connection with the bytes it got"""
    srv = socket.socket()
    srv.bind(("127.0.0.1", 0))
    srv.listen(8)

    def serve():
        while True:
            c, _ = srv.accept()
            with c:
                got = b""
                while b"\r\n\r\n" not in got:
                    got += c.recv(4096)
                c.sendall(b"ECHO " + got)
    threading.Thread(target=serve, daemon=True).start()
    return srv.getsockname()[1]


def ask(port, data):
    s = socket.create_connection(("127.0.0.1", port))
    s.sendall(data)
    s.settimeout(3)
    got = b""
    try:
        while chunk := s.recv(65536):
            got += chunk
            if got.startswith(b"HTTP/1.1 200 Connection") and got.endswith(b"\r\n\r\n"):
                return got, s
    except socket.timeout:
        pass
    return got, s


leon = echo_server()
listen = free_port()
p = subprocess.Popen([sys.executable, os.path.join(here, "proxy.py"), str(listen), NAME, str(leon)], stdout=subprocess.DEVNULL)
try:
    for _ in range(100):
        try:
            socket.create_connection(("127.0.0.1", listen)).close()
            break
        except OSError:
            time.sleep(0.05)
    req = (f"GET /ws?token=t HTTP/1.1\r\nHost: {NAME}:{leon}\r\nOrigin: http://{NAME}:{leon}\r\n"
           "Upgrade: websocket\r\n\r\n").encode()
    # CONNECT: an established tunnel, then the request reaches leon byte for byte
    got, s = ask(listen, f"CONNECT {NAME}:{leon} HTTP/1.1\r\nHost: {NAME}:{leon}\r\n\r\n".encode())
    assert got == b"HTTP/1.1 200 Connection established\r\n\r\n", got
    s.sendall(req)
    echoed = s.recv(65536)
    s.close()
    assert echoed == b"ECHO " + req, echoed
    # a plain proxy request for the name: passed on as it came
    plain = f"GET http://{NAME}:{leon}/ HTTP/1.1\r\nHost: {NAME}:{leon}\r\n\r\n".encode()
    got, s = ask(listen, plain)
    s.close()
    assert got == b"ECHO " + plain, got
    # anything else: refused, never connected
    for other in (f"CONNECT example.com:443 HTTP/1.1\r\n\r\n", f"CONNECT {NAME}:22 HTTP/1.1\r\n\r\n",
                  "GET http://127.0.0.1:22/ HTTP/1.1\r\n\r\n"):
        got, s = ask(listen, other.encode())
        s.close()
        assert got.startswith(b"HTTP/1.1 403"), (other, got)
finally:
    p.kill()
    p.wait()
print("test_proxy: ok")
