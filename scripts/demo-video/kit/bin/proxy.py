"""proxy.py PORT NAME LEON_PORT: the web demo viewer's way to the made-up MagicDNS name.

The VM can't resolve NAME (the tailnet is made up, and /etc/hosts needs root), so the viewer
(viewer.swift, an HTTP CONNECT proxy) sends its requests through this proxy on 127.0.0.1:PORT. A
CONNECT to NAME:LEON_PORT is tunnelled to 127.0.0.1:LEON_PORT and nothing in it is changed, so leon
sees the page's own Host (NAME:LEON_PORT) and Origin, and checks them and the token as ever.
Anything else is refused: the proxy reaches leon's port on this machine and nothing more.
"""
import socket
import sys
import threading


def pipe(a, b):
    try:
        while data := a.recv(65536):
            b.sendall(data)
    except OSError:
        pass
    finally:
        for s in (a, b):
            try:
                s.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass


def handle(client, name, port):
    with client:
        client.settimeout(10)
        head = b""
        while b"\r\n\r\n" not in head:
            got = client.recv(4096)
            if not got or len(head) > 16384:
                return
            head += got
        line = head.split(b"\r\n", 1)[0].decode("latin-1").split(" ")
        if len(line) != 3:
            return
        method, target = line[0], line[1]
        want = f"{name}:{port}"
        if method == "CONNECT" and target.lower() == want:
            rest, reply = head.split(b"\r\n\r\n", 1)[1], b"HTTP/1.1 200 Connection established\r\n\r\n"
        else:
            client.sendall(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            return
        client.settimeout(None)
        leon = socket.create_connection(("127.0.0.1", port))
        client.sendall(reply)
        if rest:
            leon.sendall(rest)
        t = threading.Thread(target=pipe, args=(leon, client), daemon=True)
        t.start()
        pipe(client, leon)
        t.join()


def main(listen, name, port):
    srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    srv.bind(("127.0.0.1", listen))
    srv.listen(32)
    print(f"proxy.py: {name}:{port} through 127.0.0.1:{listen}", flush=True)
    while True:
        c, _ = srv.accept()
        threading.Thread(target=handle, args=(c, name.lower(), port), daemon=True).start()


if __name__ == "__main__":
    main(int(sys.argv[1]), sys.argv[2], int(sys.argv[3]))
