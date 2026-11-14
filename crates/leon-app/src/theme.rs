//! leon's dark theme: the terminal palette and the chrome around it, as 0xRRGGBB.

pub struct Theme {
    pub fg: u32,
    pub bg: u32,
    pub cursor: u32,
    /// ANSI 0–15: black, red, green, yellow, blue, magenta, cyan, white, then the bright ones.
    pub ansi: [u32; 16],
    /// Sidebar and tab bar.
    pub chrome: u32,
    pub hover: u32,
    pub border: u32,
    pub text: u32,
    pub muted: u32,
    pub accent: u32,
}

pub const DARK: Theme = Theme {
    fg: 0xd5dae5,
    bg: 0x0f1117,
    cursor: 0x7aa2ff,
    ansi: [
        0x1b1e28, 0xf2677a, 0x8bd17e, 0xe8c46b, 0x6aa3ff, 0xc592ff, 0x5fd4d4, 0xc9ced9, // normal
        0x5c6373, 0xff8a99, 0xa6e59a, 0xf5d68c, 0x8fb9ff, 0xd6b0ff, 0x85e5e5, 0xf4f6fa, // bright
    ],
    chrome: 0x0a0c11,
    hover: 0x171a23,
    border: 0x1d2130,
    text: 0xc9ced9,
    muted: 0x6d7587,
    accent: 0x7aa2ff,
};

/// A 256-color palette entry: the theme's 16, the 6×6×6 cube, then the 24 grays.
pub fn indexed(i: u8, theme: &Theme) -> u32 {
    const LEVELS: [u32; 6] = [0, 95, 135, 175, 215, 255];
    match i {
        0..=15 => theme.ansi[i as usize],
        16..=231 => {
            let n = (i - 16) as usize;
            LEVELS[n / 36] << 16 | LEVELS[n / 6 % 6] << 8 | LEVELS[n % 6]
        }
        _ => {
            let v = 8 + 10 * (i as u32 - 232);
            v << 16 | v << 8 | v
        }
    }
}

/// `a` moved `t` of the way toward `b`, per channel.
pub fn mix(a: u32, b: u32, t: f32) -> u32 {
    let ch = |shift: u32| {
        let (x, y) = ((a >> shift & 0xff) as f32, (b >> shift & 0xff) as f32);
        ((x + (y - x) * t).round() as u32) << shift
    };
    ch(16) | ch(8) | ch(0)
}
