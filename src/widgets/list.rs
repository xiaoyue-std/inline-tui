//! List: selectable, scrollable item list (state is held by the application).

use crate::buffer::Buffer;
use crate::layout::Rect;
use crate::style::Style;
use crate::text::{truncate_line, Line};
use crate::widgets::Widget;

/// Scroll/selection state of a list.
#[derive(Debug, Clone, Default)]
pub struct ListState {
    pub offset: usize,
    pub selected: Option<usize>,
}

impl ListState {
    pub fn new() -> ListState {
        ListState::default()
    }

    pub fn select(&mut self, idx: Option<usize>) {
        self.selected = idx;
    }

    /// Selects the next item (stops at the end).
    pub fn next(&mut self, total: usize) {
        let Some(cur) = self.selected else {
            if total > 0 {
                self.selected = Some(0);
            }
            return;
        };
        if cur + 1 < total {
            self.selected = Some(cur + 1);
        }
    }

    /// Selects the previous item.
    pub fn previous(&mut self) {
        match self.selected {
            Some(0) | None => {}
            Some(cur) => self.selected = Some(cur - 1),
        }
    }

    /// Clamps the offset so the selected item is visible (call before rendering).
    pub fn sync(&mut self, total: usize, visible: usize) {
        let visible = visible.max(1);
        if let Some(sel) = self.selected {
            if sel < self.offset {
                self.offset = sel;
            } else if sel >= self.offset + visible {
                self.offset = sel + 1 - visible;
            }
        }
        self.offset = self.offset.min(total.saturating_sub(1));
    }
}

/// Selectable list.
#[derive(Debug, Clone)]
pub struct List<'a> {
    items: &'a [Line],
    state: &'a ListState,
    highlight_symbol: String,
    highlight_style: Style,
    style: Style,
}

impl<'a> List<'a> {
    pub fn new(items: &'a [Line], state: &'a ListState) -> List<'a> {
        List {
            items,
            state,
            highlight_symbol: "› ".to_string(),
            highlight_style: Style::new(),
            style: Style::new(),
        }
    }

    pub fn highlight_symbol(mut self, s: impl Into<String>) -> List<'a> {
        self.highlight_symbol = s.into();
        self
    }

    pub fn highlight_style(mut self, st: Style) -> List<'a> {
        self.highlight_style = st;
        self
    }

    pub fn style(mut self, st: Style) -> List<'a> {
        self.style = st;
        self
    }
}

impl Widget for List<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let visible = area.height as usize;
        let sym_w = crate::width::str_width(&self.highlight_symbol) as u16;

        for (i, item) in self.items.iter().skip(self.state.offset).take(visible).enumerate() {
            let idx = self.state.offset + i;
            let y = area.y + i as u16;
            let selected = self.state.selected == Some(idx);
            let mut line = Line { spans: Vec::new(), style: self.style };
            if selected {
                line.spans.push(crate::text::Span::styled(
                    self.highlight_symbol.clone(),
                    self.highlight_style,
                ));
            } else if !self.highlight_symbol.is_empty() {
                line.spans.push(crate::text::Span::raw(" ".repeat(sym_w as usize)));
            }
            line.spans.extend(item.spans.iter().cloned());
            let line = if selected { Line { spans: line.spans, style: self.highlight_style } } else { line };
            buf.set_line(area.x, y, &truncate_line(&line, area.width as usize));
        }
    }
}
