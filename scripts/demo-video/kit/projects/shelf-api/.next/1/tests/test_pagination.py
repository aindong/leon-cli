import unittest

from tests.test_http import ApiCase


class PaginationTest(ApiCase):
    def test_first_page_has_twenty_books(self):
        _, body = self.call("/books")
        self.assertEqual(len(body["books"]), 20)
        self.assertEqual(body["next"], 20)

    def test_limit_and_offset(self):
        _, body = self.call("/books?limit=5&offset=10")
        self.assertEqual([b["title"] for b in body["books"]][0], "Book 10")
        self.assertEqual(len(body["books"]), 5)

    def test_last_page_has_no_next(self):
        _, body = self.call("/books?limit=100")
        self.assertIsNone(body["next"])

    def test_a_limit_over_100_is_400(self):
        self.assertEqual(self.call("/books?limit=500")[0], 400)

    def test_total_counts_every_book(self):
        _, body = self.call("/books?limit=1")
        self.assertGreaterEqual(body["total"], 30)

