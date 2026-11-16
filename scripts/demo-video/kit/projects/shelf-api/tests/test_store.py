import tempfile
import unittest
from pathlib import Path

from shelf.store import Store


class StoreTest(unittest.TestCase):
    def setUp(self):
        self.store = Store()

    def test_add_returns_the_book_with_an_id(self):
        book = self.store.add("Dune", "Frank Herbert", 1965)
        self.assertEqual(book["id"], 1)
        self.assertEqual(book["shelf"], "unsorted")

    def test_title_and_author_are_required(self):
        with self.assertRaises(ValueError):
            self.store.add("", "Someone")
        with self.assertRaises(ValueError):
            self.store.add("A title", None)

    def test_get_a_missing_book_is_none(self):
        self.assertIsNone(self.store.get(42))

    def test_all_keeps_insertion_order(self):
        for t in ["c", "a", "b"]:
            self.store.add(t, "x")
        self.assertEqual([b["title"] for b in self.store.all()], ["c", "a", "b"])

    def test_all_filters_by_shelf(self):
        self.store.add("One", "x", shelf="fiction")
        self.store.add("Two", "x", shelf="design")
        self.assertEqual([b["title"] for b in self.store.all("design")], ["Two"])

    def test_bulk_import_of_20000_books(self):
        for i in range(20000):
            self.store.add(f"Title {i}", f"Author {i % 300}", 1900 + i % 120, f"shelf-{i % 40}")
        self.assertEqual(self.store.count(), 20000)
        self.assertEqual(len(self.store.all("shelf-7")), 500)

    def test_a_file_database_keeps_its_books(self):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / "shelf.db"
            store = Store(str(path))
            for i in range(1500):
                store.add(f"Title {i}", "Author")
            store.db.close()
            self.assertEqual(Store(str(path)).count(), 1500)

    def test_remove(self):
        book = self.store.add("Gone", "x")
        self.assertTrue(self.store.remove(book["id"]))
        self.assertFalse(self.store.remove(book["id"]))
        self.assertEqual(self.store.count(), 0)
