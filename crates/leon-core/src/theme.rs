//! leon's themes, from the brand palette (brand/README.md "Palette"): the terminal palette and the
//! app's chrome around it, as 0xRRGGBB. The app takes every colour it draws from here. [`import`]
//! reads Ghostty, iTerm2 and leon theme files.

pub mod import;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    /// Selected text in a pane (the Maroon tile, as the selected tab).
    pub selection: u32,
    /// Find matches (Wheat sunk a long way into Coal); the current one is outlined in `accent`.
    pub found: u32,
    /// A link under the mouse with cmd held (Clay, the dark-mode link colour).
    pub link: u32,
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
    selection: 0x542D35,
    found: 0x5B4C30,
    link: 0xE6AA91,
};

/// leon's light theme: Iron text on Parchment, Sand chrome, Oxide for focus and links, and the
/// status colours deepened until they read at 4.5:1 on Parchment and Sand.
pub const LIGHT: Theme = Theme {
    fg: 0x292521,
    bg: 0xF5F0E7,
    cursor: 0xB64C32,
    ansi: [
        0x292521, 0xB03A2E, 0x4F6B32, 0x80600F, 0x2F5F8A, 0x8A3D6E, 0x2C6B63, 0xD8CEC2, // normal
        0x65594F, 0x9A2F24, 0x3F5A26, 0x6B500A, 0x24507A, 0x76305D, 0x215A53, 0xEBE3D7, // bright
    ],
    chrome: 0xEBE3D7,
    hover: 0xDFD8CC,
    border: 0xD0C8BE,
    text: 0x292521,
    title: 0x1F1C19,
    muted: 0x65594F,
    accent: 0xB64C32,
    selected: 0xE5DCD5,
    brand: 0xB64C32,
    working: 0x9C4019,
    done: 0x47613A,
    blocked: 0x77580C,
    error: 0x9C3346,
    selection: 0xD8CDC7,
    found: 0xEED9AD,
    link: 0xB64C32,
};

/// The themes every leon has, by the name settings use.
pub const BUILTIN: [(&str, Theme); 2] = [("leon-dark", DARK), ("leon-light", LIGHT)];

pub fn builtin(name: &str) -> Option<Theme> {
    BUILTIN.iter().find(|(n, _)| *n == name).map(|(_, t)| *t)
}

impl Theme {
    /// leon's own token names in a theme file, beside Ghostty's `background`, `palette` and so on.
    pub const TOKENS: [&'static str; 15] = [
        "chrome", "hover", "border", "text", "title", "muted", "accent", "selected", "brand", "working", "done",
        "blocked", "error", "found", "link",
    ];

    /// The colour a theme file names `key`: a Ghostty key or one of [`Theme::TOKENS`].
    pub fn slot(&mut self, key: &str) -> Option<&mut u32> {
        Some(match key {
            "background" => &mut self.bg,
            "foreground" => &mut self.fg,
            "cursor-color" => &mut self.cursor,
            "selection-background" => &mut self.selection,
            "chrome" => &mut self.chrome,
            "hover" => &mut self.hover,
            "border" => &mut self.border,
            "text" => &mut self.text,
            "title" => &mut self.title,
            "muted" => &mut self.muted,
            "accent" => &mut self.accent,
            "selected" => &mut self.selected,
            "brand" => &mut self.brand,
            "working" => &mut self.working,
            "done" => &mut self.done,
            "blocked" => &mut self.blocked,
            "error" => &mut self.error,
            "found" => &mut self.found,
            "link" => &mut self.link,
            _ => return None,
        })
    }

    /// The pane is nearer black than white.
    pub fn is_dark(&self) -> bool {
        contrast(self.bg, 0) < contrast(self.bg, 0xFFFFFF)
    }
}

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
