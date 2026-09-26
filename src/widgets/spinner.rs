//! Spinner: frame animation + elapsed time display.

use crate::buffer::Buffer;
use crate::layout::Rect;
use crate::style::Style;
use crate::text::Span;
use crate::widgets::Widget;
use std::time::Instant;

/// Braille dot matrix (fine-grained rotation).
pub const BRAILLE: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
/// Star symbol frames.
pub const STARS: &[&str] = &["·", "✢", "✳", "∗", "✻", "✽"];

/// Loading animation. Pass `Instant::now()` as `start` in general; redraw on each frame tick.
#[derive(Debug, Clone)]
pub struct Spinner {
    frames: &'static [&'static str],
    start: Instant,
    style: Style,
    time_style: Style,
    label: String,
    show_elapsed: bool,
}

impl Spinner {
    pub fn new(label: impl Into<String>) -> Spinner {
        Spinner {
            frames: STARS,
            start: Instant::now(),
            style: Style::new(),
            time_style: Style::new(),
            label: label.into(),
            show_elapsed: true,
        }
    }

    pub fn frames(mut self, f: &'static [&'static str]) -> Spinner {
        self.frames = f;
        self
    }

    pub fn style(mut self, st: Style) -> Spinner {
        self.style = st;
        self
    }

    pub fn time_style(mut self, st: Style) -> Spinner {
        self.time_style = st;
        self
    }

    pub fn show_elapsed(mut self, on: bool) -> Spinner {
        self.show_elapsed = on;
        self
    }

    /// Current animation frame index (about 10fps).
    pub fn frame(&self) -> usize {
        (self.start.elapsed().as_millis() / 100) as usize % self.frames.len()
    }

    /// Elapsed seconds.
    pub fn elapsed_secs(&self) -> u64 {
        self.start.elapsed().as_secs()
    }
}

impl Widget for Spinner {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let frame = self.frames[self.frame()];
        let mut line = crate::text::Line::empty();
        line.push_span(Span::styled(frame.to_string(), self.style));
        line.push_span(Span::styled(" ".to_string(), Style::new()));
        line.push_span(Span::styled(self.label.clone(), self.style));
        if self.show_elapsed {
            line.push_span(Span::styled(
                format!(" ({}s)", self.elapsed_secs()),
                self.time_style,
            ));
        }
        buf.set_line(area.x, area.y, &crate::text::truncate_line(&line, area.width as usize));
    }
}
