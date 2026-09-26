//! Selection menu: slash command completion, selection dialogs.

use crate::buffer::Buffer;
use crate::layout::Rect;
use crate::style::Style;
use crate::text::{Span, Line};
use crate::widgets::{Block, BorderType, Widget};
use crate::width::str_width;

/// Pop-up selection menu.
///
/// Constructed by the application, which passes the currently filtered items and the
/// selected index; reserve space with [`Menu::height`] during layout (usually drawn
/// above the input box).
#[derive(Debug, Clone)]
pub struct Menu {
    items: Vec<String>,
    selected: usize,
    max_visible: usize,
    title: Option<String>,
    block: Block,
    selected_style: Style,
    normal_style: Style,
    cursor_symbol: String,
}

impl Menu {
    pub fn new<'a>(items: impl IntoIterator<Item = &'a str>) -> Menu {
        Menu {
            items: items.into_iter().map(str::to_string).collect(),
            selected: 0,
            max_visible: 8,
            title: None,
            block: Block::new().border_type(BorderType::Rounded),
            selected_style: Style::new(),
            normal_style: Style::new(),
            cursor_symbol: "›".to_string(),
        }
    }

    pub fn selected(mut self, idx: usize) -> Menu {
        self.selected = idx;
        self
    }

    pub fn max_visible(mut self, n: usize) -> Menu {
        self.max_visible = n.max(1);
        self
    }

    pub fn title(mut self, t: impl Into<String>) -> Menu {
        self.title = Some(t.into());
        self
    }

    pub fn block(mut self, b: Block) -> Menu {
        self.block = b;
        self
    }

    pub fn selected_style(mut self, st: Style) -> Menu {
        self.selected_style = st;
        self
    }

    pub fn normal_style(mut self, st: Style) -> Menu {
        self.normal_style = st;
        self
    }

    /// Height required to render the menu (borders included).
    pub fn height(&self) -> u16 {
        menu_height(self.items.len(), self.max_visible)
    }

    /// Height required to render the menu.
    pub fn menu_height(n_items: usize, max_visible: usize) -> u16 {
        menu_height(n_items, max_visible)
    }
}

/// Number of rows the menu needs: visible items + top and bottom borders.
pub fn menu_height(n_items: usize, max_visible: usize) -> u16 {
    (n_items.min(max_visible.max(1)) as u16) + 2
}

impl Widget for Menu {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() || self.items.is_empty() {
            return;
        }
        let inner = self.block.inner(area);
        let title: Option<String> = self.title.clone();
        let mut block = self.block;
        if let Some(t) = title {
            block = block.title(Line::raw(t));
        }
        block.render(area, buf);
        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let visible = inner.height as usize;
        // Keep the selected item visible
        let offset = self.selected.saturating_sub(visible.saturating_sub(1));
        for (i, item) in self.items.iter().skip(offset).take(visible).enumerate() {
            let idx = offset + i;
            let y = inner.y + i as u16;
            let selected = idx == self.selected;
            let mut line = Line::empty();
            let cursor_w = str_width(&self.cursor_symbol) + 1;
            if selected {
                line.push_span(Span::styled(
                    format!("{} ", self.cursor_symbol),
                    self.selected_style,
                ));
            } else {
                line.spans.push(Span::raw(" ".repeat(cursor_w)));
            }
            let item_st = if selected { self.selected_style } else { self.normal_style };
            line.push_span(Span::styled(item.clone(), item_st));
            buf.set_line(inner.x, y, &crate::text::truncate_line(&line, inner.width as usize));
        }
    }
}
