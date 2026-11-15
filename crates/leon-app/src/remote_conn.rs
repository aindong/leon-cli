//! One request-and-reply connection to a server, and the one deadline a call has for its write
//! and its reply (lines that aren't the reply don't extend it).

use super::*;

/// One NDJSON connection used for request and reply only.
pub(super) struct Conn {
    pub(super) r: BufReader<UnixStream>,
    pub(super) id: u64,
    /// How long a whole call may take; None waits for ever.
    pub(super) timeout: Option<Duration>,
}

/// The time left until `end`, for a socket's timeout; an error once it has passed.
pub(super) fn left(end: Option<Instant>) -> io::Result<Option<Duration>> {
    let Some(end) = end else { return Ok(None) };
    match end.saturating_duration_since(Instant::now()) {
        d if d.is_zero() => Err(io::Error::new(io::ErrorKind::TimedOut, "no reply in time")),
        d => Ok(Some(d)),
    }
}

impl Conn {
    pub(super) fn open(socket: &Path, timeout: Option<Duration>) -> io::Result<Conn> {
        let s = UnixStream::connect(socket)?;
        s.set_read_timeout(timeout)?;
        s.set_write_timeout(timeout)?;
        Ok(Conn { r: BufReader::new(s), id: 0, timeout })
    }

    /// `line`, each write given only what is left of the deadline (a slow reader can't stretch it).
    fn write(&self, mut line: &[u8], end: Option<Instant>) -> io::Result<()> {
        let mut s = self.r.get_ref();
        while !line.is_empty() {
            s.set_write_timeout(left(end)?)?;
            match s.write(line) {
                Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                Ok(n) => line = &line[n..],
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    pub(super) fn call(&mut self, method: &str, params: Value) -> Result<Value, ApiError> {
        let end = self.timeout.map(|t| Instant::now() + t);
        self.id += 1;
        let mut line = serde_json::to_vec(&json!({ "id": self.id, "method": method, "params": params })).expect("json");
        line.push(b'\n');
        self.write(&line, end).map_err(unreachable)?;
        loop {
            let mut reply = String::new();
            match read_line(&mut self.r, &mut reply, end) {
                Ok(0) => return Err(unreachable("the connection closed")),
                Err(e) => return Err(unreachable(e)),
                Ok(_) => {}
            }
            let mut v: Value = serde_json::from_str(&reply).map_err(unreachable)?;
            if v["id"] != self.id {
                continue; // a late reply to a call that timed out
            }
            return match v.get_mut("error").filter(|e| !e.is_null()) {
                Some(e) => Err(serde_json::from_value(e.take()).unwrap_or_else(unreachable)),
                None => Ok(v["result"].take()),
            };
        }
    }
}
