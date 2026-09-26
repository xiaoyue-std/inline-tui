//! Style system: colors, modifiers, and combined styles.

use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not, Sub, SubAssign};

/// Color: 16 basic colors / 256-color / RGB true color.
///
/// `Reset` means the terminal's default foreground/background color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Color {
    Reset,
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    Gray,
    DarkGray,
    LightRed,
    LightGreen,
    LightYellow,
    LightBlue,
    LightMagenta,
    LightCyan,
    White,
    /// 256-color palette index (0-255).
    Indexed(u8),
    /// 24-bit true color.
    Rgb(u8, u8, u8),
}

impl Color {
    /// Emits the SGR parameter sequence for this color as a foreground color
    /// (without ESC and the trailing m).
    pub(crate) fn sgr_fg(&self, out: &mut String) {
        match self {
            Color::Reset => out.push_str("39"),
            Color::Black => out.push_str("30"),
            Color::Red => out.push_str("31"),
            Color::Green => out.push_str("32"),
            Color::Yellow => out.push_str("33"),
            Color::Blue => out.push_str("34"),
            Color::Magenta => out.push_str("35"),
            Color::Cyan => out.push_str("36"),
            Color::Gray => out.push_str("37"),
            Color::DarkGray => out.push_str("90"),
            Color::LightRed => out.push_str("91"),
            Color::LightGreen => out.push_str("92"),
            Color::LightYellow => out.push_str("93"),
            Color::LightBlue => out.push_str("94"),
            Color::LightMagenta => out.push_str("95"),
            Color::LightCyan => out.push_str("96"),
            Color::White => out.push_str("97"),
            Color::Indexed(i) => {
                out.push_str("38;5;");
                out.push_str(&i.to_string());
            }
            Color::Rgb(r, g, b) => {
                out.push_str("38;2;");
                push_u8(out, *r);
                out.push(';');
                push_u8(out, *g);
                out.push(';');
                push_u8(out, *b);
            }
        }
    }

    /// Emits the SGR parameter sequence for this color as a background color.
    pub(crate) fn sgr_bg(&self, out: &mut String) {
        match self {
            Color::Reset => out.push_str("49"),
            Color::Black => out.push_str("40"),
            Color::Red => out.push_str("41"),
            Color::Green => out.push_str("42"),
            Color::Yellow => out.push_str("43"),
            Color::Blue => out.push_str("44"),
            Color::Magenta => out.push_str("45"),
            Color::Cyan => out.push_str("46"),
            Color::Gray => out.push_str("47"),
            Color::DarkGray => out.push_str("100"),
            Color::LightRed => out.push_str("101"),
            Color::LightGreen => out.push_str("102"),
            Color::LightYellow => out.push_str("103"),
            Color::LightBlue => out.push_str("104"),
            Color::LightMagenta => out.push_str("105"),
            Color::LightCyan => out.push_str("106"),
            Color::White => out.push_str("107"),
            Color::Indexed(i) => {
                out.push_str("48;5;");
                out.push_str(&i.to_string());
            }
            Color::Rgb(r, g, b) => {
                out.push_str("48;2;");
                push_u8(out, *r);
                out.push(';');
                push_u8(out, *g);
                out.push(';');
                push_u8(out, *b);
            }
        }
    }
}

fn push_u8(out: &mut String, v: u8) {
    out.push_str(&v.to_string());
}

/// Text modifier bit set (bold, italic, underline, etc.).
///
/// Supports bitwise combination: `Modifier::BOLD | Modifier::ITALIC`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifier(u16);

impl Modifier {
    pub const EMPTY: Modifier = Modifier(0);
    pub const BOLD: Modifier = Modifier(1 << 0);
    pub const DIM: Modifier = Modifier(1 << 1);
    pub const ITALIC: Modifier = Modifier(1 << 2);
    pub const UNDERLINE: Modifier = Modifier(1 << 3);
    pub const SLOW_BLINK: Modifier = Modifier(1 << 4);
    pub const RAPID_BLINK: Modifier = Modifier(1 << 5);
    pub const REVERSED: Modifier = Modifier(1 << 6);
    pub const HIDDEN: Modifier = Modifier(1 << 7);
    pub const CROSSED_OUT: Modifier = Modifier(1 << 8);

    pub const fn bits(self) -> u16 {
        self.0
    }

    /// Constructs from a raw bit value (used by the rendering layer to compose modifier bits).
    pub const fn from_bits(bits: u16) -> Modifier {
        Modifier(bits)
    }

    pub const fn contains(self, other: Modifier) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn intersects(self, other: Modifier) -> bool {
        self.0 & other.0 != 0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl BitOr for Modifier {
    type Output = Modifier;
    fn bitor(self, rhs: Modifier) -> Modifier {
        Modifier(self.0 | rhs.0)
    }
}

impl BitOrAssign for Modifier {
    fn bitor_assign(&mut self, rhs: Modifier) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for Modifier {
    type Output = Modifier;
    fn bitand(self, rhs: Modifier) -> Modifier {
        Modifier(self.0 & rhs.0)
    }
}

impl BitAndAssign for Modifier {
    fn bitand_assign(&mut self, rhs: Modifier) {
        self.0 &= rhs.0;
    }
}

/// `a - b` removes all bits set in b.
impl Sub for Modifier {
    type Output = Modifier;
    fn sub(self, rhs: Modifier) -> Modifier {
        Modifier(self.0 & !rhs.0)
    }
}

impl SubAssign for Modifier {
    fn sub_assign(&mut self, rhs: Modifier) {
        self.0 &= !rhs.0;
    }
}

impl Not for Modifier {
    type Output = Modifier;
    fn not(self) -> Modifier {
        Modifier(!self.0)
    }
}

/// Style: optional foreground/background color + add/remove modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub add_modifier: Modifier,
    pub sub_modifier: Modifier,
}

impl Style {
    pub const fn new() -> Style {
        Style {
            fg: None,
            bg: None,
            add_modifier: Modifier::EMPTY,
            sub_modifier: Modifier::EMPTY,
        }
    }

    pub const fn fg(mut self, c: Color) -> Style {
        self.fg = Some(c);
        self
    }

    pub const fn bg(mut self, c: Color) -> Style {
        self.bg = Some(c);
        self
    }

    pub fn add_modifier(mut self, m: Modifier) -> Style {
        self.sub_modifier = self.sub_modifier.sub(m);
        self.add_modifier = self.add_modifier.bitor(m);
        self
    }

    pub fn remove_modifier(mut self, m: Modifier) -> Style {
        self.add_modifier = self.add_modifier.sub(m);
        self.sub_modifier = self.sub_modifier.bitor(m);
        self
    }

    /// Overlays `other` on top of `self` and returns the merged style (ratatui semantics).
    pub fn patch(mut self, other: Style) -> Style {
        self.fg = other.fg.or(self.fg);
        self.bg = other.bg.or(self.bg);
        self.add_modifier = self.add_modifier.sub(other.sub_modifier).bitor(other.add_modifier);
        self.sub_modifier = self.sub_modifier.sub(other.add_modifier).bitor(other.sub_modifier);
        self
    }
}

impl From<Color> for Style {
    fn from(c: Color) -> Style {
        Style::new().fg(c)
    }
}
