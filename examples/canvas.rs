//! canvas demo: an animated braille sine wave with scatter points.
//!
//! Run: `cargo run --example canvas`
//!
//! The wave phase advances every frame (the animation tick), so the whole
//! grid is continuously redrawn — still flicker-free thanks to diffing.
//! **Esc / Ctrl+C** quit.

use inline_tui::layout::{vsplit, Constraint};
use inline_tui::style::{Color, Style};
use inline_tui::text::Line;
use inline_tui::widgets::{Block, Canvas, StatusBar, Widget};
use inline_tui::Terminal;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

fn main() -> inline_tui::Result<()> {
    let mut term = Terminal::inline(14)?;
    let rx = inline_tui::app::spawn_input_thread();
    let accent = Style::new().fg(Color::Rgb(215, 119, 87));
    let start = std::time::Instant::now();

    loop {
        match rx.recv_timeout(Duration::from_millis(60)) {
            Ok(event) => match event {
                inline_tui::Event::Key(k) if k.is_ctrl('c') => break,
                inline_tui::Event::Key(k) if k.code == inline_tui::event::KeyCode::Esc => break,
                _ => {}
            },
            Err(RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }

        let t = start.elapsed().as_secs_f64();
        term.draw(14, |frame| {
            let rows = vsplit(frame.area, &[Constraint::Fill(1), Constraint::Length(1)]);
            Block::rounded()
                .border_style(accent)
                .title(Line::from(" ✻ canvas — braille sine "))
                .render(rows[0], frame.buffer);
            let area = rows[0].inner(inline_tui::layout::Margin::new(1, 1));

            // Animated sine wave as connected line segments…
            let mut c = Canvas::new()
                .x_bounds([0.0, 10.0])
                .y_bounds([-1.2, 1.2])
                .style(accent);
            let n = 120;
            let mut prev: Option<(f64, f64)> = None;
            for i in 0..=n {
                let x = i as f64 / n as f64 * 10.0;
                let y = (x * 1.5 + t * 2.5).sin() * (1.0 - x / 15.0);
                if let Some((px, py)) = prev {
                    c = c.line(px, py, x, y);
                }
                prev = Some((x, y));
            }
            // …plus drifting scatter points.
            for k in 0..6u64 {
                let x = (t * 1.7 + k as f64 * 1.9) % 10.0;
                let y = ((t * 1.1 + k as f64).sin()) * 0.9;
                c = c.point(x, y);
            }
            c.render(area, frame.buffer);

            StatusBar::hints(&[("esc", "quit"), ("60fps", "animation tick")])
                .render(rows[1], frame.buffer);
        })?;
    }
    Ok(())
}
