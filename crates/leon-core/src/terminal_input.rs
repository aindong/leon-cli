//! Input for a terminal's program: text, a paste, a key, and when input last came.

use super::*;

impl Terminal {
    /// Queue input for the program; never blocks.
    pub fn write(&self, data: &[u8]) -> io::Result<()> {
        self.write_at(data).map(drop)
    }

    /// `write`, returning the instant it was stamped with (`last_input`).
    pub fn write_at(&self, data: &[u8]) -> io::Result<std::time::Instant> {
        let at = std::time::Instant::now();
        *lock(&self.last_input) = Some(at);
        self.writer.input(data.to_vec()).map(|_| at)
    }

    /// When input was last queued for the program, from any client.
    pub fn last_input(&self) -> Option<std::time::Instant> {
        *lock(&self.last_input)
    }

    /// Whether the program asked for bracketed paste (a line editor does while it reads a line).
    pub fn bracketed_paste(&self) -> bool {
        lock(&self.term).mode().contains(TermMode::BRACKETED_PASTE)
    }

    /// Paste `text` the way the program asked for it now (`keys::paste_bytes`).
    pub fn paste(&self, text: &str) -> io::Result<()> {
        self.write(&crate::keys::paste_bytes(text, self.bracketed_paste()))
    }

    /// Press one key given in binding syntax (`enter`, `ctrl+c`, `up`).
    pub fn key(&self, key: &str) -> Result<(), String> {
        let app_cursor = lock(&self.term).mode().contains(TermMode::APP_CURSOR);
        let bytes = crate::keys::encode(key, app_cursor)?;
        self.write(&bytes).map_err(|e| e.to_string())
    }
}
