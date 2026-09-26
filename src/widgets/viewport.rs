//! Scroll viewport: message history area + scroll state (supports "follow bottom").

use crate::buffer::Buffer;
use crate::layout::Rect;
use crate::text::{truncate_line, Line};
use crate::widgets::Widget;

/// Scroll state, held by the application.
#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    /// Scroll offset from the top (index of the first visible line).
    pub offset: usize,
    /// Whether to follow the bottom of the content (auto-scrolls on new content).
    pub follow: bool,
}

impl ScrollState {
    /// Call before rendering: sticks to the bottom in follow mode and clamps the
    /// offset into the valid range.
    pub fn sync(&mut self, content_height: usize, visible: usize) {
        let visible = visible.max(1);
        if self.follow {
            self.offset = content_height.saturating_sub(visible);
        }
        self.offset = self.offset.min(content_height.saturating_sub(visible));
    }

    /// Scrolls up n lines (leaves follow mode).
    pub fn scroll_up(&mut self, n: usize) {
        self.offset = self.offset.saturating_sub(n);
        self.follow = false;
    }

    /// Scrolls down n lines; resumes following when reaching the bottom.
    pub fn scroll_down(&mut self, n: usize, content_height: usize, visible: usize) {
        let visible = visible.max(1);
        self.offset = (self.offset + n).min(content_height.saturating_sub(visible));
        if self.offset + visible >= content_height {
            self.follow = true;
        }
    }
}

/// Scrollable view of a line collection. The application computes `offset` itself with `ScrollState`.
#[derive(Debug, Clone)]
pub struct Viewport<'a> {
    lines: &'a [Line],
    offset: usize,
}

impl<'a> Viewport<'a> {
    pub fn new(lines: &'a [Line], offset: usize) -> Viewport<'a> {
        Viewport { lines, offset }
    }
}

impl Widget for Viewport<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        for (i, line) in self.lines.iter().skip(self.offset).enumerate() {
            let y = area.y + i as u16;
            if y >= area.bottom() {
                break;
            }
            let l = truncate_line(line, area.width as usize);
            buf.set_line(area.x, y, &l);
        }
    }
}
