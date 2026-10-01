//! Gauge: a single-row percentage bar with a centered label.

use crate::buffer::Buffer;
use crate::layout::Rect;
use crate::style::{Color, Modifier, Style};
use crate::widgets::Widget;
use crate::width::str_width;

/// Progress gauge (0.0–1.0): the bar is drawn with `█` cells — the filled
/// prefix uses `style`, the unfilled remainder uses `track` — and the label
/// (default `NN%`) is centered on the bar in a reversed band so it stays
/// readable over both halves.
#[derive(Debug, Clone)]
pub struct Gauge {
    percent: f32,
    label: Option<String>,
    style: Style,
    track: Style,
}

impl Gauge {
    /// Creates a gauge; `percent` is clamped to 0.0–1.0.
    pub fn new(percent: f32) -> Gauge {
        Gauge {
            percent: percent.clamp(0.0, 1.0),
            label: None,
            style: Style::new().fg(Color::Indexed(252)),
            track: Style::new().fg(Color::Indexed(238)),
        }
    }

    /// Overrides the default `NN%` label (drawn centered, reversed band).
    pub fn label(mut self, label: impl Into<String>) -> Gauge {
        self.label = Some(label.into());
        self
    }

    /// Style of the filled portion.
    pub fn style(mut self, st: Style) -> Gauge {
        self.style = st;
        self
    }

    /// Style of the unfilled remainder.
    pub fn track(mut self, st: Style) -> Gauge {
        self.track = st;
        self
    }
}

impl Widget for Gauge {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let w = area.width as usize;
        let filled = (self.percent * w as f32).round() as usize;
        for i in 0..w {
            let st = if i < filled { self.style } else { self.track };
            buf.set_cell(area.x + i as u16, area.y, '█', st);
        }
        let label = self
            .label
            .unwrap_or_else(|| format!("{:.0}%", self.percent * 100.0));
        let lw = str_width(&label).min(w);
        let lx = area.x + ((w - lw) / 2) as u16;
        buf.set_string(
            lx,
            area.y,
            &label.chars().take(lw).collect::<String>(),
            Style::new().add_modifier(Modifier::REVERSED),
        );
    }
}
