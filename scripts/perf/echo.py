"""The key-to-pixel echo program: raw mode, and for each byte read a truecolor bar at the top left
that flips between red and green, then the cursor moves to a new column on row 3. The bar is what
winframes sees from outside; the cursor moving is what the app's LEON_FRAME_LOG sees inside.

python3 echo.py [--delay MS]   MS: wait before each echo (a known delay the clocks must see)
"""
import os
import signal
import sys
import termios
import time
import tty

delay = float(sys.argv[sys.argv.index("--delay") + 1]) / 1000 if "--delay" in sys.argv else 0
RED, GREEN = "255;0;0", "0;255;0"


def draw(i):
    colour = GREEN if i % 2 else RED
    # the whole screen each time: a resize can leave an old bar behind for the probe to find
    os.write(1, f"\033[H\033[2J\033[48;2;{colour}m{' ' * 12}\033[0m\033[3;{1 + i % 60}H".encode())


fd = sys.stdin.fileno()
old = termios.tcgetattr(fd)
tty.setraw(fd)
try:
    i = 0
    signal.signal(signal.SIGWINCH, lambda *_: draw(i))
    draw(0)
    while True:
        b = os.read(fd, 64)
        if not b or b"\x03" in b:
            break
        for _ in b:
            i += 1
            if delay:
                time.sleep(delay)
            draw(i)
finally:
    termios.tcsetattr(fd, termios.TCSADRAIN, old)
