//! Lifts OSC 7501 out of a PTY byte stream, in any chunking, and spots RIS and OSC 133 A on the way.

/// What the scanner hands back, in stream order.
#[derive(Debug, PartialEq, Eq)]
pub enum Piece<'a> {
    /// Bytes for the emulator, unchanged.
    Bytes(&'a [u8]),
    /// The payload of an `OSC 7501 ; … ST`, which is not passed on.
    Report(&'a [u8]),
    /// `OSC 133 ; A` (a new prompt) ended; its bytes were passed on.
    Prompt,
    /// `ESC c` (RIS); its bytes were passed on.
    Reset,
}

const ESC: u8 = 0x1b;
const BEL: u8 = 0x07;
const CAN: u8 = 0x18;
const SUB: u8 = 0x1a;
const PREFIX: &[u8] = b"\x1b]7501;";
/// The whole escape, prefix and terminator included.
const MAX_SEQUENCE: usize = 4096;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Mode {
    #[default]
    Ground,
    /// Held an ESC. `osc`: it came inside another OSC, so `\` completes that OSC.
    Esc { osc: bool },
    /// `buf` holds a proper prefix of `ESC ] 7501 ;`, not passed on yet.
    Prefix,
    /// Inside OSC 7501; `buf` is the payload. `esc`: the last byte was ESC.
    Report { esc: bool },
    /// Inside another OSC, passing it on; `buf` keeps its first 6 bytes to spot `133;A`.
    Osc,
}

/// Every ESC starts a new escape from any mode, as in vte, so we split sequences where the emulator does.
#[derive(Debug, Default)]
pub struct Scanner {
    mode: Mode,
    buf: Vec<u8>,
    /// Pass-through bytes not yet handed out.
    pass: Vec<u8>,
}

impl Scanner {
    pub fn feed(&mut self, input: &[u8], mut f: impl FnMut(Piece<'_>)) {
        let mut i = 0;
        while i < input.len() {
            if self.mode == Mode::Ground {
                let n = input[i..].iter().position(|&b| b == ESC).unwrap_or(input.len() - i);
                self.pass.extend_from_slice(&input[i..i + n]);
                i += n;
                if i == input.len() {
                    break;
                }
            }
            self.step(input[i], &mut f);
            i += 1;
        }
        self.flush(&mut f);
    }

    fn step(&mut self, b: u8, f: &mut impl FnMut(Piece<'_>)) {
        match self.mode {
            Mode::Ground if b == ESC => self.mode = Mode::Esc { osc: false },
            Mode::Ground => self.pass.push(b),
            Mode::Esc { osc: true } if b == b'\\' => {
                self.pass.extend_from_slice(b"\x1b\\");
                self.end_osc(f);
            }
            Mode::Esc { osc } => {
                if osc {
                    // vte ends an OSC at any ESC, so a cut-off `133;A` is still a prompt.
                    self.end_osc(f);
                }
                self.after_esc(b, f);
            }
            Mode::Prefix => {
                self.buf.push(b);
                if self.buf == PREFIX {
                    self.buf.clear();
                    self.mode = Mode::Report { esc: false };
                } else if !PREFIX.starts_with(&self.buf) {
                    // Some other OSC: release what we held and carry on passing it through.
                    self.buf.pop();
                    self.pass.extend_from_slice(&self.buf);
                    self.buf.drain(..2);
                    self.mode = Mode::Osc;
                    self.step(b, f);
                }
            }
            Mode::Report { esc: false } => match b {
                BEL => self.finish(1, f),
                ESC => self.mode = Mode::Report { esc: true },
                CAN | SUB => {
                    self.buf.clear();
                    self.pass.push(b);
                    self.mode = Mode::Ground;
                }
                // Past the limit only the length matters, so memory stays bounded.
                _ if self.buf.len() <= MAX_SEQUENCE => self.buf.push(b),
                _ => {}
            },
            Mode::Report { esc: true } if b == b'\\' => self.finish(2, f),
            Mode::Report { esc: true } => {
                // Not ST: the report is cut off, and this ESC starts something new.
                self.buf.clear();
                self.mode = Mode::Esc { osc: false };
                self.step(b, f);
            }
            Mode::Osc => match b {
                BEL => {
                    self.pass.push(b);
                    self.end_osc(f);
                }
                ESC => self.mode = Mode::Esc { osc: true },
                CAN | SUB => {
                    self.pass.push(b);
                    self.end_osc(f);
                }
                _ => {
                    self.pass.push(b);
                    if self.buf.len() < 6 {
                        self.buf.push(b);
                    }
                }
            },
        }
    }

    /// The byte after an ESC (held, not yet passed on).
    fn after_esc(&mut self, b: u8, f: &mut impl FnMut(Piece<'_>)) {
        match b {
            b']' => {
                self.buf.clear();
                self.buf.extend_from_slice(b"\x1b]");
                self.mode = Mode::Prefix;
            }
            b'c' => {
                self.pass.extend_from_slice(b"\x1bc");
                self.flush(f);
                f(Piece::Reset);
                self.mode = Mode::Ground;
            }
            ESC => {
                self.pass.push(ESC);
                self.mode = Mode::Esc { osc: false };
            }
            _ => {
                self.pass.extend_from_slice(&[ESC, b]);
                self.mode = Mode::Ground;
            }
        }
    }

    fn end_osc(&mut self, f: &mut impl FnMut(Piece<'_>)) {
        let prompt = self.buf == b"133;A" || self.buf.starts_with(b"133;A;");
        self.buf.clear();
        self.mode = Mode::Ground;
        if prompt {
            self.flush(f);
            f(Piece::Prompt);
        }
    }

    fn finish(&mut self, terminator: usize, f: &mut impl FnMut(Piece<'_>)) {
        self.mode = Mode::Ground;
        if PREFIX.len() + self.buf.len() + terminator <= MAX_SEQUENCE {
            self.flush(f);
            f(Piece::Report(&self.buf));
        }
        self.buf.clear();
    }

    fn flush(&mut self, f: &mut impl FnMut(Piece<'_>)) {
        if !self.pass.is_empty() {
            f(Piece::Bytes(&self.pass));
            self.pass.clear();
        }
    }
}
