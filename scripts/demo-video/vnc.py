#!/usr/bin/env python3
"""vnc.py URL CMD...: drive a Tart VM's VNC server (tart run --vnc-experimental) with no window on the host.

  shot PATH          save the screen as PNG
  click X Y          left click at framebuffer pixel X,Y
  move X Y
  type TEXT          type ASCII text
  key NAME [MOD...]  press a key (return tab esc space backspace, a letter) with mods cmd/shift/ctrl/alt
Commands chain: vnc.py URL click 10 10 type hello key return shot a.png
Stdlib only; VNC auth's DES comes from the openssl CLI.
"""
import socket, struct, subprocess, sys, time, urllib.parse, zlib

KEYS = {"return": 0xFF0D, "tab": 0xFF09, "esc": 0xFF1B, "space": 0x20, "backspace": 0xFF08,
        "left": 0xFF51, "up": 0xFF52, "right": 0xFF53, "down": 0xFF54, "f2": 0xFFBF}
MODS = {"shift": 0xFFE1, "ctrl": 0xFFE3, "alt": 0xFFE9, "cmd": 0xFFEB}


def des(key8, data):
    k = bytes(int(f"{b:08b}"[::-1], 2) for b in key8)  # VNC reverses each key byte's bits
    return subprocess.run(["/usr/bin/openssl", "enc", "-des-ecb", "-nopad", "-K", k.hex()],
                          input=data, capture_output=True, check=True).stdout


class VNC:
    def __init__(self, url):
        u = urllib.parse.urlparse(url)
        self.s = socket.create_connection((u.hostname, u.port), timeout=30)
        self.s.recv(12)
        self.s.sendall(b"RFB 003.008\n")
        types = self.read(self.read(1)[0])
        if 2 in types:
            self.s.sendall(b"\x02")
            pw = (u.password or "").encode()[:8].ljust(8, b"\0")
            self.s.sendall(des(pw, self.read(16)))
        else:
            self.s.sendall(bytes([types[0]]))
        if struct.unpack(">I", self.read(4))[0] != 0:
            sys.exit("vnc: auth failed")
        self.s.sendall(b"\x01")
        self.w, self.h = struct.unpack(">HH", self.read(4))
        self.read(16)
        self.read(struct.unpack(">I", self.read(4))[0])
        self.s.sendall(struct.pack(">BxxxBBBBHHHBBBxxx", 0, 32, 24, 0, 1, 255, 255, 255, 16, 8, 0))
        # raw, DesktopSize, ExtendedDesktopSize: the VZ server aborts the VM without the pseudo-encodings
        self.s.sendall(struct.pack(">BxHiii", 2, 3, 0, -223, -308))

    def read(self, n):
        b = b""
        while len(b) < n:
            c = self.s.recv(n - len(b))
            if not c:
                sys.exit("vnc: closed")
            b += c
        return b

    def shot(self, path):
        self.s.sendall(struct.pack(">BBHHHH", 3, 0, 0, 0, self.w, self.h))
        fb = bytearray(self.w * self.h * 4)
        while True:
            t = self.read(1)[0]
            if t == 2:
                continue
            if t == 3:
                self.read(3); self.read(struct.unpack(">I", self.read(4))[0]); continue
            if t != 0:
                sys.exit(f"vnc: unexpected message {t}")
            self.read(1)
            pixels = False
            for _ in range(struct.unpack(">H", self.read(2))[0]):
                x, y, w, h, enc = struct.unpack(">HHHHi", self.read(12))
                if enc in (-223, -308):
                    if enc == -308:
                        self.read(16 * self.read(4)[0])
                    if (w, h) != (self.w, self.h):
                        self.w, self.h, fb = w, h, bytearray(w * h * 4)
                    continue
                if enc != 0:
                    sys.exit(f"vnc: unexpected encoding {enc}")
                pixels = True
                data = self.read(w * h * 4)
                for r in range(h):
                    o = ((y + r) * self.w + x) * 4
                    fb[o:o + w * 4] = data[r * w * 4:(r + 1) * w * 4]
            if pixels:
                break
            self.s.sendall(struct.pack(">BBHHHH", 3, 0, 0, 0, self.w, self.h))
        raw = bytearray()
        for r in range(self.h):
            row = fb[r * self.w * 4:(r + 1) * self.w * 4]
            raw += b"\0"
            rgb = bytearray(self.w * 3)
            rgb[0::3], rgb[1::3], rgb[2::3] = row[2::4], row[1::4], row[0::4]  # BGRX little-endian
            raw += rgb
        def chunk(k, d):
            return struct.pack(">I", len(d)) + k + d + struct.pack(">I", zlib.crc32(k + d))
        png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", self.w, self.h, 8, 2, 0, 0, 0))
        open(path, "wb").write(png + chunk(b"IDAT", zlib.compress(bytes(raw), 6)) + chunk(b"IEND", b""))

    def ptr(self, x, y, mask):
        self.s.sendall(struct.pack(">BBHH", 5, mask, x, y))

    def key(self, sym, down):
        self.s.sendall(struct.pack(">BBxxI", 4, int(down), sym))


def main(argv):
    v, a = VNC(argv[0]), argv[1:]
    while a:
        c = a.pop(0)
        if c == "shot":
            v.shot(a.pop(0))
        elif c in ("click", "move"):
            x, y = int(a.pop(0)), int(a.pop(0))
            v.ptr(x, y, 0); time.sleep(0.15)
            if c == "click":
                v.ptr(x, y, 1); time.sleep(0.08); v.ptr(x, y, 0)
        elif c == "type":
            for ch in a.pop(0):
                v.key(ord(ch), True); v.key(ord(ch), False); time.sleep(0.03)
        elif c == "key":
            name, mods = a.pop(0), []
            while a and a[0] in MODS:
                mods.append(MODS[a.pop(0)])
            sym = KEYS.get(name, ord(name[0]) if len(name) == 1 else None)
            for m in mods:
                v.key(m, True)
            v.key(sym, True); time.sleep(0.05); v.key(sym, False)
            for m in reversed(mods):
                v.key(m, False)
        elif c == "sleep":
            time.sleep(float(a.pop(0)))
        else:
            sys.exit(f"vnc: unknown command {c}")
        time.sleep(0.2)


if __name__ == "__main__":
    main(sys.argv[1:])
