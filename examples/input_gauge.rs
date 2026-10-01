//! input + gauge demo: a single-line input whose length drives a gauge.
//!
//! Run: `cargo run --example input_gauge`
//!
//! - type into the single-line input (Enter submits, the input clears,
//!   the submitted text moves to the status line)
//! - the gauge mirrors the input length (fills at 24 chars)
//! - **Esc / Ctrl+C** quit

use inline_tui::event::{Event, KeyCode};
use inline_tui::layout::{vsplit, Constraint};
use inline_tui::style::{Color, Style};
use inline_tui::text::Span;
use inline_tui::widgets::{Block, Editor, Gauge, InputAction, StatusBar, Widget};
use inline_tui::Terminal;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

const FULL_AT: f32 = 24.0;

fn main() -> inline_tui::Result<()> {
    let mut term = Terminal::inline(10)?;
    term.enable_mouse().enable_paste();
    let rx = inline_tui::app::spawn_input_thread();
    let accent = Style::new().fg(Color::Rgb(215, 119, 87));

    let mut input = Editor::new()
        .single_line(true)
        .with_block(
            Block::rounded()
                .border_style(accent)
                .title(" ✻ single-line input "),
        )
        .with_placeholder("type — the gauge tracks your length…");
    let mut last: Option<String> = None;

    loop {
        match rx.recv_timeout(Duration::from_millis(80)) {
            Ok(Event::Key(k)) if k.is_ctrl('c') => break,
            Ok(Event::Key(k)) if k.code == KeyCode::Esc => break,
            Ok(Event::Key(k)) => {
                if let InputAction::Submitted(t) = input.handle_key(k) {
                    last = Some(t);
                }
            }
            Ok(Event::Paste(t)) => input.handle_paste(&t),
            Ok(_) => {}
            Err(RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }

        term.draw(10, |frame| {
            let rows = vsplit(
                frame.area,
                &[
                    Constraint::Length(3), // input
                    Constraint::Length(1), // gauge
                    Constraint::Length(1), // spacer
                    Constraint::Length(1), // status
                ],
            );
            if let Some((cx, cy)) = input.render_editor(rows[0], frame.buffer) {
                frame.set_cursor(cx, cy);
            }
            let percent = input.text().chars().count() as f32 / FULL_AT;
            Gauge::new(percent)
                .style(accent)
                .label(format!("{} / {}", input.text().chars().count(), FULL_AT as u32))
                .render(rows[1], frame.buffer);
            let mut spans = vec![
                Span::styled("✻", accent),
                Span::styled("  last: ", Style::new().fg(Color::Indexed(245))),
            ];
            match &last {
                Some(t) => spans.push(Span::raw(t.clone())),
                None => spans.push(Span::styled(
                    "(nothing submitted yet — Enter submits)",
                    Style::new().fg(Color::Indexed(245)),
                )),
            }
            StatusBar::new(spans).render(rows[3], frame.buffer);
        })?;
    }
    Ok(())
}
