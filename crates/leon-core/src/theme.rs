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

/// leon's light theme: Iron text on Parchment, Sand chrome, Oxide for the cursor and links (a
/// deeper Oxide for focus and accent text), a selected row a step deeper than hover, and the
/// secondary and status colours deepened until they read at 4.5:1 on every one of those.
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
    muted: 0x61554C,
    accent: 0x8F402D,
    selected: 0xDCCDC6,
    brand: 0xB64C32,
    working: 0x933D19,
    done: 0x455E39,
    blocked: 0x70530D,
    error: 0x9A3345,
    selection: 0xD8CDC7,
    found: 0xEED9AD,
    link: 0xB64C32,
};

// The themes below are leon's own, built from the brand palette (brand/README.md) in a few moods;
// crates/leon-core/tests/themes_builtin.rs holds every one to the same contrast rules.

/// A warm dark: Deep maroon under Parchment-warm text, Gold for the cursor and Amber for focus.
pub const MAROON: Theme = Theme {
    fg: 0xEBDCD0,
    bg: 0x22181A,
    cursor: 0xFFD477,
    ansi: [
        0x3D2B2E, 0xF07A68, 0xB5C48E, 0xF2CA72, 0x92AECB, 0xD99BBB, 0x93C9B8, 0xE8D9CF, // normal
        0x7E6A66, 0xFF9584, 0xCFDDA8, 0xFFDD99, 0xB0C8E0, 0xEDB9D3, 0xB2E0D2, 0xFBF1E8, // bright
    ],
    chrome: 0x2C1F22,
    hover: 0x3A2A2D,
    border: 0x47353A,
    text: 0xE8D9CF,
    title: 0xFBF1E8,
    muted: 0xBBA9A4,
    accent: 0xFFB65C,
    selected: 0x522A32,
    brand: 0xB64C32,
    working: 0xF48E63,
    done: 0xB9CDA4,
    blocked: 0xF2CF7A,
    error: 0xF39AAB,
    selection: 0x522A32,
    found: 0x5E4A2A,
    link: 0xEBAE95,
};

/// A cool dark: slate blue-grey, with leon's Amber kept for the cursor and focus.
pub const TIDE: Theme = Theme {
    fg: 0xD3DCE3,
    bg: 0x161C22,
    cursor: 0xF3A747,
    ansi: [
        0x2A323B, 0xEE7B72, 0x9CCB8C, 0xE3C77A, 0x7FB2E5, 0xC79DE0, 0x74C9C6, 0xCDD6DE, // normal
        0x66727F, 0xFF978E, 0xB6E0A6, 0xF3DA98, 0x9DC8F2, 0xDAB6F0, 0x96DEDB, 0xEEF3F7, // bright
    ],
    chrome: 0x1E252C,
    hover: 0x29313A,
    border: 0x343E48,
    text: 0xCDD6DE,
    title: 0xEEF3F7,
    muted: 0x9DA9B5,
    accent: 0xF3A747,
    selected: 0x283A4C,
    brand: 0xB64C32,
    working: 0xEE8A5F,
    done: 0x9ED0A8,
    blocked: 0xE8C77D,
    error: 0xF2919E,
    selection: 0x283A4C,
    found: 0x4D4630,
    link: 0x8EC3E6,
};

/// The high-contrast dark: near-black under Parchment, every colour brighter.
pub const COAL: Theme = Theme {
    fg: 0xF5F0E7,
    bg: 0x0E0D0C,
    cursor: 0xFFD477,
    ansi: [
        0x2E2B28, 0xFF7F6E, 0xB7D98F, 0xFFD877, 0x8FBCF0, 0xF0A0D0, 0x86DCCB, 0xE6DED4, // normal
        0x8A8077, 0xFF9F92, 0xCFF0AA, 0xFFE6A3, 0xB0D2FF, 0xFFBDE3, 0xA8F0E0, 0xFFFFFF, // bright
    ],
    chrome: 0x171513,
    hover: 0x24211E,
    border: 0x3A3632,
    text: 0xF2ECE4,
    title: 0xFFFFFF,
    muted: 0xC7BDB1,
    accent: 0xFFC061,
    selected: 0x5A2F38,
    brand: 0xB64C32,
    working: 0xFF9466,
    done: 0xC6DDB2,
    blocked: 0xFFD877,
    error: 0xFF9BAA,
    selection: 0x5A2F38,
    found: 0x6A5528,
    link: 0xF0B8A0,
};

/// A paper light: near-white with Iron ink, the brightest of the light themes.
pub const LINEN: Theme = Theme {
    fg: 0x2A2420,
    bg: 0xFBF8F2,
    cursor: 0xB64C32,
    ansi: [
        0x2A2420, 0xA8352A, 0x3F6B2E, 0x7A5A08, 0x245F96, 0x873A79, 0x1F6E6A, 0xD6CFC4, // normal
        0x6A5E55, 0x8F2A20, 0x33591F, 0x634905, 0x1C4E7E, 0x702E63, 0x175A56, 0xEDE7DD, // bright
    ],
    chrome: 0xF1ECE3,
    hover: 0xE5DFD5,
    border: 0xD6CFC4,
    text: 0x2A2420,
    title: 0x15110E,
    muted: 0x5E5249,
    accent: 0x8F402D,
    selected: 0xE2D4CD,
    brand: 0xB64C32,
    working: 0x933D19,
    done: 0x455E39,
    blocked: 0x70530D,
    error: 0x9A3345,
    selection: 0xE6D3C9,
    found: 0xF5DE9E,
    link: 0xA2432C,
};

/// A soft light: Sand under a dark Iron, less glare than Parchment.
pub const SAND: Theme = Theme {
    fg: 0x2B241F,
    bg: 0xEBE3D7,
    cursor: 0xB64C32,
    ansi: [
        0x3A322B, 0x9C3226, 0x3E6029, 0x6E5107, 0x275A86, 0x7C3570, 0x1E625E, 0xC9BCAA, // normal
        0x6A5E52, 0x86281E, 0x30511F, 0x5A4204, 0x1D4A72, 0x672C5D, 0x16524E, 0xF3EDE4, // bright
    ],
    chrome: 0xE2D9CB,
    hover: 0xD8CDBD,
    border: 0xC9BCAA,
    text: 0x3A322B,
    title: 0x241E19,
    muted: 0x584C42,
    accent: 0x7E3523,
    selected: 0xD3C2B6,
    brand: 0xB64C32,
    working: 0x823413,
    done: 0x3B5530,
    blocked: 0x5E4507,
    error: 0x8A2A3C,
    selection: 0xD6C4B9,
    found: 0xE5C985,
    link: 0x8F3A22,
};

/// A cool light: blue-grey paper with slate ink, Oxide for the cursor and focus.
pub const MIST: Theme = Theme {
    fg: 0x1F2A33,
    bg: 0xF2F5F7,
    cursor: 0xB64C32,
    ansi: [
        0x1F2A33, 0xB0352B, 0x3B6B2A, 0x7A5A00, 0x1F5F9E, 0x7E3C8C, 0x136C72, 0xC9D2DA, // normal
        0x5B6873, 0x92281F, 0x2E5720, 0x614700, 0x184C80, 0x662F72, 0x0E585D, 0xE6EBEF, // bright
    ],
    chrome: 0xE6EBEF,
    hover: 0xDAE1E7,
    border: 0xC9D2DA,
    text: 0x1F2A33,
    title: 0x111A21,
    muted: 0x4C5A66,
    accent: 0x8F402D,
    selected: 0xCFDCE8,
    brand: 0xB64C32,
    working: 0x963E15,
    done: 0x3C6040,
    blocked: 0x6B5208,
    error: 0xA02F48,
    selection: 0xC9DAEA,
    found: 0xF1DD9C,
    link: 0x1F5F99,
};

/// The themes every leon has, by the name settings use: leon's dark and light, then the dark and
/// the light moods.
pub const BUILTIN: [(&str, Theme); 8] = [
    ("leon-dark", DARK),
    ("leon-light", LIGHT),
    ("leon-maroon", MAROON),
    ("leon-tide", TIDE),
    ("leon-coal", COAL),
    ("leon-linen", LINEN),
    ("leon-sand", SAND),
    ("leon-mist", MIST),
];

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
