//! quickstart: the smallest useful stilt app — a selectable, toggleable todo list.
//!
//! Run: `cargo run --example quickstart`  (↑↓ move · Space toggle · q/Esc/Ctrl+C quit)

use stilt::event::{Event, KeyCode};
use stilt::layout::{vsplit, Constraint};
use stilt::style::{Color, Modifier, Style};
use stilt::text::Line;
use stilt::widgets::{List, ListState, StatusBar, Widget};
use stilt::Terminal;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

fn main() -> stilt::Result<()> {
    // 1. Take over the terminal: raw mode + an inline region (bottom 10 rows,
    //    scrollback history above stays intact). The original terminal state is
    //    restored automatically on Drop — even when the app panics.
    let mut term = Terminal::inline(10)?;

    // 2. A background thread reads keys / mouse / paste and sends Events.
    //    recv_timeout on the receiver doubles as the animation tick.
    let rx = stilt::app::spawn_input_thread();

    // 3. Your application state lives OUTSIDE the widgets (ratatui-style):
    //    widgets borrow it for rendering, you own every mutation.
    let items = vec![
        "write the render core",
        "add the widget set",
        "publish to crates.io",
    ];
    let mut done: Vec<bool> = vec![true, true, false];
    let mut list = ListState { offset: 0, selected: Some(0) };

    loop {
        // 4. Event loop: wait for input, or tick on timeout.
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Event::Key(k)) if k.is_ctrl('c') => break,
            Ok(Event::Key(k)) if k.code == KeyCode::Esc => break,
            Ok(Event::Key(k)) => match k.code {
                KeyCode::Char('q') => break,
                KeyCode::Up => list.previous(),
                KeyCode::Down => list.next(items.len()),
                KeyCode::Char(' ') => {
                    if let Some(i) = list.selected {
                        done[i] = !done[i];
                    }
                }
                _ => {}
            },
            Ok(_ev) => {}                        // mouse / paste / resize
            Err(RecvTimeoutError::Timeout) => {} // tick: drive animations here
            Err(_) => break,
        }

        // 5. Render: paint every widget into the frame each iteration.
        //    stilt diffs this frame against the previous one — only the changed
        //    cells are written to the terminal, so streaming updates never flicker.
        let accent = Style::new().fg(Color::Rgb(215, 119, 87)).add_modifier(Modifier::BOLD);
        term.draw(10, |frame| {
            let rows = vsplit(frame.area, &[Constraint::Fill(1), Constraint::Length(1)]);

            let lines: Vec<Line> = items
                .iter()
                .enumerate()
                .map(|(i, it)| {
                    if done[i] {
                        Line::styled(format!("✓ {}", it), Style::new().fg(Color::Green))
                    } else {
                        Line::raw(it.to_string())
                    }
                })
                .collect();

            List::new(&lines, &list)
                .highlight_symbol("› ")
                .highlight_style(accent)
                .render(rows[0], frame.buffer);

            StatusBar::hints(&[("↑↓", "move"), ("space", "toggle"), ("q", "quit")])
                .render(rows[1], frame.buffer);
        })?;
    }
    Ok(()) // terminal is restored here (Drop)
}
