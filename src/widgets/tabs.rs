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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{Color, Modifier};

    #[test]
    fn renders_with_selection() {
        let mut b = Buffer::empty(Rect::new(0, 0, 30, 1));
        Tabs::new(["List", "Table", "Editor"])
            .selected(1)
            .styles(
                Style::new().fg(Color::Gray),
                Style::new().fg(Color::White).add_modifier(Modifier::BOLD),
            )
            .render(Rect::new(0, 0, 30, 1), &mut b);
        let r: String = (0..30).map(|x| b.get(x, 0).unwrap().symbol.clone()).collect();
        assert!(r.contains("List"));
        assert!(r.contains("Table"));
        // The selected item uses the highlight style
        let table_pos = r.find("Table").unwrap();
        assert_eq!(b.get(table_pos as u16, 0).unwrap().modifier, Modifier::BOLD);
        let list_pos = r.find("List").unwrap();
        assert_eq!(b.get(list_pos as u16, 0).unwrap().fg, Color::Gray);
    }
}
