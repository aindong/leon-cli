//! The web page's WebSocket (docs/architecture/web.md): RFC 6455 framing over a connection
//! `web.rs` upgraded, and a bridge that hands each text message to `serve::connection` as one
//! request line and sends each line it writes back as one text message. Same API, same checks.

use std::sync::Arc;

use base64::Engine;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use crate::agents::Peer;
use crate::serve::{Shared, MAX_LINE};

/// The most a message may hold: a request line, its newline added here.
const MOST: usize = MAX_LINE - 1;
/// The pipe between the frames and `serve::connection`: a slow browser fills it and waits, as a
/// slow socket reader does.
const PIPE: usize = 64 << 10;

/// `Sec-WebSocket-Accept` for a client's `Sec-WebSocket-Key`.
pub(crate) fn accept_key(key: &str) -> String {
    let mut h = sha1_smol::Sha1::new();
    h.update(key.as_bytes());
    h.update(b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    base64::engine::general_purpose::STANDARD.encode(h.digest().bytes())
}

/// What the client sent, as far as the bridge cares.
#[derive(Debug, PartialEq)]
enum Got {
    /// A whole text message: one request line, without its newline.
    Text(Vec<u8>),
    Ping(Vec<u8>),
    /// End with this close code (the client's close is answered with 1000).
    Close(u16),
}

/// Client frames in, messages out. Every length is checked against `MOST` from the frame header,
/// before its payload is read or kept, so a connection buffers at most about `MOST` bytes.
#[derive(Default)]
struct Decoder {
    buf: Vec<u8>,
    /// Where the next frame starts in `buf`: frames are skipped, and `buf` compacted only when
    /// that moves past half of it, so many small frames cost linear time.
    at: usize,
    /// The message so far, while its continuation frames come.
    msg: Option<Vec<u8>>,
}

impl Decoder {
    /// The next thing the client said, or None until more bytes come.
    fn next(&mut self) -> Option<Got> {
        loop {
            if self.at > 0 && self.at * 2 >= self.buf.len() {
                self.buf.drain(..self.at);
                self.at = 0;
            }
            let b = &self.buf[self.at..];
            if b.len() < 2 {
                return None;
            }
            let (fin, rsv, op, masked) = (b[0] & 0x80 != 0, b[0] & 0x70, b[0] & 0x0f, b[1] & 0x80 != 0);
            if rsv != 0 || !masked {
                return Some(Got::Close(1002)); // no extensions; a client frame must be masked
            }
            let (len, at) = match b[1] & 0x7f {
                126 if b.len() >= 4 => (u16::from_be_bytes([b[2], b[3]]) as u64, 4),
                127 if b.len() >= 10 => (u64::from_be_bytes(b[2..10].try_into().expect("8 bytes")), 10),
                126 | 127 => return None,
                n => (n as u64, 2),
            };
            let control = op >= 8;
            if control && (!fin || len > 125) {
                return Some(Got::Close(1002));
            }
            let so_far = self.msg.as_ref().map_or(0, Vec::len) as u64;
            if !control && len > MOST as u64 - so_far {
                return Some(Got::Close(1009)); // before a byte of it is kept
            }
            let len = len as usize;
            if b.len() < at + 4 + len {
                return None;
            }
            let mask: [u8; 4] = b[at..at + 4].try_into().expect("4 bytes");
            let data: Vec<u8> = b[at + 4..at + 4 + len].iter().enumerate().map(|(i, x)| x ^ mask[i % 4]).collect();
            self.at += at + 4 + len;
            let part = match (op, self.msg.take()) {
                (0x8, _) => return Some(Got::Close(1000)),
                (0x9, msg) => {
                    self.msg = msg;
                    return Some(Got::Ping(data));
                }
                (0xA, msg) => {
                    self.msg = msg; // an unasked pong: nothing to do
                    continue;
                }
                (0x1, None) => data,
                (0x0, Some(mut m)) => {
                    m.extend_from_slice(&data);
                    m
                }
                (0x2, _) => return Some(Got::Close(1003)), // binary: the API is text
                _ => return Some(Got::Close(1002)),        // a stray continuation, an unknown opcode
            };
            if !fin {
                self.msg = Some(part);
                continue;
            }
            if std::str::from_utf8(&part).is_err() || part.contains(&b'\n') {
                return Some(Got::Close(1007)); // one message is one request line
            }
            return Some(Got::Text(part));
        }
    }
}

/// A server frame: never masked.
fn frame(op: u8, data: &[u8]) -> Vec<u8> {
    let mut f = vec![0x80 | op];
    match data.len() {
        n if n < 126 => f.push(n as u8),
        n if n < 65536 => {
            f.push(126);
            f.extend_from_slice(&(n as u16).to_be_bytes());
        }
        n => {
            f.push(127);
            f.extend_from_slice(&(n as u64).to_be_bytes());
        }
    }
    f.extend_from_slice(data);
    f
}

/// What the reader asks the writer to send.
enum Ctl {
    Pong(Vec<u8>),
    Close(u16),
}

/// Serve the API over an upgraded connection until either side ends. `rest`: bytes read past the
/// upgrade's head. As on the socket, a client that goes away lets the call being answered finish:
/// the reader ends the API's input, `serve::connection` returns after that call, and the writer
/// drains what it wrote (to the client while it reads, else nowhere) so it never blocks on the pipe.
pub(crate) async fn bridge<S>(stream: S, rest: Vec<u8>, shared: &Arc<Shared>)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (mut tcp_r, mut tcp_w) = tokio::io::split(stream);
    let (ours, theirs) = tokio::io::duplex(PIPE);
    let (api_r, api_w) = tokio::io::split(theirs);
    let (our_r, mut our_w) = tokio::io::split(ours);
    let (ctl, mut ctls) = mpsc::channel::<Ctl>(4);
    let inbound = async move {
        let mut d = Decoder { buf: rest, ..Decoder::default() };
        let mut chunk = vec![0u8; 16 << 10];
        'read: loop {
            while let Some(got) = d.next() {
                match got {
                    Got::Text(mut line) => {
                        line.push(b'\n');
                        if our_w.write_all(&line).await.is_err() {
                            break 'read;
                        }
                    }
                    Got::Ping(p) => {
                        let _ = ctl.send(Ctl::Pong(p)).await;
                    }
                    Got::Close(code) => {
                        let _ = ctl.send(Ctl::Close(code)).await;
                        break 'read;
                    }
                }
            }
            match tcp_r.read(&mut chunk).await {
                Ok(0) | Err(_) => break,
                Ok(n) => d.buf.extend_from_slice(&chunk[..n]),
            }
        }
        drop((our_w, ctl)); // the API's input ends here
        std::future::pending::<()>().await // ended by the other two
    };
    let outbound = async {
        let mut lines = BufReader::new(our_r);
        let mut line = Vec::new();
        let (mut open, mut ctls_open) = (true, true); // `open`: still writing to the client
        loop {
            let out = tokio::select! {
                biased;
                c = ctls.recv(), if ctls_open => match c {
                    None => {
                        ctls_open = false;
                        continue;
                    }
                    Some(Ctl::Pong(p)) => frame(0xA, &p),
                    Some(Ctl::Close(code)) => {
                        let f = frame(0x8, &code.to_be_bytes());
                        if open {
                            let _ = send(&mut tcp_w, &f).await;
                        }
                        open = false;
                        continue;
                    }
                },
                n = lines.read_until(b'\n', &mut line) => {
                    if !matches!(n, Ok(n) if n > 0) {
                        if open {
                            let _ = send(&mut tcp_w, &frame(0x8, &1000u16.to_be_bytes())).await; // the API side ended
                        }
                        return;
                    }
                    line.pop_if(|b| *b == b'\n');
                    let f = frame(0x1, &line);
                    line.clear();
                    f
                }
            };
            if open && send(&mut tcp_w, &out).await.is_err() {
                open = false;
            }
        }
    };
    tokio::select! {
        _ = async { tokio::join!(crate::serve::connection(api_r, api_w, shared, Peer::Remote), outbound) } => {}
        _ = inbound => {}
    }
}

/// Write a frame and flush it: over TLS a write can leave the records in rustls's buffer until
/// the next one, and the last reply of a burst would wait for a frame that may never come.
async fn send(w: &mut (impl AsyncWrite + Unpin), frame: &[u8]) -> std::io::Result<()> {
    w.write_all(frame).await?;
    w.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A masked client frame.
    fn client(fin: bool, op: u8, data: &[u8]) -> Vec<u8> {
        let mask = [9u8, 8, 7, 6];
        let mut f = frame(op, data);
        f[0] = (f[0] & 0x7f) | if fin { 0x80 } else { 0 };
        f[1] |= 0x80;
        let at = f.len() - data.len();
        f.splice(at..at, mask);
        f[at + 4..].iter_mut().enumerate().for_each(|(i, b)| *b ^= mask[i % 4]);
        f
    }

    fn decode(bytes: Vec<u8>) -> Option<Got> {
        Decoder { buf: bytes, ..Decoder::default() }.next()
    }

    #[test]
    fn accept_key() {
        assert_eq!(super::accept_key("dGhlIHNhbXBsZSBub25jZQ=="), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }

    #[test]
    fn unmasked_frame_closes() {
        assert_eq!(decode(frame(0x1, b"{}")), Some(Got::Close(1002)));
        assert_eq!(decode(client(true, 0x1, b"{}")), Some(Got::Text(b"{}".to_vec())));
    }

    #[test]
    fn fragments_over_the_cap_close() {
        let part = vec![b'a'; MOST / 2 + 1];
        let mut d = Decoder::default();
        d.buf.extend(client(false, 0x1, &part));
        assert_eq!(d.next(), None, "the first part waits for the rest");
        d.buf.extend(client(true, 0x0, &part));
        assert_eq!(d.next(), Some(Got::Close(1009)), "the total passes the cap though each part doesn't");
        let mut ok = Decoder::default();
        ok.buf.extend(client(false, 0x1, b"{\"id\""));
        ok.buf.extend(client(true, 0x0, b":1}"));
        assert_eq!(ok.next(), Some(Got::Text(b"{\"id\":1}".to_vec())));
    }

    #[test]
    fn a_huge_declared_length_closes_unread() {
        let mut head = vec![0x81u8, 0x80 | 127];
        head.extend_from_slice(&(1u64 << 62).to_be_bytes()); // no mask, no payload: never waited for
        assert_eq!(decode(head), Some(Got::Close(1009)));
        let mut d = Decoder::default();
        d.buf.extend(client(false, 0x1, b"{"));
        d.buf.extend([0x80, 0x80 | 127]);
        d.buf.extend(u64::MAX.to_be_bytes()); // with the part before it, past u64: no overflow
        assert_eq!(d.next(), Some(Got::Close(1009)));
    }

    #[test]
    fn many_small_frames_need_no_stack() {
        let mut d = Decoder::default();
        d.buf.extend(client(false, 0x1, b"{"));
        for _ in 0..200_000 {
            d.buf.extend(client(true, 0xA, b"")); // unasked pongs
            d.buf.extend(client(false, 0x0, b"")); // empty continuations
        }
        d.buf.extend(client(true, 0x0, b"}"));
        assert_eq!(d.next(), Some(Got::Text(b"{}".to_vec())));
    }

    #[test]
    fn binary_is_refused() {
        assert_eq!(decode(client(true, 0x2, b"{}")), Some(Got::Close(1003)));
    }

    #[test]
    fn a_newline_is_refused() {
        assert_eq!(decode(client(true, 0x1, b"{}\n{}")), Some(Got::Close(1007)));
        assert_eq!(decode(client(true, 0x1, &[0xff, 0xfe])), Some(Got::Close(1007)), "not UTF-8");
    }

    #[test]
    fn ping_gets_pong() {
        let mut d = Decoder::default();
        d.buf.extend(client(false, 0x1, b"{"));
        d.buf.extend(client(true, 0x9, b"hi"));
        d.buf.extend(client(true, 0x0, b"}"));
        assert_eq!(d.next(), Some(Got::Ping(b"hi".to_vec())), "a ping between a message's parts");
        assert_eq!(d.next(), Some(Got::Text(b"{}".to_vec())));
        assert_eq!(frame(0xA, b"hi"), [0x8A, 2, b'h', b'i'], "the pong carries the same payload");
        assert_eq!(decode(client(true, 0x8, &1000u16.to_be_bytes())), Some(Got::Close(1000)));
    }
}
