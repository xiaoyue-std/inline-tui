//! gallery: overview of cctui widgets (flagship example).
//!
//! Run `cargo run --example gallery` and switch pages with ←/→ or Tab:
//! - **List**   list + scrollbar (↑↓ to select)
//! - **Table**  table + row selection (↑↓ to select)
//! - **Form**   checkboxes + radio group (↑↓ to move, Space to toggle)
//! - **Charts** Sparkline / gradient progress bar / color wave / shimmer (animations)
//! - **Editor** multi-line input editor (type directly)
//!
//! q / Esc / Ctrl+C to quit.

use cctui::buffer::Buffer;
use cctui::event::{Event, KeyCode, KeyEvent};
use cctui::layout::{vsplit, Constraint, Rect};
use cctui::style::{Color, Modifier, Style};
use cctui::text::Line;
use cctui::theme::Theme;
use cctui::widgets::anim::{ProgressBar, Shimmer, Wave};
use cctui::widgets::sparkline::Sparkline;
use cctui::widgets::{
    Block, Checkbox, Editor, List, ListState, RadioGroup, Scrollbar, StatusBar, Table, TableState,
    Tabs, Widget,
};
use cctui::Terminal;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

const PAGES: [&str; 5] = ["List", "Table", "Form", "Charts", "Editor"];
const LIST_ITEMS: [&str; 9] = [
    "Rust", "C", "C++", "Python", "TypeScript", "Go", "Java", "Zig", "Haskell",
];

fn main() -> cctui::Result<()> {
    let mut term = Terminal::inline(24)?;
    term.enable_mouse().enable_paste();
    let rx = cctui::app::spawn_input_thread();
    let theme = Theme::default();
    let accent = Style::new().fg(Color::Rgb(215, 119, 87));
    let accent_bold = accent.add_modifier(Modifier::BOLD);
    let start = Instant::now();

    let mut page = 0usize;
    let mut list_state = ListState { offset: 0, selected: Some(0) };
    let mut table_state = TableState { offset: 0, selected: Some(0) };
    let mut checks = [true, false, false];
    let mut radio = 1usize;
    let mut editor = Editor::new()
        .with_placeholder("Type here… (type directly on this page; Enter submits and clears)")
        .placeholder_style(theme.placeholder)
        .text_style(theme.text);

    let table_rows: Vec<Vec<String>> = LIST_ITEMS
        .iter()
        .enumerate()
        .map(|(i, name)| {
            vec![
                name.to_string(),
                (1985 + i as u64 * 7).to_string(),
                format!("{}.{} MB", i + 1, i * 3 % 10),
            ]
        })
        .collect();

    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(Event::Key(k)) if k.is_ctrl('c') => break,
            Ok(Event::Key(k)) if k.code == KeyCode::Char('q') && k.modifiers.is_empty() => break,
            Ok(Event::Key(k)) if k.code == KeyCode::Esc => break,
            Ok(Event::Key(k)) => {
                handle_key(
                    k,
                    &mut page,
                    &mut list_state,
                    &mut table_state,
                    &mut checks,
                    &mut radio,
                    &mut editor,
                );
            }
            Ok(Event::Resize(..)) | Ok(_) => {}
            Err(RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }

        let t = start.elapsed();
        term.draw(24, |frame| {
            let area = frame.area;
            let rows = vsplit(
                area,
                &[Constraint::Length(3), Constraint::Fill(1), Constraint::Length(1)],
            );

            // Tab bar
            Tabs::new(PAGES)
                .selected(page)
                .styles(theme.dim, accent_bold)
                .render(rows[0], frame.buffer);

            // Page outer frame (draw the frame first, then the content)
            Block::rounded()
                .border_style(Style::new().fg(Color::Rgb(70, 70, 70)))
                .title(Line::styled(format!(" {} ", PAGES[page]), accent_bold))
                .render(rows[1], frame.buffer);
            let body = Rect {
                x: rows[1].x + 2,
                y: rows[1].y + 1,
                width: rows[1].width.saturating_sub(3),
                height: rows[1].height.saturating_sub(2),
            };

            match page {
                0 => render_list_page(frame.buffer, body, &list_state, accent),
                1 => render_table_page(frame.buffer, body, &table_rows, &table_state, &theme, accent),
                2 => render_form_page(frame.buffer, body, &checks, radio, accent),
                3 => render_charts_page(frame.buffer, body, t, accent),
                _ => {
                    if let Some((cx, cy)) = editor.render_editor(body, frame.buffer) {
                        frame.set_cursor(cx, cy);
                    }
                }
            }

            let hints: &[(&str, &str)] = match page {
                0 => &[("↑↓", "select"), ("←→", "page"), ("q", "quit")],
                1 => &[("↑↓", "select row"), ("←→", "page"), ("q", "quit")],
                2 => &[("↑↓", "move"), ("space", "toggle"), ("←→", "page")],
                3 => &[("←→", "page"), ("q", "quit")],
                _ => &[("just type", "edit"), ("←→", "page")],
            };
            let mut spans =
                StatusBar::hint_spans(hints, theme.status_hint_key, theme.status_hint, theme.dim);
            spans.insert(
                0,
                cctui::text::Span::styled(format!("{} / {}   ", page + 1, PAGES.len()), accent_bold),
            );
            StatusBar::new(spans).render(rows[2], frame.buffer);
        })?;
    }
    Ok(())
}

fn handle_key(
    k: KeyEvent,
    page: &mut usize,
    list_state: &mut ListState,
    table_state: &mut TableState,
    checks: &mut [bool; 3],
    radio: &mut usize,
    editor: &mut Editor,
) {
    match k.code {
        KeyCode::Left | KeyCode::BackTab => {
            *page = (*page + PAGES.len() - 1) % PAGES.len();
        }
        KeyCode::Right | KeyCode::Tab => {
            *page = (*page + 1) % PAGES.len();
        }
        KeyCode::Up => match page {
            0 => list_state.previous(),
            1 => table_state.previous(),
            _ => {}
        },
        KeyCode::Down => match page {
            0 => list_state.next(LIST_ITEMS.len()),
            1 => table_state.next(LIST_ITEMS.len()),
            _ => {}
        },
        KeyCode::Char(' ') if *page == 2 => {
            // Simplified: Space toggles the first unchecked checkbox, cycling back to all checked
            let idx = checks.iter().position(|c| !c).unwrap_or(0);
            checks[idx] = !checks[idx];
        }
        KeyCode::Enter if *page == 2 => {
            *radio = (*radio + 1) % 3;
        }
        other if *page == 4 => {
            editor.handle_key(KeyEvent::new(other, k.modifiers));
        }
        _ => {}
    }
}

fn render_list_page(buf: &mut Buffer, area: Rect, state: &ListState, accent: Style) {
    let items: Vec<Line> = LIST_ITEMS.iter().map(|s| Line::raw(s.to_string())).collect();
    let inner_w = area.width.saturating_sub(1); // right edge reserved for the scrollbar
    let list_area = Rect { width: inner_w, ..area };
    List::new(&items, state)
        .highlight_symbol("› ")
        .highlight_style(accent.add_modifier(Modifier::BOLD))
        .render(list_area, buf);
    Scrollbar::new()
        .style(Style::new().fg(Color::Rgb(90, 90, 90)))
        .render_with_metrics(list_area, buf, LIST_ITEMS.len(), area.height as usize, state.offset);
}

fn render_table_page(
    buf: &mut Buffer,
    area: Rect,
    rows: &[Vec<String>],
    state: &TableState,
    theme: &Theme,
    accent: Style,
) {
    Table::new(
        ["Language", "Year", "Size"],
        rows,
        vec![Constraint::Fill(1), Constraint::Length(10), Constraint::Length(12)],
        state,
    )
    .column_spacing(2)
    .styles(theme.text, theme.dim.add_modifier(Modifier::BOLD), accent)
    .highlight_symbol("› ")
    .render(area, buf);
}

fn render_form_page(buf: &mut Buffer, area: Rect, checks: &[bool; 3], radio: usize, accent: Style) {
    let labels = ["Autosave", "Syntax highlighting", "Mouse support"];
    for (i, label) in labels.iter().enumerate() {
        Checkbox::new(label, checks[i])
            .styles(Style::new().fg(Color::Rgb(160, 160, 160)), accent)
            .render(Rect::new(area.x, area.y + i as u16, area.width, 1), buf);
    }
    RadioGroup::new(["Light theme", "Dark theme", "Follow system"], radio)
        .styles(Style::new().fg(Color::Rgb(160, 160, 160)), accent)
        .render(Rect::new(area.x, area.y + 4, area.width, 3), buf);
    let hint = Line::styled(
        "(Space toggles checkboxes · Enter cycles radio)",
        Style::new().add_modifier(Modifier::DIM),
    );
    buf.set_line(area.x, area.y + 7, &hint);
}

fn render_charts_page(buf: &mut Buffer, area: Rect, t: Duration, accent: Style) {
    let half = area.width as usize / 2;
    let data: Vec<u64> = (0..half)
        .map(|i| {
            let phase = t.as_millis() as f32 / 3000.0 * std::f32::consts::TAU;
            let v = ((phase + i as f32 * 0.35).sin() * 0.5 + 0.5) * 100.0;
            v as u64
        })
        .collect();
    Sparkline::new(&data)
        .max(100)
        .style(accent)
        .render(Rect::new(area.x, area.y, area.width, 6), buf);

    let progress = (t.as_millis() % 4000) as f32 / 4000.0;
    ProgressBar::new(progress).width(30).at(t).render(Rect::new(area.x, area.y + 7, 36, 1), buf);

    Wave::new("✦ Animation widgets: Wave · Shimmer · ProgressBar · Sparkline")
        .at(t)
        .render(Rect::new(area.x, area.y + 9, area.width, 1), buf);
    Shimmer::new("shimmer loading placeholder effect — cctui anim")
        .at(t)
        .render(Rect::new(area.x, area.y + 11, area.width, 1), buf);
}
