"""The book store: a small SQLite table behind the API."""
import sqlite3
import threading


class Store:
    def __init__(self, path=":memory:"):
        self.db = sqlite3.connect(path, check_same_thread=False)
        self.db.row_factory = sqlite3.Row
        # one connection serves every request thread: one statement at a time
        self.lock = threading.RLock()
        self.db.execute(
            "CREATE TABLE IF NOT EXISTS books ("
            " id INTEGER PRIMARY KEY, title TEXT NOT NULL, author TEXT NOT NULL,"
            " year INTEGER, shelf TEXT DEFAULT 'unsorted')"
        )

    def add(self, title, author, year=None, shelf="unsorted"):
        if not title or not author:
            raise ValueError("title and author are required")
        with self.lock:
            cur = self.db.execute(
                "INSERT INTO books (title, author, year, shelf) VALUES (?, ?, ?, ?)",
                (title, author, year, shelf),
            )
            self.db.commit()
            return self.get(cur.lastrowid)

    def get(self, book_id):
        with self.lock:
            row = self.db.execute("SELECT * FROM books WHERE id = ?", (book_id,)).fetchone()
        return dict(row) if row else None

    def all(self, shelf=None):
        with self.lock:
            if shelf:
                rows = self.db.execute("SELECT * FROM books WHERE shelf = ? ORDER BY id", (shelf,))
            else:
                rows = self.db.execute("SELECT * FROM books ORDER BY id")
            return [dict(r) for r in rows]

    def remove(self, book_id):
        with self.lock:
            cur = self.db.execute("DELETE FROM books WHERE id = ?", (book_id,))
            self.db.commit()
            return cur.rowcount == 1

    def count(self):
        with self.lock:
            return self.db.execute("SELECT COUNT(*) FROM books").fetchone()[0]
