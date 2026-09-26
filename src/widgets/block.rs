//! Bordered box: rounded/plain/thick/double borders with a title.

use crate::buffer::Buffer;
use crate::layout::{Margin, Rect};
use crate::style::Style;
use crate::text::Line;
use crate::widgets::Widget;

/// Border drawing position flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Borders(u8);

impl Borders {
    pub const NONE: Borders = Borders(0);
    pub const TOP: Borders = Borders(1);
    pub const RIGHT: Borders = Borders(2);
    pub const BOTTOM: Borders = Borders(4);
    pub const LEFT: Borders = Borders(8);
    pub const ALL: Borders = Borders(15);
}

/// Border character sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderType {
    Plain,
    Rounded,
    Thick,
    Double,
}

impl BorderType {
    fn glyphs(self) -> (char, char, char, char, char, char) {
        // (top-left, top-right, bottom-left, bottom-right, horizontal, vertical)
        match self {
            BorderType::Plain => ('┌', '┐', '└', '┘', '─', '│'),
            BorderType::Rounded => ('╭', '╮', '╰', '╯', '─', '│'),
            BorderType::Thick => ('┏', '┓', '┗', '┛', '━', '┃'),
            BorderType::Double => ('╔', '╗', '╚', '╝', '═', '║'),
        }
    }
}

/// A box with borders and a title.
#[derive(Debug, Clone)]
pub struct Block {
    borders: Borders,
    border_type: BorderType,
    border_style: Style,
    title: Option<Line>,
    padding: Margin,
}

impl Default for Block {
    fn default() -> Block {
        Block {
            borders: Borders::ALL,
            border_type: BorderType::Plain,
            border_style: Style::new(),
            title: None,
            padding: Margin::default(),
        }
    }
}

impl Block {
    pub fn new() -> Block {
        Block::default()
    }

    /// Rounded border.
    pub fn rounded() -> Block {
        Block::new().border_type(BorderType::Rounded)
    }

    pub fn border_type(mut self, t: BorderType) -> Block {
        self.border_type = t;
        self
    }

    pub fn borders(mut self, b: Borders) -> Block {
        self.borders = b;
        self
    }

    pub fn border_style(mut self, st: Style) -> Block {
        self.border_style = st;
        self
    }

    /// The title is drawn at the left of the top edge.
    pub fn title(mut self, title: impl Into<Line>) -> Block {
        self.title = Some(title.into());
        self
    }

    pub fn padding(mut self, horizontal: u16, vertical: u16) -> Block {
        self.padding = Margin::new(horizontal, vertical);
        self
    }

    /// Content area (borders and padding removed).
    pub fn inner(&self, area: Rect) -> Rect {
        let bx = if self.borders.intersects(Borders::LEFT) { 1 } else { 0 };
        let by = if self.borders.intersects(Borders::TOP) { 1 } else { 0 };
        let br = if self.borders.intersects(Borders::RIGHT) { 1 } else { 0 };
        let bb = if self.borders.intersects(Borders::BOTTOM) { 1 } else { 0 };
        let mut r = Rect {
            x: area.x + bx,
            y: area.y + by,
            width: area.width.saturating_sub(bx + br),
            height: area.height.saturating_sub(by + bb),
        };
        r.x += self.padding.horizontal.min(r.width);
        r.y += self.padding.vertical.min(r.height);
        r.width = r.width.saturating_sub(self.padding.horizontal * 2);
        r.height = r.height.saturating_sub(self.padding.vertical * 2);
        r
    }
}

impl Widget for Block {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let (tl, tr, bl, br, hline, vline) = self.border_type.glyphs();

        if self.borders.intersects(Borders::LEFT) {
            for y in area.top()..area.bottom() {
                buf.set_string(area.left(), y, &vline.to_string(), self.border_style);
            }
        }
        if self.borders.intersects(Borders::TOP) {
            for x in area.left()..area.right() {
                buf.set_string(x, area.top(), &hline.to_string(), self.border_style);
            }
        }
        if self.borders.intersects(Borders::RIGHT) {
            let x = area.right().saturating_sub(1);
            for y in area.top()..area.bottom() {
                buf.set_string(x, y, &vline.to_string(), self.border_style);
            }
        }
        if self.borders.intersects(Borders::BOTTOM) {
            let y = area.bottom().saturating_sub(1);
            for x in area.left()..area.right() {
                buf.set_string(x, y, &hline.to_string(), self.border_style);
            }
        }
        // Four corners
        if self.borders.contains_all() {
            buf.set_string(area.left(), area.top(), &tl.to_string(), self.border_style);
            let rx = area.right().saturating_sub(1);
            buf.set_string(rx, area.top(), &tr.to_string(), self.border_style);
            let by = area.bottom().saturating_sub(1);
            buf.set_string(area.left(), by, &bl.to_string(), self.border_style);
            buf.set_string(rx, by, &br.to_string(), self.border_style);
        }
        // Title: inside the top edge, leaving room for the corner on the left
        if let Some(title) = &self.title {
            let bx = if self.borders.intersects(Borders::LEFT) { 1 } else { 0 };
            let tx = area.left() + bx;
            let max_w = area.width.saturating_sub(if self.borders.contains_all() { 2 } else { bx });
            let mut l = Line { style: self.border_style, spans: Vec::new() };
            l.spans.extend(title.spans.iter().cloned());
            buf.set_line(tx, area.top(), &crate::text::truncate_line(&l, max_w as usize));
        }
    }
}

impl Borders {
    fn intersects(self, other: Borders) -> bool {
        self.0 & other.0 != 0
    }

    fn contains_all(self) -> bool {
        self.0 & Borders::ALL.0 == Borders::ALL.0
    }
}
