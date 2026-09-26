//! keyspector: key/mouse/paste event inspector.
//!
//! Run `cargo run --example keyspector` and press any key to see the parsed events;
//! q / Esc / Ctrl+C to quit.

use stilt::event::{Event, KeyCode, KeyModifiers};
use stilt::layout;
use stilt::style::{Color, Style};
use stilt::text::Line;
use stilt::widgets::{Block, StatusBar, Viewport, Widget};
use stilt::Terminal;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

fn main() -> stilt::Result<()> {
    let mut term = Terminal::inline(16)?;
    term.enable_mouse().enable_paste();

    let rx = stilt::app::spawn_input_thread();
    let mut log: Vec<String> = Vec::new();

    loop {
        let ev = match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(ev) => Some(ev),
            Err(RecvTimeoutError::Timeout) => None,
            Err(_) => break,
        };
        if let Some(ev) = &ev {
            match ev {
                Event::Key(k) if k.is_ctrl('c') => break,
                Event::Key(k) if k.code == KeyCode::Char('q') && k.modifiers.is_empty() => break,
                Event::Key(k) if k.code == KeyCode::Esc => break,
                _ => {}
            }
            log.push(describe(ev));
            if log.len() > 11 {
                log.remove(0);
            }
        }

        term.draw(16, |frame| {
            let area = frame.area;
            let rows = layout::vsplit(
                area,
                &[layout::Constraint::Length(3), layout::Constraint::Fill(1), layout::Constraint::Length(1)],
            );
            Block::rounded()
                .border_style(Style::new().fg(Color::Rgb(215, 119, 87)))
                .title(Line::raw(" ✻ keyspector — press any key (q/Esc/Ctrl+C to quit)"))
                .render(rows[0], frame.buffer);

            let mut lines: Vec<Line> = Vec::new();
            for l in &log {
                lines.push(Line::raw(l.clone()));
            }
            let offset = lines.len().saturating_sub(rows[1].height as usize);
            Viewport::new(&lines, offset).render(rows[1], frame.buffer);

            StatusBar::hints(&[("q/Esc", "quit"), ("wheel", "scroll events"), ("paste", "any text")])
                .render(rows[2], frame.buffer);
        })?;
    }
    Ok(())
}

fn describe(ev: &Event) -> String {
    match ev {
        Event::Key(k) => {
            let mut s = String::from("Key ");
            if k.modifiers.contains(KeyModifiers::CONTROL) {
                s.push_str("Ctrl+");
            }
            if k.modifiers.contains(KeyModifiers::ALT) {
                s.push_str("Alt+");
            }
            if k.modifiers.contains(KeyModifiers::SHIFT) {
                s.push_str("Shift+");
            }
            s.push_str(&format!("{:?}", k.code));
            s
        }
        Event::Mouse(m) => format!("Mouse {:?} @ ({}, {})", m.kind, m.column, m.row),
        Event::Paste(t) => format!("Paste {:?} ({} chars)", t, t.chars().count()),
        Event::Resize(w, h) => format!("Resize {}x{}", w, h),
        Event::FocusGained => "FocusGained".into(),
        Event::FocusLost => "FocusLost".into(),
        Event::CursorPosition(c, r) => format!("CursorPosition ({}, {})", c, r),
    }
}
