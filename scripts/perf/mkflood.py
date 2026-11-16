"""Write the flood file: SIZE MB of mixed text and 16-colour SGR (no truecolor, so the harness's
truecolor sentinels never appear in it), seeded, so every run uses the same bytes.

python3 mkflood.py OUT [SIZE_MB]
"""
import random
import sys

out, size = sys.argv[1], int(sys.argv[2]) * 1_000_000 if len(sys.argv) > 2 else 100_000_000
rng = random.Random(7501)
words = "the quick brown fox jumps over a lazy dog cargo build test ok error warning compiling leon".split()
with open(out, "wb") as f:
    n = 0
    while n < size:
        parts = []
        for _ in range(rng.randint(3, 14)):
            w = rng.choice(words)
            if rng.random() < 0.3:
                w = f"\033[{rng.choice([1, 2, 3, 4, 22, 39])};{rng.choice(list(range(30, 38)) + list(range(90, 98)))}m{w}\033[0m"
            parts.append(w)
        line = (" ".join(parts) + "\n").encode()
        f.write(line)
        n += len(line)
