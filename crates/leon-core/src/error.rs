use serde::{Deserialize, Serialize};

/// An API error: an HTTP-like status, a stable machine code, and a message for people.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    pub status: u16,
    pub code: String,
    pub message: String,
}

impl ApiError {
    pub fn new(status: u16, code: &str, message: impl Into<String>) -> Self {
        ApiError { status, code: code.into(), message: message.into() }
    }
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(400, "bad_request", message)
    }
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(404, "not_found", message)
    }
    pub fn session_not_found(id: &str) -> Self {
        Self::new(404, "session_not_found", format!("no session {id}"))
    }
    pub fn method_not_found(name: &str) -> Self {
        Self::new(404, "method_not_found", format!("no method {name}"))
    }
    /// A query that named no target, or more than one: the CLI exits 3 for these (docs/cli.md).
    pub fn is_target_error(&self) -> bool {
        matches!(self.code.as_str(), "not_found" | "session_not_found" | "ambiguous")
    }

    pub fn ambiguous(query: &str, matches: &[String]) -> Self {
        Self::new(409, "ambiguous", format!("{query:?} matches more than one: {}", matches.join(", ")))
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ApiError {}
