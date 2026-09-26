//! Sparkline: draws data as a multi-row bar chart using block characters (▁▂▃▄▅▆▇█).

use crate::buffer::Buffer;
use crate::layout::Rect;
use crate::style::Style;
use crate::widgets::Widget;

/// 8-level bar characters (from empty to full).
const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Sparkline: data → bar chart, filling the given area (multi-row stacks from the bottom up).
#[derive(Debug, Clone)]
pub struct Sparkline<'a> {
    data: &'a [u64],
    max: Option<u64>,
    style: Style,
}

impl<'a> Sparkline<'a> {
    pub fn new(data: &'a [u64]) -> Sparkline<'a> {
        Sparkline { data, max: None, style: Style::new() }
    }

    /// Data maximum (defines the full-scale); defaults to the data's own maximum.
    pub fn max(mut self, v: u64) -> Sparkline<'a> {
        self.max = Some(v);
        self
    }

    pub fn style(mut self, st: Style) -> Sparkline<'a> {
        self.style = st;
        self
    }
}

impl Widget for Sparkline<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() || self.data.is_empty() {
            return;
        }
        let max = match self.max {
            Some(m) if m > 0 => m as f32,
            _ => self.data.iter().copied().max().unwrap_or(1).max(1) as f32,
        };
        let rows = area.height as usize;
        for (i, &v) in self.data.iter().enumerate() {
            let x = area.x + i as u16;
            if x >= area.right() {
                break;
            }
            let level = ((v as f32 / max) * (rows as f32 * 8.0)).ceil() as usize; // total cells
            for r in 0..rows {
                // r = number of rows from the bottom; this row covers the cell range [r*8, r*8+8)
                let cells_in_row = level.saturating_sub(r * 8).min(8);
                if cells_in_row == 0 {
                    break;
                }
                let ch = BARS[cells_in_row - 1];
                let y = area.bottom() - 1 - r as u16;
                buf.set_cell(x, y, ch, self.style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Color;

    #[test]
    fn single_row_bars() {
        let data = [0u64, 4, 8];
        let mut b = Buffer::empty(Rect::new(0, 0, 3, 1));
        Sparkline::new(&data).render(Rect::new(0, 0, 3, 1), &mut b);
        assert_eq!(b.get(0, 0).unwrap().symbol, " ");
        assert_eq!(b.get(1, 0).unwrap().symbol, "▄"); // 4/8 half cell
        assert_eq!(b.get(2, 0).unwrap().symbol, "█");
    }

    #[test]
    fn multi_row_stacks_from_bottom() {
        let data = [8u64];
        let mut b = Buffer::empty(Rect::new(0, 0, 1, 2));
        Sparkline::new(&data).max(8).render(Rect::new(0, 0, 1, 2), &mut b);
        assert_eq!(b.get(0, 0).unwrap().symbol, "█"); // top row
        assert_eq!(b.get(0, 1).unwrap().symbol, "█"); // bottom row
        let data = [4u64];
        let mut b = Buffer::empty(Rect::new(0, 0, 1, 2));
        Sparkline::new(&data).max(8).render(Rect::new(0, 0, 1, 2), &mut b);
        assert_eq!(b.get(0, 0).unwrap().symbol, " "); // top row empty
        assert_eq!(b.get(0, 1).unwrap().symbol, "█"); // bottom row full
    }

    #[test]
    fn styled() {
        let data = [5u64];
        let mut b = Buffer::empty(Rect::new(0, 0, 1, 1));
        Sparkline::new(&data).style(Style::new().fg(Color::Red)).render(Rect::new(0, 0, 1, 1), &mut b);
        assert_eq!(b.get(0, 0).unwrap().fg, Color::Red);
    }
}
