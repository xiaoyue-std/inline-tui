//! Table: header + data rows, constrained column widths, optional row selection and scrolling.

use crate::buffer::Buffer;
use crate::layout::{solve, Constraint, Rect};
use crate::style::Style;
use crate::text::{truncate_line, Line, Span};
use crate::width::str_width;
use crate::widgets::Widget;

/// Scroll/selection state of a table.
#[derive(Debug, Clone, Default)]
pub struct TableState {
    pub offset: usize,
    pub selected: Option<usize>,
}

impl TableState {
    pub fn new() -> TableState {
        TableState::default()
    }

    pub fn select(&mut self, idx: Option<usize>) {
        self.selected = idx;
    }

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

    pub fn previous(&mut self) {
        match self.selected {
            Some(0) | None => {}
            Some(cur) => self.selected = Some(cur - 1),
        }
    }

    /// Clamps the offset so the selected row is visible.
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

/// Table. Column widths are described with [`Constraint`]; row content is truncated by display width.
#[derive(Debug, Clone)]
pub struct Table<'a> {
    header: Vec<String>,
    rows: &'a [Vec<String>],
    widths: Vec<Constraint>,
    column_spacing: u16,
    state: &'a TableState,
    header_style: Style,
    style: Style,
    highlight_style: Style,
    highlight_symbol: String,
}

impl<'a> Table<'a> {
    pub fn new(header: impl IntoIterator<Item = &'a str>, rows: &'a [Vec<String>], widths: Vec<Constraint>, state: &'a TableState) -> Table<'a> {
        Table {
            header: header.into_iter().map(str::to_string).collect(),
            rows,
            widths,
            column_spacing: 2,
            state,
            header_style: Style::new(),
            style: Style::new(),
            highlight_style: Style::new(),
            highlight_symbol: String::new(),
        }
    }

    pub fn column_spacing(mut self, n: u16) -> Table<'a> {
        self.column_spacing = n;
        self
    }

    pub fn styles(mut self, normal: Style, header: Style, highlight: Style) -> Table<'a> {
        self.style = normal;
        self.header_style = header;
        self.highlight_style = highlight;
        self
    }

    pub fn highlight_symbol(mut self, s: impl Into<String>) -> Table<'a> {
        self.highlight_symbol = s.into();
        self
    }

    fn column_rects(&self, area: Rect) -> Vec<Rect> {
        let n = self.widths.len();
        if n == 0 || area.width == 0 {
            return Vec::new();
        }
        let spacing_total = self.column_spacing.saturating_mul(n as u16);
        let avail = area.width.saturating_sub(spacing_total);
        let widths = solve(avail, &self.widths);
        let mut x = area.x;
        widths
            .into_iter()
            .map(|w| {
                let r = Rect { x, y: area.y, width: w, height: area.height };
                x += w + self.column_spacing;
                r
            })
            .collect()
    }
}

impl Widget for Table<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let cols = self.column_rects(area);
        if cols.is_empty() {
            return;
        }
        let mut y = area.y;

        // Header
        if !self.header.is_empty() && area.height > 0 {
            for (ci, cell) in self.header.iter().enumerate() {
                if ci >= cols.len() {
                    break;
                }
                let l = Line::styled(cell.clone(), self.header_style);
                buf.set_line(cols[ci].x, y, &truncate_line(&l, cols[ci].width as usize));
            }
            y += 1;
        }

        // Data rows
        let body_h = area.bottom().saturating_sub(y) as usize;
        let sym_w = str_width(&self.highlight_symbol);
        for (i, row) in self.rows.iter().skip(self.state.offset).take(body_h).enumerate() {
            let idx = self.state.offset + i;
            let selected = self.state.selected == Some(idx);
            let style = if selected { self.highlight_style } else { self.style };
            for (ci, cell) in row.iter().enumerate() {
                if ci >= cols.len() {
                    break;
                }
                let mut l = Line::empty();
                if ci == 0 && !self.highlight_symbol.is_empty() {
                    l.spans.push(if selected {
                        Span::styled(self.highlight_symbol.clone(), style)
                    } else {
                        Span::raw(" ".repeat(sym_w))
                    });
                }
                l.spans.push(Span::styled(cell.clone(), style));
                let limit = cols[ci].width.saturating_sub(if ci == 0 { sym_w as u16 } else { 0 });
                buf.set_line(cols[ci].x, y, &truncate_line(&l, limit as usize));
            }
            y += 1;
        }
    }
}
