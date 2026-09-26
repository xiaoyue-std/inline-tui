//! Tabs: horizontal tab bar.

use crate::buffer::Buffer;
use crate::layout::Rect;
use crate::style::Style;
use crate::text::{truncate_line, Line, Span};
use crate::widgets::Widget;

/// Tab bar. The selected item is rendered with the highlight style.
#[derive(Debug, Clone)]
pub struct Tabs {
    titles: Vec<String>,
    selected: usize,
    style: Style,
    highlight_style: Style,
    divider: String,
}

impl Tabs {
    pub fn new<'a>(titles: impl IntoIterator<Item = &'a str>) -> Tabs {
        Tabs {
            titles: titles.into_iter().map(str::to_string).collect(),
            selected: 0,
            style: Style::new(),
            highlight_style: Style::new(),
            divider: "│".to_string(),
        }
    }

    pub fn selected(mut self, idx: usize) -> Tabs {
        self.selected = idx;
        self
    }

    pub fn styles(mut self, normal: Style, highlight: Style) -> Tabs {
        self.style = normal;
        self.highlight_style = highlight;
        self
    }

    pub fn divider(mut self, s: impl Into<String>) -> Tabs {
        self.divider = s.into();
        self
    }
}

impl Widget for Tabs {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() || self.titles.is_empty() {
            return;
        }
        let mut line = Line::empty();
        let div = Span::styled(self.divider.clone(), self.style);
        for (i, title) in self.titles.iter().enumerate() {
            if i > 0 {
                line.spans.push(div.clone());
            }
            let st = if i == self.selected { self.highlight_style } else { self.style };
            line.spans.push(Span::styled(format!(" {} ", title), st));
        }
        buf.set_line(area.x, area.y, &truncate_line(&line, area.width as usize));
    }
}
