//! Diff view: unified diff rendering with colored +/− lines.

use crate::buffer::Buffer;
use crate::layout::Rect;
use crate::style::Style;
use crate::text::{Line, Span};
use crate::widgets::Widget;

/// Parses unified diff text into colored lines (reused by line-based viewports and [`DiffView`]).
pub fn diff_lines(
    raw: &str,
    style_add: Style,
    style_del: Style,
    style_hunk: Style,
    style_meta: Style,
) -> Vec<Line> {
    let dim = Style::new().fg(crate::style::Color::Indexed(240));
    raw.lines()
        .map(|line| {
            let mut l = Line::empty();
            let style = if line.starts_with("diff --git")
                || line.starts_with("index ")
                || line.starts_with("--- ")
                || line.starts_with("+++ ")
            {
                style_meta
            } else if line.starts_with("@@") {
                style_hunk
            } else if line.starts_with('+') {
                style_add
            } else if line.starts_with('-') {
                style_del
            } else {
                dim
            };
            l.spans.push(Span::styled(line.to_string(), style));
            l
        })
        .collect()
}

/// Unified diff text renderer.
#[derive(Debug, Clone)]
pub struct DiffView<'a> {
    raw: &'a str,
    style_add: Style,
    style_del: Style,
    style_hunk: Style,
    style_meta: Style,
}

impl<'a> DiffView<'a> {
    pub fn new(raw: &'a str) -> DiffView<'a> {
        DiffView {
            raw,
            style_add: Style::new(),
            style_del: Style::new(),
            style_hunk: Style::new(),
            style_meta: Style::new(),
        }
    }

    pub fn styles(mut self, add: Style, del: Style, hunk: Style, meta: Style) -> DiffView<'a> {
        self.style_add = add;
        self.style_del = del;
        self.style_hunk = hunk;
        self.style_meta = meta;
        self
    }

    /// Height required to render (line count).
    pub fn height(&self) -> u16 {
        self.raw.lines().count() as u16
    }
}

impl Widget for DiffView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let lines = diff_lines(self.raw, self.style_add, self.style_del, self.style_hunk, self.style_meta);
        for (i, line) in lines.iter().enumerate() {
            let y = area.y + i as u16;
            if y >= area.bottom() {
                break;
            }
            buf.set_line(area.x, y, &crate::text::truncate_line(line, area.width as usize));
        }
    }
}
