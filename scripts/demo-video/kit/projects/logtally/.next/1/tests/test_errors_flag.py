import unittest

import logtally
from tests.test_tally import LOG


class ErrorsFlagTest(unittest.TestCase):
    def test_4xx_count_when_asked(self):
        t = logtally.tally(LOG.splitlines(), include_4xx=True)
        self.assertAlmostEqual(t["error_rate"], 0.4)

    def test_404_is_never_an_error(self):
        self.assertFalse(logtally.is_error(404, include_4xx=True))
        self.assertTrue(logtally.is_error(429, include_4xx=True))

    def test_default_is_5xx_only(self):
        self.assertFalse(logtally.is_error(401))
