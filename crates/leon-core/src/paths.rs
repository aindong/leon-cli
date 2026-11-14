use std::path::PathBuf;

/// Where the server's socket lives: `$LEON_RUNTIME_DIR`, else `$XDG_RUNTIME_DIR/leon`,
/// else `~/.local/state/leon/run`.
pub fn runtime_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("LEON_RUNTIME_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(d);
    }
    if let Some(d) = std::env::var_os("XDG_RUNTIME_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(d).join("leon");
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join(".local/state/leon/run")
}

pub fn socket_path() -> PathBuf {
    runtime_dir().join("server.sock")
}
