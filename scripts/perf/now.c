// now: the boot clock (CLOCK_UPTIME_RAW) in nanoseconds, the clock every perf log uses. A C
// program so a stamp costs about a millisecond, not an interpreter's start.
#include <stdio.h>
#include <time.h>
int main(void) {
  printf("%llu\n", (unsigned long long)clock_gettime_nsec_np(CLOCK_UPTIME_RAW));
  return 0;
}
