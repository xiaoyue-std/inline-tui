//! Scrollbar: a visual scroll indicator rendered at the edge of an area (used with
//! Viewport/content areas).

use crate::buffer::Buffer;
use crate::layout::Rect;
use crate::style::Style;
use crate::widgets::Widget;

/// Scrollbar orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollOrientation {
    Vertical,
    Horizontal,
}

/// Scrollbar. Rendered at the right edge (vertical) or bottom edge (horizontal) of the
/// given area.
///
/// Provide content length / viewport length / position via [`Scrollbar::metrics`]
/// before rendering.
#[derive(Debug, Clone)]
pub struct Scrollbar {
    orientation: ScrollOrientation,
    track_symbol: String,
    thumb_symbol: String,
    style: Style,
}

impl Default for Scrollbar {
    fn default() -> Scrollbar {
        Scrollbar {
            orientation: ScrollOrientation::Vertical,
            track_symbol: "│".to_string(),
            thumb_symbol: "█".to_string(),
            style: Style::new(),
        }
    }
}

impl Scrollbar {
    pub fn new() -> Scrollbar {
        Scrollbar::default()
    }

    pub fn orientation(mut self, o: ScrollOrientation) -> Scrollbar {
        self.orientation = o;
        self
    }

    pub fn symbols(mut self, track: impl Into<String>, thumb: impl Into<String>) -> Scrollbar {
        self.track_symbol = track.into();
        self.thumb_symbol = thumb.into();
        self
    }

    pub fn style(mut self, st: Style) -> Scrollbar {
        self.style = st;
        self
    }

    /// Renders the scrollbar. `pos` is the index of the first viewport row (column) within the content.
    pub fn render_with_metrics(self, area: Rect, buf: &mut Buffer, content_len: usize, viewport_len: usize, pos: usize) {
        if area.is_empty() || content_len == 0 || viewport_len == 0 {
            return;
        }
        let track_len = match self.orientation {
            ScrollOrientation::Vertical => area.height as usize,
            ScrollOrientation::Horizontal => area.width as usize,
        } as u32;
        if track_len == 0 {
            return;
        }
        let content = content_len as u32;
        let view = (viewport_len as u32).min(content);
        let thumb_len = ((view * track_len) / content).max(1).min(track_len);
        let max_pos = (content - view).max(1);
        let pos = (pos as u32).min(max_pos);
        let thumb_pos = ((pos * (track_len - thumb_len)) / max_pos).min(track_len - thumb_len);

        for i in 0..track_len as u16 {
            let (x, y) = match self.orientation {
                ScrollOrientation::Vertical => (area.right().saturating_sub(1), area.y + i),
                ScrollOrientation::Horizontal => (area.x + i, area.bottom().saturating_sub(1)),
            };
            let sym = if (i as u32) >= thumb_pos && (i as u32) < thumb_pos + thumb_len {
                &self.thumb_symbol
            } else {
                &self.track_symbol
            };
            buf.set_string(x, y, sym, self.style);
        }
    }
}

impl Widget for Scrollbar {
    fn render(self, _area: Rect, _buf: &mut Buffer) {
        // The scrollbar needs content metrics; use render_with_metrics.
    }
}
