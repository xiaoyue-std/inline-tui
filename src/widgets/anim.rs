//! Animation effects: color wave, shimmer, pulse, gradient progress bar, loading dots,
//! typewriter, ticker.
//!
//! Design principle: all animations are **pure functions of time** — fix the moment
//! with `.at(elapsed)` at construction, and the render result for the same moment is
//! deterministic (testable). Examples drive it with the frame loop's `Instant::elapsed()`;
//! a tick frequency of ≥ 12fps is recommended (`recv_timeout(80ms)` suffices).

use crate::buffer::Buffer;
use crate::layout::Rect;
use crate::style::{Color, Style};
use crate::text::{Line, Span};
use crate::widgets::Widget;
use crate::width::{char_width, str_width};
use std::time::Duration;

/// RGB linear interpolation. Degrades to picking one side when both ends are not `Color::Rgb`.
pub fn lerp_rgb(a: Color, b: Color, t: f32) -> Color {
    let (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) = (a, b) else {
        return if t < 0.5 { a } else { b };
    };
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color::Rgb(mix(r1, r2), mix(g1, g2), mix(b1, b2))
}

/// Sine breathing pulse between two colors (for brightness undulation of borders/titles).
pub fn pulse_color(a: Color, b: Color, period_ms: u64, elapsed: Duration) -> Color {
    let t = elapsed.as_millis() as f32 / period_ms.max(1) as f32;
    let s = (t * std::f32::consts::TAU).sin() * 0.5 + 0.5;
    lerp_rgb(a, b, s)
}

/// Color wave text: the phase of the i-th visible character shifts with position and
/// time; the color interpolates sinusoidally between a/b.
///
/// Adjacent same-colored characters are merged into one span to control ANSI output volume.
pub fn wave_spans(
    text: &str,
    a: Color,
    b: Color,
    base: Style,
    period_ms: u64,
    wavelength: usize,
    elapsed: Duration,
) -> Vec<Span> {
    let mut spans: Vec<Span> = Vec::new();
    let time_phase = elapsed.as_millis() as f32 / period_ms.max(1) as f32 * std::f32::consts::TAU;
    let wl = wavelength.max(1) as f32;
    let mut i = 0.0f32;
    for ch in text.chars() {
        if char_width(ch) == 0 {
            continue;
        }
        let t = ((time_phase - i / wl).sin() + 1.0) / 2.0;
        let color = lerp_rgb(a, b, t);
        match spans.last_mut() {
            Some(last) if last.style.fg == Some(color) => last.content.push(ch),
            _ => spans.push(Span::styled(ch.to_string(), base.fg(color))),
        }
        i += 1.0;
    }
    spans
}

/// Color wave text widget: text whose colors flow along the character positions.
#[derive(Debug, Clone)]
pub struct Wave<'a> {
    text: &'a str,
    color_a: Color,
    color_b: Color,
    base: Style,
    period_ms: u64,
    wavelength: usize,
    elapsed: Duration,
}

impl<'a> Wave<'a> {
    pub fn new(text: &'a str) -> Wave<'a> {
        Wave {
            text,
            color_a: Color::Rgb(215, 119, 87),
            color_b: Color::Rgb(255, 200, 170),
            base: Style::new(),
            period_ms: 2400,
            wavelength: 10,
            elapsed: Duration::ZERO,
        }
    }

    pub fn colors(mut self, a: Color, b: Color) -> Wave<'a> {
        self.color_a = a;
        self.color_b = b;
        self
    }

    pub fn base_style(mut self, st: Style) -> Wave<'a> {
        self.base = st;
        self
    }

    pub fn wavelength(mut self, n: usize) -> Wave<'a> {
        self.wavelength = n.max(1);
        self
    }

    pub fn period(mut self, ms: u64) -> Wave<'a> {
        self.period_ms = ms.max(1);
        self
    }

    pub fn at(mut self, elapsed: Duration) -> Wave<'a> {
        self.elapsed = elapsed;
        self
    }
}

impl Widget for Wave<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let spans = wave_spans(
            self.text,
            self.color_a,
            self.color_b,
            self.base,
            self.period_ms,
            self.wavelength,
            self.elapsed,
        );
        buf.set_line(area.x, area.y, &Line::from_spans(spans));
    }
}

/// Shimmer: a highlight band sweeps across the text from left to right — the classic
/// loading placeholder effect.
#[derive(Debug, Clone)]
pub struct Shimmer<'a> {
    text: &'a str,
    base: Style,
    highlight: Style,
    band: usize,
    period_ms: u64,
    elapsed: Duration,
}

impl<'a> Shimmer<'a> {
    pub fn new(text: &'a str) -> Shimmer<'a> {
        Shimmer {
            text,
            base: Style::new().fg(Color::Rgb(95, 95, 95)),
            highlight: Style::new().fg(Color::Rgb(240, 240, 240)),
            band: 6,
            period_ms: 1800,
            elapsed: Duration::ZERO,
        }
    }

    pub fn styles(mut self, base: Style, highlight: Style) -> Shimmer<'a> {
        self.base = base;
        self.highlight = highlight;
        self
    }

    pub fn band(mut self, n: usize) -> Shimmer<'a> {
        self.band = n.max(1);
        self
    }

    pub fn period(mut self, ms: u64) -> Shimmer<'a> {
        self.period_ms = ms.max(1);
        self
    }

    pub fn at(mut self, elapsed: Duration) -> Shimmer<'a> {
        self.elapsed = elapsed;
        self
    }
}

impl Widget for Shimmer<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let total = str_width(self.text) as f32;
        let phase = (self.elapsed.as_millis() % self.period_ms.max(1) as u128) as f32
            / self.period_ms.max(1) as f32;
        let band = self.band as f32;
        let pos = phase * (total + band) - band; // sweeps in from beyond the left, out beyond the right
        let mut x = 0.0f32;
        for ch in self.text.chars() {
            let w = char_width(ch);
            if w == 0 {
                continue;
            }
            let mid = x + w as f32 * 0.5;
            let st = if mid >= pos && mid < pos + band {
                self.highlight
            } else {
                self.base
            };
            if (x as u16) < area.width {
                buf.set_cell(area.x + x as u16, area.y, ch, st);
            }
            x += w as f32;
        }
    }
}

/// Gradient progress bar: the fill gradients from `from` to `to`; supports determinate
/// and indeterminate modes.
#[derive(Debug, Clone)]
pub struct ProgressBar {
    progress: f32,
    width: usize,
    indeterminate: bool,
    from: Color,
    to: Color,
    track: Color,
    show_percent: bool,
    period_ms: u64,
    elapsed: Duration,
}

impl ProgressBar {
    pub fn new(progress: f32) -> ProgressBar {
        ProgressBar {
            progress: progress.clamp(0.0, 1.0),
            width: 24,
            indeterminate: false,
            from: Color::Rgb(215, 119, 87),
            to: Color::Rgb(120, 200, 255),
            track: Color::Indexed(238),
            show_percent: true,
            period_ms: 2200,
            elapsed: Duration::ZERO,
        }
    }

    pub fn width(mut self, n: usize) -> ProgressBar {
        self.width = n.max(1);
        self
    }

    pub fn indeterminate(mut self, on: bool) -> ProgressBar {
        self.indeterminate = on;
        self
    }

    pub fn colors(mut self, from: Color, to: Color) -> ProgressBar {
        self.from = from;
        self.to = to;
        self
    }

    pub fn track(mut self, c: Color) -> ProgressBar {
        self.track = c;
        self
    }

    pub fn show_percent(mut self, on: bool) -> ProgressBar {
        self.show_percent = on;
        self
    }

    pub fn period(mut self, ms: u64) -> ProgressBar {
        self.period_ms = ms.max(1);
        self
    }

    pub fn at(mut self, elapsed: Duration) -> ProgressBar {
        self.elapsed = elapsed;
        self
    }
}

impl Widget for ProgressBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let w = self.width.min(area.width as usize);
        let phase = (self.elapsed.as_millis() % self.period_ms.max(1) as u128) as f32
            / self.period_ms.max(1) as f32;

        let (filled_start, filled_end): (usize, usize) = if self.indeterminate {
            let band = (w / 4).max(1);
            let tri = if phase < 0.5 { phase * 2.0 } else { 2.0 - phase * 2.0 };
            let s = (tri * (w - band) as f32).round() as usize;
            (s, s + band)
        } else {
            let n = (w as f32 * self.progress).round() as usize;
            (0, n)
        };

        for i in 0..w {
            let (ch, color) = if i >= filled_start && i < filled_end {
                let denom = (filled_end - filled_start).max(1) as f32;
                let t = (i - filled_start) as f32 / denom;
                ('█', lerp_rgb(self.from, self.to, t))
            } else {
                ('░', self.track)
            };
            buf.set_cell(area.x + i as u16, area.y, ch, Style::new().fg(color));
        }

        if self.show_percent && !self.indeterminate && area.width as usize > w + 5 {
            let label = format!("{:>3.0}%", self.progress * 100.0);
            buf.set_string(area.x + w as u16 + 1, area.y, &label, Style::new().fg(self.track));
        }
    }
}

/// Loading dots: `Working` → `Working·` → `Working··` → `Working···` → repeat.
#[derive(Debug, Clone)]
pub struct LoadingDots<'a> {
    label: &'a str,
    style: Style,
    dots_style: Style,
    max_dots: usize,
    period_ms: u64,
    elapsed: Duration,
}

impl<'a> LoadingDots<'a> {
    pub fn new(label: &'a str) -> LoadingDots<'a> {
        LoadingDots {
            label,
            style: Style::new(),
            dots_style: Style::new(),
            max_dots: 3,
            period_ms: 400,
            elapsed: Duration::ZERO,
        }
    }

    pub fn styles(mut self, label: Style, dots: Style) -> LoadingDots<'a> {
        self.style = label;
        self.dots_style = dots;
        self
    }

    pub fn max_dots(mut self, n: usize) -> LoadingDots<'a> {
        self.max_dots = n.max(1);
        self
    }

    pub fn period(mut self, ms: u64) -> LoadingDots<'a> {
        self.period_ms = ms.max(1);
        self
    }

    pub fn at(mut self, elapsed: Duration) -> LoadingDots<'a> {
        self.elapsed = elapsed;
        self
    }
}

impl Widget for LoadingDots<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let cycle = self.max_dots + 1;
        let n = (self.elapsed.as_millis() / self.period_ms.max(1) as u128) as usize % cycle;
        buf.set_string(area.x, area.y, self.label, self.style);
        let dots = "·".repeat(n);
        buf.set_string(area.x + str_width(self.label) as u16, area.y, &dots, self.dots_style);
    }
}

/// Typewriter: progressively reveals text at a cps (chars per second) rate, simulating
/// streaming output.
#[derive(Debug, Clone)]
pub struct Typewriter<'a> {
    text: &'a str,
    style: Style,
    cps: f32,
    elapsed: Duration,
}

impl<'a> Typewriter<'a> {
    pub fn new(text: &'a str) -> Typewriter<'a> {
        Typewriter { text, style: Style::new(), cps: 24.0, elapsed: Duration::ZERO }
    }

    pub fn style(mut self, st: Style) -> Typewriter<'a> {
        self.style = st;
        self
    }

    pub fn cps(mut self, n: f32) -> Typewriter<'a> {
        self.cps = n.max(0.1);
        self
    }

    pub fn at(mut self, elapsed: Duration) -> Typewriter<'a> {
        self.elapsed = elapsed;
        self
    }

    /// Whether the full text has been revealed.
    pub fn done(&self) -> bool {
        self.visible_chars() >= self.text.chars().count()
    }

    fn visible_chars(&self) -> usize {
        (self.elapsed.as_secs_f32() * self.cps) as usize
    }
}

impl Widget for Typewriter<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let n = self.visible_chars();
        let shown: String = self.text.chars().take(n).collect();
        buf.set_string(area.x, area.y, &shown, self.style);
    }
}

/// Ticker: scrolls text horizontally within a window.
#[derive(Debug, Clone)]
pub struct Ticker<'a> {
    text: &'a str,
    style: Style,
    period_ms: u64,
    elapsed: Duration,
}

impl<'a> Ticker<'a> {
    pub fn new(text: &'a str) -> Ticker<'a> {
        Ticker { text, style: Style::new(), period_ms: 8000, elapsed: Duration::ZERO }
    }

    pub fn style(mut self, st: Style) -> Ticker<'a> {
        self.style = st;
        self
    }

    pub fn period(mut self, ms: u64) -> Ticker<'a> {
        self.period_ms = ms.max(1);
        self
    }

    pub fn at(mut self, elapsed: Duration) -> Ticker<'a> {
        self.elapsed = elapsed;
        self
    }
}

impl Widget for Ticker<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let win = area.width as usize;
        let total = str_width(self.text) + win;
        let phase = (self.elapsed.as_millis() % self.period_ms.max(1) as u128) as f32
            / self.period_ms.max(1) as f32;
        let skip = (phase * total as f32) as usize;

        let mut acc = 0usize;
        let mut out = String::new();
        let mut out_w = 0usize;
        for ch in self.text.chars() {
            let w = char_width(ch);
            if w == 0 {
                continue;
            }
            if acc >= skip && out_w + w <= win {
                out.push(ch);
                out_w += w;
            }
            acc += w;
            if out_w >= win {
                break;
            }
        }
        buf.set_string(area.x, area.y, &out, self.style);
    }
}
