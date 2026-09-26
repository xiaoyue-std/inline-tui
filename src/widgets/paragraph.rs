//! Paragraph: wrappable, scrollable rich text.

use crate::buffer::Buffer;
use crate::layout::Rect;
use crate::style::Style;
use crate::text::{wrap_line, Line, Text};
use crate::widgets::Widget;

/// Rich text paragraph. Wraps automatically by default; `scroll` is the number of skipped lines.
#[derive(Debug, Clone)]
pub struct Paragraph {
    text: Text,
    style: Style,
    wrap: bool,
    scroll: u16,
}

impl Paragraph {
    pub fn new(text: impl Into<Text>) -> Paragraph {
        Paragraph { text: text.into(), style: Style::new(), wrap: true, scroll: 0 }
    }

    pub fn style(mut self, st: Style) -> Paragraph {
        self.style = st;
        self
    }

    pub fn wrap(mut self, on: bool) -> Paragraph {
        self.wrap = on;
        self
    }

    pub fn scroll(mut self, rows: u16) -> Paragraph {
        self.scroll = rows;
        self
    }
}

impl From<&str> for Paragraph {
    fn from(s: &str) -> Paragraph {
        Paragraph::new(Text::raw(s))
    }
}

impl From<Text> for Paragraph {
    fn from(t: Text) -> Paragraph {
        Paragraph::new(t)
    }
}

impl Widget for Paragraph {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let mut visual: Vec<Line> = Vec::new();
        for line in &self.text.lines {
            let mut l = Line { spans: Vec::new(), style: self.style.patch(line.style) };
            l.spans.extend(line.spans.iter().cloned());
            if self.wrap {
                visual.extend(wrap_line(&l, area.width as usize));
            } else {
                visual.push(l);
            }
        }
        for (i, line) in visual.iter().skip(self.scroll as usize).enumerate() {
            let y = area.y + i as u16;
            if y >= area.bottom() {
                break;
            }
            buf.set_line(area.x, y, line);
        }
    }
}
