//! theme demo: custom accent colors, switched live.
//!
//! Run: `cargo run --example theme`
//!
//! Press **1-5** to switch the accent color — every accent-bearing role
//! (borders, spinner, bullets, inline code, selection, wave, progress bar)
//! re-derives from it on the next frame via `Theme::with_accent`.
//! **Esc / Ctrl+C** quit; the editor is fully playable (Shift+arrows select).

use inline_tui::event::{Event, KeyCode};
use inline_tui::layout::{vsplit, Constraint, Rect};
use inline_tui::markdown;
use inline_tui::style::{Color, Modifier, Style};
use inline_tui::text::{Line, Span};
use inline_tui::theme::Theme;
use inline_tui::widgets::anim::{lerp_rgb, ProgressBar, Wave};
use inline_tui::widgets::{Block, Editor, Paragraph, StatusBar, Widget};
use inline_tui::Terminal;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

const THEMES: [(&str, Color); 5] = [
    ("1 default", Color::Rgb(215, 119, 87)),
    ("2 ocean", Color::Rgb(80, 160, 255)),
    ("3 forest", Color::Rgb(100, 200, 130)),
    ("4 sunset", Color::Rgb(255, 140, 105)),
    ("5 rose", Color::Rgb(235, 130, 180)),
];

const SAMPLE: &str = "## Live recolor\n\nPress `1-5` and every **accent** role re-derives:\n\n- spinner, bullets, borders\n- inline code, quote bars, selection\n\n> one accent color in — a whole theme out";

fn main() -> inline_tui::Result<()> {
    let mut term = Terminal::inline(22)?;
    term.enable_mouse().enable_paste();
    let rx = inline_tui::app::spawn_input_thread();
    let start = Instant::now();

    let mut theme_idx = 0usize;
    let mut editor = Editor::new()
        .with_placeholder("type here — selection follows the theme…")
        .placeholder_style(Style::new().add_modifier(Modifier::DIM));
    editor.set_text("Hold Shift + arrows to select this line,\nthen press 1-5 to recolor everything.");

    loop {
        match rx.recv_timeout(Duration::from_millis(80)) {
            Ok(Event::Key(k)) if k.is_ctrl('c') => break,
            Ok(Event::Key(k)) if k.code == KeyCode::Esc => break,
            Ok(Event::Key(k)) => match k.code {
                KeyCode::Char(c @ '1'..='5') => theme_idx = (c as u8 - b'1') as usize,
                _ => {
                    editor.handle_key(k);
                }
            },
            Ok(Event::Paste(t)) => editor.handle_paste(&t),
            Ok(_) => {}
            Err(RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }

        let t = start.elapsed();
        let (name, accent) = THEMES[theme_idx];
        let theme = Theme::default().with_accent(accent);
        editor.set_selection_style(Style::new().fg(Color::Rgb(15, 15, 15)).bg(accent));

        term.draw(22, |frame| {
            let area = frame.area;
            let rows = vsplit(
                area,
                &[
                    Constraint::Length(3), // theme header + wave
                    Constraint::Fill(1),   // markdown sample
                    Constraint::Length(5), // editor
                    Constraint::Length(1), // progress bar
                    Constraint::Length(1), // status bar
                ],
            );

            let mut title = Line::from_spans(vec![
                Span::raw(" ✻ theme: ".to_string()),
                Span::styled(name.to_string(), theme.accent.add_modifier(Modifier::BOLD)),
                Span::styled(format!(" — accent {accent:?} "), theme.accent),
            ]);
            title.style = theme.border_focused;
            Block::rounded()
                .border_style(theme.border_focused)
                .title(title)
                .render(rows[0], frame.buffer);
            Wave::new("✻ with_accent(accent) → whole theme")
                .colors(accent, lerp_rgb(accent, Color::White, 0.6))
                .at(t)
                .render(
                    Rect::new(
                        rows[0].x + 2,
                        rows[0].y + 1,
                        rows[0].width.saturating_sub(4),
                        1,
                    ),
                    frame.buffer,
                );

            Paragraph::new(markdown::render(SAMPLE, &theme, rows[1].width as usize))
                .render(rows[1], frame.buffer);

            if let Some((cx, cy)) = editor.render_editor(rows[2], frame.buffer) {
                frame.set_cursor(cx, cy);
            }

            let progress = (t.as_millis() % 4000) as f32 / 4000.0;
            ProgressBar::new(progress)
                .width(24)
                .colors(accent, lerp_rgb(accent, Color::White, 0.7))
                .track(Color::Indexed(238))
                .show_percent(false)
                .at(t)
                .render(Rect::new(rows[3].x, rows[3].y, 26, 1), frame.buffer);

            let hints: &[(&str, &str)] =
                &[("1-5", "theme"), ("shift+arrows", "select"), ("esc", "quit")];
            let mut spans =
                StatusBar::hint_spans(hints, theme.status_hint_key, theme.status_hint, theme.dim);
            spans.insert(0, Span::styled(format!("✻ {name}   "), theme.accent));
            StatusBar::new(spans).render(rows[4], frame.buffer);
        })?;
    }
    Ok(())
}
