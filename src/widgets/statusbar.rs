//! Status bar: bottom hint strip (shortcut hints etc.).

use crate::buffer::Buffer;
use crate::layout::Rect;
use crate::style::Style;
use crate::text::{pad_line, Line, Span};
use crate::widgets::Widget;

/// Status bar: a set of spans rendered as one line, with the right side filled to the
/// full row width using the background.
#[derive(Debug, Clone)]
pub struct StatusBar {
    spans: Vec<Span>,
    style: Style,
}

impl StatusBar {
    pub fn new(spans: Vec<Span>) -> StatusBar {
        StatusBar { spans, style: Style::new() }
    }

    pub fn style(mut self, st: Style) -> StatusBar {
        self.style = st;
        self
    }

    /// Shortcut hint list: `(key, label)`, e.g. `ctrl+c quit · pgup page`.
    pub fn hints(hints: &[(&str, &str)]) -> StatusBar {
        let mut spans = Vec::new();
        for (i, (key, label)) in hints.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" · ".to_string(), Style::new()));
            }
            spans.push(Span::styled(key.to_string(), Style::new()));
            spans.push(Span::styled(format!(" {}", label), Style::new()));
        }
        StatusBar::new(spans)
    }

    /// Produces the span sequence for a hint strip (apps can freely concatenate, e.g. append a Spinner).
    pub fn hint_spans(hints: &[(&str, &str)], key_style: Style, label_style: Style, sep_style: Style) -> Vec<Span> {
        let mut spans = Vec::new();
        for (i, (key, label)) in hints.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" · ".to_string(), sep_style));
            }
            spans.push(Span::styled(key.to_string(), key_style));
            spans.push(Span::styled(format!(" {}", label), label_style));
        }
        spans
    }
}

impl Widget for StatusBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() || area.height == 0 {
            return;
        }
        let mut line = Line { spans: self.spans, style: self.style };
        line = pad_line(&line, area.width as usize);
        let line = crate::text::truncate_line(&line, area.width as usize);
        buf.set_line(area.x, area.y, &line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_row() {
        let mut b = Buffer::empty(Rect::new(0, 0, 20, 1));
        StatusBar::hints(&[("ctrl+c", "退出")]).render(Rect::new(0, 0, 20, 1), &mut b);
        let row: String = (0..20).map(|x| b.get(x, 0).unwrap().symbol.clone()).collect();
        assert_eq!(row.trim_end().len(), "ctrl+c 退出".len());
        assert!(row.ends_with(' ')); // background fill
    }
}
