//! Canvas: braille dot-grid drawing for points and lines in user coordinates.
//!
//! Each terminal cell maps to one braille character covering a 2×4 dot grid,
//! giving a resolution of `2×width × 4×height` dots. Coordinates are given in
//! user space (`x_bounds` / `y_bounds`) and mapped onto that dot grid with the
//! y-axis inverted (bounds are in mathematical orientation: y grows upward).
//!
//! ```
//! use inline_tui::{buffer::Buffer, layout::Rect, widgets::{Canvas, Widget}};
//!
//! let c = Canvas::new()
//!     .x_bounds([0.0, 1.0])
//!     .y_bounds([0.0, 1.0])
//!     .point(0.0, 1.0); // top-left dot
//! let mut b = Buffer::empty(Rect::new(0, 0, 2, 1));
//! c.render(Rect::new(0, 0, 2, 1), &mut b);
//! assert_ne!(b.get(0, 0).unwrap().symbol, "⠀"); // blank braille is U+2800
//! ```

use crate::buffer::Buffer;
use crate::layout::Rect;
use crate::style::Style;
use crate::widgets::Widget;

const DOT_BITS: [[u8; 4]; 2] = [
    [0x01, 0x02, 0x04, 0x40], // left column
    [0x08, 0x10, 0x20, 0x80], // right column
];

fn braille_char(bits: u8) -> char {
    // bits is 0..=0xFF; 0x2800 + bits is always a valid braille pattern.
    char::from_u32(0x2800 + bits as u32).unwrap_or('\u{2800}')
}

/// A canvas collecting points and line segments, rendered as braille dots.
#[derive(Debug, Clone, Default)]
pub struct Canvas {
    x_bounds: [f64; 2],
    y_bounds: [f64; 2],
    style: Style,
    points: Vec<(f64, f64)>,
    lines: Vec<([f64; 2], [f64; 2])>,
}

impl Canvas {
    pub fn new() -> Canvas {
        Canvas {
            x_bounds: [0.0, 1.0],
            y_bounds: [0.0, 1.0],
            style: Style::new(),
            points: Vec::new(),
            lines: Vec::new(),
        }
    }

    /// Horizontal user-space bounds `[min, max]`.
    pub fn x_bounds(mut self, b: [f64; 2]) -> Canvas {
        self.x_bounds = b;
        self
    }

    /// Vertical user-space bounds `[min, max]` (y grows upward).
    pub fn y_bounds(mut self, b: [f64; 2]) -> Canvas {
        self.y_bounds = b;
        self
    }

    pub fn style(mut self, st: Style) -> Canvas {
        self.style = st;
        self
    }

    /// Adds a point at user coordinates.
    pub fn point(mut self, x: f64, y: f64) -> Canvas {
        self.points.push((x, y));
        self
    }

    /// Adds a line segment between two user-space points.
    pub fn line(mut self, x1: f64, y1: f64, x2: f64, y2: f64) -> Canvas {
        self.lines.push(([x1, y1], [x2, y2]));
        self
    }

    /// Adds a polyline: consecutive segments through the given points.
    pub fn polyline(mut self, pts: &[(f64, f64)]) -> Canvas {
        for w in pts.windows(2) {
            self.lines.push(([w[0].0, w[0].1], [w[1].0, w[1].1]));
        }
        for &p in pts {
            self.points.push(p);
        }
        self
    }
}

impl Widget for Canvas {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let cols = area.width as usize;
        let rows = area.height as usize;
        let cols2 = (cols * 2) as f64;
        let rows4 = (rows * 4) as f64;
        let xspan = self.x_bounds[1] - self.x_bounds[0];
        let yspan = self.y_bounds[1] - self.y_bounds[0];
        if xspan <= 0.0 || yspan <= 0.0 {
            return;
        }
        let mut grid = vec![0u8; cols * rows];
        let set_dot = |gx: i32, gy: i32, grid: &mut [u8]| {
            if gx < 0 || gy < 0 || gx >= cols2 as i32 || gy >= rows4 as i32 {
                return;
            }
            let cell = (gy as usize / 4) * cols + gx as usize / 2;
            grid[cell] |= DOT_BITS[gx as usize % 2][gy as usize % 4];
        };
        // User space → dot grid (y inverted: top row = max y).
        let gx = |x: f64| -> i32 {
            (((x - self.x_bounds[0]) / xspan) * (cols2 - 1.0))
                .round()
                .clamp(0.0, cols2 - 1.0) as i32
        };
        let gy = |y: f64| -> i32 {
            (((self.y_bounds[1] - y) / yspan) * (rows4 - 1.0))
                .round()
                .clamp(0.0, rows4 - 1.0) as i32
        };

        for &(x, y) in &self.points {
            set_dot(gx(x), gy(y), &mut grid);
        }
        for (&[x1, y1], &[x2, y2]) in self.lines.iter().map(|(a, b)| (a, b)) {
            // Bresenham over the dot grid.
            let (mut x, mut y) = (gx(x1), gy(y1));
            let (x2, y2) = (gx(x2), gy(y2));
            let dx = (x2 - x).abs();
            let sx = if x2 >= x { 1 } else { -1 };
            let dy = -(y2 - y).abs();
            let sy = if y2 >= y { 1 } else { -1 };
            let mut err = dx + dy;
            loop {
                set_dot(x, y, &mut grid);
                if x == x2 && y == y2 {
                    break;
                }
                let e2 = 2 * err;
                if e2 >= dy {
                    err += dy;
                    x += sx;
                }
                if e2 <= dx {
                    err += dx;
                    y += sy;
                }
            }
        }

        for (i, bits) in grid.iter().enumerate() {
            let x = area.x + (i % cols) as u16;
            let y = area.y + (i / cols) as u16;
            buf.set_string(x, y, &braille_char(*bits).to_string(), self.style);
        }
    }
}
