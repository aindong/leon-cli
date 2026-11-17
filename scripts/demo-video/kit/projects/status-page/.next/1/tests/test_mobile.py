import unittest

import build
from tests.test_build import SERVICES


class MobileTest(unittest.TestCase):
    def setUp(self):
        self.page = build.render(SERVICES)

    def test_the_page_sets_a_viewport(self):
        has = '<meta name="viewport" content="width=device-width, initial-scale=1">' in self.page
        self.assertTrue(has, "no viewport meta tag")

    def test_rows_stack_on_a_narrow_screen(self):
        self.assertTrue("@media (max-width: 600px)" in self.page, "no narrow-screen layout")
