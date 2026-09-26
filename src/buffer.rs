//! Cell buffer: the intermediate representation of rendering and the
//! foundation of diff-based rendering.

use crate::layout::Rect;
use crate::style::{Color, Modifier, Style};
use crate::width::char_width;

/// A single character cell in the buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// The displayed string (a wide char occupies one cell; continuation
    /// placeholder cells of a wide char have an empty symbol).
    pub symbol: String,
    pub fg: Color,
    pub bg: Color,
    pub modifier: Modifier,
    /// The display width this cell occupies (1 or 2).
    pub width: u8,
}

impl Cell {
    pub fn empty() -> Cell {
        Cell {
            symbol: " ".to_string(),
            fg: Color::Reset,
            bg: Color::Reset,
            modifier: Modifier::EMPTY,
            width: 1,
        }
    }

    /// Materialize a Style into concrete Cell colors (None → Reset).
    pub fn from_style(ch: char, style: Style, width: u8) -> Cell {
        Cell {
            symbol: ch.to_string(),
            fg: style.fg.unwrap_or(Color::Reset),
            bg: style.bg.unwrap_or(Color::Reset),
            modifier: style.add_modifier,
            width,
        }
    }
}

/// A rectangular rendering buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Buffer {
    pub area: Rect,
    pub content: Vec<Cell>,
}

impl Buffer {
    pub fn empty(area: Rect) -> Buffer {
        let n = area.width as usize * area.height as usize;
        Buffer { area, content: vec![Cell::empty(); n] }
    }

    pub fn reset(&mut self) {
        for c in &mut self.content {
            *c = Cell::empty();
        }
    }

    fn index_of(&self, x: u16, y: u16) -> Option<usize> {
        if x < self.area.width && y < self.area.height {
            Some(y as usize * self.area.width as usize + x as usize)
        } else {
            None
        }
    }

    pub fn get(&self, x: u16, y: u16) -> Option<&Cell> {
        self.index_of(x, y).map(|i| &self.content[i])
    }

    pub fn get_mut(&mut self, x: u16, y: u16) -> Option<&mut Cell> {
        self.index_of(x, y).map(|i| &mut self.content[i])
    }

    /// Writes a single character (wide chars occupy the following placeholder cells).
    pub fn set_cell(&mut self, x: u16, y: u16, ch: char, style: Style) {
        let w = char_width(ch) as u8;
        if w == 0 {
            return;
        }
        if x >= self.area.width || x + w as u16 > self.area.width {
            return; // not enough room for the full wide char
        }
        let idx = self.index_of(x, y).unwrap();
        self.content[idx] = Cell::from_style(ch, style, w);
        let cont = Cell {
            symbol: String::new(),
            fg: style.fg.unwrap_or(Color::Reset),
            bg: style.bg.unwrap_or(Color::Reset),
            modifier: style.add_modifier,
            width: 0,
        };
        for k in 1..w as usize {
            self.content[idx + k] = cont.clone();
        }
    }

    /// Writes a string (no wrapping, stops at the boundary) and returns the end position.
    pub fn set_string(&mut self, x: u16, y: u16, s: &str, style: Style) -> (u16, u16) {
        let mut cx = x;
        for ch in s.chars() {
            let cw = char_width(ch);
            if cw == 0 {
                continue;
            }
            if cx + cw as u16 > self.area.width {
                break;
            }
            self.set_cell(cx, y, ch, style);
            cx += cw as u16;
        }
        (cx, y)
    }

    /// Writes a Span.
    pub fn set_span(&mut self, x: u16, y: u16, span: &Span) -> (u16, u16) {
        self.set_string(x, y, &span.content, span.style)
    }

    /// Writes the spans of a line (line-level style is patched in as the base style).
    pub fn set_line(&mut self, x: u16, y: u16, line: &crate::text::Line) -> (u16, u16) {
        let mut cx = x;
        for span in &line.spans {
            let st = line.style.patch(span.style);
            let (nx, _) = self.set_string(cx, y, &span.content, st);
            cx = nx;
        }
        (cx, y)
    }

    /// Overlays a style on all cells within the area.
    pub fn set_style(&mut self, area: Rect, style: Style) {
        let ax = area.x.saturating_sub(self.area.x);
        let ay = area.y.saturating_sub(self.area.y);
        for y in ay..(ay + area.height).min(self.area.height) {
            for x in ax..(ax + area.width).min(self.area.width) {
                if let Some(c) = self.get_mut(x, y) {
                    let st = Style::new()
                        .fg(c.fg)
                        .bg(c.bg)
                        .add_modifier(c.modifier)
                        .patch(style);
                    c.fg = st.fg.unwrap_or(Color::Reset);
                    c.bg = st.bg.unwrap_or(Color::Reset);
                    c.modifier = st.add_modifier;
                }
            }
        }
    }

    /// Compares cell-by-cell with another buffer and returns the list of changed cells.
    ///
    /// The caller guarantees both buffers have the same area (Terminal performs a
    /// full redraw when the size changes).
    pub fn diff<'a>(&'a self, other: &'a Buffer) -> Vec<(u16, u16, &'a Cell)> {
        let mut out = Vec::new();
        for y in 0..other.area.height {
            for x in 0..other.area.width {
                let i = (y * other.area.width + x) as usize;
                let old = self.content.get(i);
                let new = &other.content[i];
                if old != Some(new) {
                    out.push((x, y, new));
                }
            }
        }
        out
    }
}

use crate::text::Span;
