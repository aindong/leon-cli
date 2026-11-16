//! A release server on loopback for the update tests: it serves a directory, records every
//! request, and can be told to misbehave for one file.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// How the server answers for one file instead of serving it.
#[derive(Clone)]
pub enum Mode {
    /// This status, with an error page for a body.
    Status(u16),
    /// 200 with these bytes instead of the file's.
    Bytes(Vec<u8>),
    /// The file's length in the header, half its bytes, then the connection closes.
    Truncated,
    /// 200 with no length, then zeros until the client hangs up.
    Endless,
    /// The header and the first half of the file, then nothing until the client hangs up.
    Stall,
    /// 302 to this address.
    Redirect(String),
}

pub struct Server {
    pub port: u16,
    modes: Arc<Mutex<HashMap<String, Mode>>>,
    requests: Arc<Mutex<Vec<String>>>,
    sent: Arc<AtomicU64>,
}

impl Server {
    /// Serve `dir` on a free loopback port until the test ends.
    pub fn start(dir: &Path) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let s = Server { port, modes: Arc::default(), requests: Arc::default(), sent: Arc::default() };
        let (dir, modes, requests, sent) = (dir.to_path_buf(), s.modes.clone(), s.requests.clone(), s.sent.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (dir, modes, requests, sent) = (dir.clone(), modes.clone(), requests.clone(), sent.clone());
                std::thread::spawn(move || serve(stream, &dir, &modes, &requests, &sent));
            }
        });
        s
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Answer requests for `name` with `mode` from now on.
    pub fn set(&self, name: &str, mode: Mode) {
        self.modes.lock().unwrap().insert(name.to_string(), mode);
    }

    /// Serve `name` as it is again.
    pub fn clear(&self, name: &str) {
        self.modes.lock().unwrap().remove(name);
    }

    /// Every request so far: its whole head, as the client sent it.
    pub fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }

    /// The file names asked for so far, in order.
    pub fn asked(&self) -> Vec<String> {
        self.requests()
            .iter()
            .filter_map(|r| Some(r.split_whitespace().nth(1)?.trim_start_matches('/').to_string()))
            .collect()
    }

    /// Body bytes written so far, over all requests.
    pub fn sent(&self) -> u64 {
        self.sent.load(Ordering::SeqCst)
    }
}

fn serve(
    mut stream: TcpStream,
    dir: &Path,
    modes: &Mutex<HashMap<String, Mode>>,
    requests: &Mutex<Vec<String>>,
    sent: &AtomicU64,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") && head.len() < 16 << 10 {
        match stream.read(&mut byte) {
            Ok(1) => head.push(byte[0]),
            _ => return,
        }
    }
    let head = String::from_utf8_lossy(&head).into_owned();
    let name = head.split_whitespace().nth(1).unwrap_or("/").trim_start_matches('/').to_string();
    requests.lock().unwrap().push(head);
    let mode = modes.lock().unwrap().get(&name).cloned();
    let file = std::fs::read(dir.join(&name)).ok();
    let body = |stream: &mut TcpStream, bytes: &[u8]| {
        let ok = stream.write_all(bytes).is_ok();
        if ok {
            sent.fetch_add(bytes.len() as u64, Ordering::SeqCst);
        }
        ok
    };
    let head_for = |status: &str, len: Option<usize>| {
        let len = len.map(|n| format!("Content-Length: {n}\r\n")).unwrap_or_default();
        format!("HTTP/1.1 {status}\r\n{len}Connection: close\r\n\r\n")
    };
    match (mode, file) {
        (Some(Mode::Status(code)), _) => {
            // with an error page, as real servers send: longer than a signature may be
            let page = "<p>told to answer this</p>\n".repeat(400);
            let _ = stream.write_all(head_for(&format!("{code} Told To"), Some(page.len())).as_bytes());
            let _ = stream.write_all(page.as_bytes());
        }
        (Some(Mode::Redirect(to)), _) => {
            let _ = stream.write_all(
                format!("HTTP/1.1 302 Found\r\nLocation: {to}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            );
        }
        (Some(Mode::Bytes(bytes)), _) => {
            let _ = stream.write_all(head_for("200 OK", Some(bytes.len())).as_bytes());
            body(&mut stream, &bytes);
        }
        (Some(Mode::Endless), _) => {
            let _ = stream.write_all(head_for("200 OK", None).as_bytes());
            let chunk = [0u8; 64 << 10];
            // a client that never hangs up fails its test long before 4 GiB
            while sent.load(Ordering::SeqCst) < 4 << 30 && body(&mut stream, &chunk) {}
        }
        (Some(Mode::Truncated), Some(file)) => {
            let _ = stream.write_all(head_for("200 OK", Some(file.len())).as_bytes());
            body(&mut stream, &file[..file.len() / 2]);
        }
        (Some(Mode::Stall), Some(file)) => {
            let _ = stream.write_all(head_for("200 OK", Some(file.len())).as_bytes());
            body(&mut stream, &file[..file.len() / 2]);
            let _ = stream.set_read_timeout(Some(Duration::from_secs(120)));
            let _ = stream.read(&mut byte); // until the client hangs up
        }
        (None, Some(file)) => {
            let _ = stream.write_all(head_for("200 OK", Some(file.len())).as_bytes());
            body(&mut stream, &file);
        }
        (_, None) => {
            let _ = stream.write_all(head_for("404 Not Found", Some(0)).as_bytes());
        }
    }
}
