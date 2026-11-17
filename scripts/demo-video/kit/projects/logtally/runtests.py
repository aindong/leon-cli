"""runtests.py: run the unittest suite with one short line a test."""
import sys
import time
import unittest

G, R, D, B, Z = "\033[32m", "\033[31m", "\033[2m", "\033[1m", "\033[0m"


class Result(unittest.TestResult):
    def __init__(self):
        super().__init__()
        self.module = None
        self.passed = 0

    def startTest(self, test):
        super().startTest(test)
        module = type(test).__module__.replace(".", "/") + ".py"
        if module != self.module:
            self.module = module
            print(f"\n{B}{module}{Z}", flush=True)
        self.t = time.perf_counter()

    def line(self, test, mark, note=""):
        ms = (time.perf_counter() - getattr(self, "t", time.perf_counter())) * 1000
        name = getattr(test, "_testMethodName", str(test)).removeprefix("test_").replace("_", " ")
        print(f"  {mark} {name:<32} {D}{ms:5.0f} ms{Z}{note}", flush=True)

    def addSuccess(self, test):
        self.passed += 1
        self.line(test, f"{G}✓{Z}")

    def addFailure(self, test, err):
        super().addFailure(test, err)
        self.line(test, f"{R}✗{Z}", f"  {R}{str(err[1]).split(" : ")[-1]}{Z}")

    def addError(self, test, err):
        super().addError(test, err)
        self.line(test, f"{R}✗{Z}", f"  {R}{err[0].__name__}: {err[1]}{Z}")


if __name__ == "__main__":
    start = time.perf_counter()
    result = Result()
    unittest.defaultTestLoader.discover("tests", top_level_dir=".").run(result)
    failed = len(result.failures) + len(result.errors)
    took = time.perf_counter() - start
    summary = f"{G}{result.passed} passed{Z}" + (f", {R}{failed} failed{Z}" if failed else "")
    print(f"\n{B}{summary}{B} in {took:.2f} s{Z}")
    sys.exit(1 if failed else 0)
