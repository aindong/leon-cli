"""genlog.py N [SEED]: write N lines of a plausible access log to stdout."""
import random
import sys

PATHS = ["/books", "/books/1", "/books/2", "/health", "/search", "/login", "/books/17", "/cart"]
WEIGHTS = [30, 12, 9, 20, 10, 6, 4, 9]


def lines(n, seed=7):
    rng = random.Random(seed)
    for _ in range(n):
        path = rng.choices(PATHS, WEIGHTS)[0]
        roll = rng.random()
        status = 500 if roll < 0.004 else 404 if roll < 0.03 else 429 if roll < 0.035 else 200
        ms = rng.lognormvariate(2.4, 0.7) * (8 if path == "/search" else 1)
        yield f"{rng.choice(['GET', 'GET', 'GET', 'POST'])} {path} {status} {ms:.1f}ms\n"


if __name__ == "__main__":
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 1000
    sys.stdout.writelines(lines(n, int(sys.argv[2]) if len(sys.argv) > 2 else 7))
