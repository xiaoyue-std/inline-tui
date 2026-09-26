//! Collapsible panel: tool-call card style (▸ collapsed / ▾ expanded).

use crate::buffer::Buffer;
use crate::layout::Rect;
use crate::style::Style;
use crate::text::{truncate_line, Line, Text};
use crate::widgets::Widget;

/// Collapsible content block.
#[derive(Debug, Clone)]
pub struct Collapsible<'a> {
    title: &'a str,
    open: bool,
    content: &'a Text,
    title_style: Style,
    content_style: Style,
    indent: usize,
}

impl<'a> Collapsible<'a> {
    pub fn new(title: &'a str, open: bool, content: &'a Text) -> Collapsible<'a> {
        Collapsible {
            title,
            open,
            content,
            title_style: Style::new(),
            content_style: Style::new(),
            indent: 2,
        }
    }

    pub fn title_style(mut self, st: Style) -> Collapsible<'a> {
        self.title_style = st;
        self
    }

    pub fn content_style(mut self, st: Style) -> Collapsible<'a> {
        self.content_style = st;
        self
    }

    pub fn indent(mut self, n: usize) -> Collapsible<'a> {
        self.indent = n;
        self
    }

    /// Height required to render.
    pub fn height(&self) -> u16 {
        if self.open {
            1 + self.content.lines.len() as u16
        } else {
            1
        }
    }
}

impl Widget for Collapsible<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let arrow = if self.open { "▾ " } else { "▸ " };
        let mut title = Line::empty();
        title.push_span(crate::text::Span::styled(arrow.to_string(), self.title_style));
        title.push_span(crate::text::Span::styled(self.title.to_string(), self.title_style));
        buf.set_line(area.x, area.y, &truncate_line(&title, area.width as usize));

        if self.open {
            let pad = " ".repeat(self.indent);
            for (i, line) in self.content.lines.iter().enumerate() {
                let y = area.y + 1 + i as u16;
                if y >= area.bottom() {
                    break;
                }
                let mut l = Line { spans: Vec::new(), style: self.content_style };
                if self.indent > 0 {
                    l.spans.push(crate::text::Span::raw(pad.clone()));
                }
                l.spans.extend(line.spans.iter().cloned());
                buf.set_line(area.x, y, &truncate_line(&l, area.width as usize));
            }
        }
    }
}
