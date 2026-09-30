//! selection demo: try the editor's selection support.
//!
//! Run: `cargo run --example selection`
//!
//! - **Shift+←/→/↑/↓** (and Shift+Home/End): select — rendered reversed
//! - plain arrows: collapse the selection
//! - **type**: replace the selection
//! - **Backspace / Ctrl+W / Delete**: delete the selection
//! - **Ctrl+Z / Ctrl+Y**: undo / redo
//! - **Esc / Ctrl+C**: quit

use inline_tui::event::{Event, KeyCode};
use inline_tui::layout::{vsplit, Constraint};
use inline_tui::style::{Color, Modifier, Style};
use inline_tui::widgets::{Block, Editor, StatusBar, Widget};
use inline_tui::Terminal;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

fn main() -> inline_tui::Result<()> {
    let mut term = Terminal::inline(14)?;
    term.enable_mouse().enable_paste();
    let rx = inline_tui::app::spawn_input_thread();
    let accent = Style::new().fg(Color::Rgb(215, 119, 87)).add_modifier(Modifier::BOLD);

    let mut editor = Editor::new()
        .with_block(
            Block::rounded()
                .border_style(accent)
                .title(" ✻ selection demo "),
        )
        .placeholder_style(Style::new().add_modifier(Modifier::DIM))
        .selection_style(Style::new().fg(Color::Rgb(20, 20, 20)).bg(Color::Rgb(215, 119, 87)));
    editor.set_text(
        "The quick brown fox\njumps over the lazy dog.\nHold Shift + arrows to select me, then try Backspace or just type.",
    );

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Event::Key(k)) if k.is_ctrl('c') => break,
            Ok(Event::Key(k)) if k.code == KeyCode::Esc => break,
            Ok(Event::Key(k)) => {
                editor.handle_key(k);
            }
            Ok(Event::Paste(t)) => editor.handle_paste(&t),
            Ok(_) => {}
            Err(RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }

        term.draw(14, |frame| {
            let rows = vsplit(frame.area, &[Constraint::Fill(1), Constraint::Length(1)]);
            if let Some((cx, cy)) = editor.render_editor(rows[0], frame.buffer) {
                frame.set_cursor(cx, cy);
            }
            StatusBar::hints(&[
                ("shift+arrows", "select"),
                ("type", "replace"),
                ("bs/ctrl+w", "delete sel"),
                ("ctrl+z", "undo"),
                ("esc", "quit"),
            ])
            .render(rows[1], frame.buffer);
        })?;
    }
    Ok(())
}
