//! leon's dark theme, from the brand palette (brand/README.md "Palette"): the terminal palette and
//! the app's chrome around it, as 0xRRGGBB. The app takes every colour it draws from here.

pub struct Theme {
    pub fg: u32,
    /// The terminal pane (Coal).
    pub bg: u32,
    pub cursor: u32,
    /// ANSI 0–15: black, red, green, yellow, blue, magenta, cyan, white, then the bright ones.
    pub ansi: [u32; 16],
    /// The window around the panes: sidebar, titlebar, the channels between panes (Iron).
    pub chrome: u32,
    pub hover: u32,
    pub border: u32,
    /// Body text (Chalk) and titles (Parchment).
    pub text: u32,
    pub title: u32,
    /// Secondary text: Chalk a quarter of the way to Iron, still 4.5:1 on Iron, hover and Maroon.
    pub muted: u32,
    /// Focus and the cursor (Amber).
    pub accent: u32,
    /// The selected tab (the Maroon app tile).
    pub selected: u32,
    /// The brand anchor (Oxide), for the mark.
    pub brand: u32,
    pub working: u32,
    pub done: u32,
    pub blocked: u32,
    pub error: u32,
}

pub const DARK: Theme = Theme {
    fg: 0xD8CEC2,
    bg: 0x1F1C19,
    cursor: 0xF3A747,
    ansi: [
        0x3A342E, 0xE0715F, 0xA9B98F, 0xE8C77D, 0x86A6C2, 0xC994B0, 0x8FBCB0, 0xD8CEC2, // normal
        0x6E645A, 0xF08A7A, 0xC2D1A8, 0xF3D79A, 0xA2BDD6, 0xDDB0C8, 0xA9D3C8, 0xF5F0E7, // bright
    ],
    chrome: 0x292521,
    hover: 0x35312D,
    border: 0x3D3934,
    text: 0xD8CEC2,
    title: 0xF5F0E7,
    muted: 0xACA49A,
    accent: 0xF3A747,
    selected: 0x542D35,
    brand: 0xB64C32,
    working: 0xE78155,
    done: 0xB8C6AC,
    blocked: 0xE8C77D,
    error: 0xE28E9B,
};

/// The app's text sizes in points: UI text, and secondary text (session labels, shortcuts).
pub const UI_TEXT: f32 = 13.;
pub const SECONDARY_TEXT: f32 = 11.;

/// The WCAG contrast ratio of two colours (1 to 21).
pub fn contrast(a: u32, b: u32) -> f64 {
    let lum = |c: u32| {
        let ch = |shift: u32| {
            let v = (c >> shift & 0xff) as f64 / 255.;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * ch(16) + 0.7152 * ch(8) + 0.0722 * ch(0)
    };
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

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
