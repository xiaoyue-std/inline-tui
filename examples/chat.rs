//! chat: widget assembly demo — builds a messaging app from stilt's standard widgets.
//!
//! Run `cargo run --example chat`.
//!
//! Widget combinations demonstrated:
//! - [`Viewport`] (follow-bottom scrolling) + [`markdown`](stilt::markdown) rendered
//!   message stream
//! - [`Editor`] multi-line editor + slash command completion menu (opened with `/`)
//! - [`diff_lines`] colored rendering, status bar, animated spinner/loading dots
//!
//! Commands: `/help`, `/diff`, `/clear`, `/exit`; any message gets a simulated
//! streaming reply.
//! `Ctrl+C` clears the input; press again to quit.

use stilt::event::{Event, KeyCode, MouseEventKind};
use stilt::layout::{vsplit, Constraint};
use stilt::markdown;
use stilt::style::{Color, Modifier, Style};
use stilt::text::{wrap_line, Line, Span};
use stilt::theme::Theme;
use stilt::widgets::diff::diff_lines;
use stilt::widgets::spinner::STARS;
use stilt::widgets::{Editor, InputAction, Menu, ScrollState, StatusBar, Viewport, Widget};
use stilt::Terminal;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

enum Msg {
    User(String),
    Assistant(String),
    Diff(String),
}

enum StreamMsg {
    Chunk(String),
    Done,
}

const WELCOME: &str = "Welcome to **chat** — a demo app assembled from `stilt` widgets ✻\n\nTry these:\n- Type `/` to open the command menu (↑↓ to select, Enter or Tab to accept)\n- Say anything and you'll get a **streaming** Markdown reply\n- `/diff` to see diff coloring · `/help` for shortcuts · `Ctrl+C` to quit";

const REPLY_CODE: &str = "The core idea of `stilt`'s diff rendering:\n\n1. Each frame, widgets are drawn into a `Buffer` (character grid)\n2. Compare cell-by-cell with the previous frame and emit only the **changed** cells\n3. Consecutive runs with the same style are merged into one cursor move + one color set\n\n```rust\n// The key to no flicker: only write the changed cells\nfor (x, y, cell) in prev.diff(&next) {\n    move_cursor(x, y);\n    emit_sgr(cell.style);\n    print(cell.symbol);\n}\n```\n\nThe whole thing is only a handful of syscalls per frame, so streaming refreshes never flicker ✻";

const REPLY_INLINE: &str = "> The essence of terminal UIs: **restraint**.\n\nIn inline mode the UI occupies only a few lines at the bottom of the terminal, while the scrollback history above is preserved — the most fundamental difference from fullscreen TUIs like vim.\n\nImplementation notes: the region is anchored to the bottom (CUP absolute addressing), a height change triggers a full redraw, and wide characters occupy two cells according to the wcwidth table.";

const HELP: &str = "## Available commands\n\n- `/help` — show this help\n- `/diff` — display a unified diff rendering\n- `/clear` — clear messages\n- `/exit` — quit\n\n### Shortcuts\n\n- `Ctrl+C` — clear the input; press again to quit\n- `Alt+Enter` — insert a newline\n- `↑ / ↓` — browse submitted history (single-line state)\n- `PgUp / PgDn` or mouse wheel — scroll history\n- Type `/` — open the command menu";

const SAMPLE_DIFF: &str = "diff --git a/src/main.rs b/src/main.rs\nindex 3a4b2c1..d5e6f7a 100644\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,5 +1,7 @@\n fn main() {\n-    println!(\"Hello, world!\");\n+    let name = \"stilt\";\n+    // Light up the terminal with diff rendering\n+    println!(\"Hello, {}!\", name);\n }";

fn main() -> stilt::Result<()> {
    let mut term = Terminal::inline(24)?;
    term.enable_mouse().enable_paste();

    let rx = stilt::app::spawn_input_thread();
    let (stx, srx) = mpsc::channel::<StreamMsg>();

    let theme = Theme::default();
    let commands = ["help", "diff", "clear", "exit"];
    let accent = Style::new().fg(Color::Rgb(215, 119, 87)).add_modifier(Modifier::BOLD);
    let user_mark = Style::new().fg(Color::Rgb(120, 180, 255)).add_modifier(Modifier::BOLD);
    let mut editor = Editor::new()
        .with_completions(commands.iter().map(|s| s.to_string()).collect())
        .with_placeholder("Type a message… ('/' commands · Alt+Enter newline · Ctrl+C quit)")
        .with_block(
            stilt::widgets::Block::rounded()
                .border_style(theme.border_focused)
                .title(Line::styled(" ✻ chat ", accent)),
        )
        .placeholder_style(theme.placeholder)
        .text_style(theme.text)
        .max_grow_lines(5);

    let mut msgs: Vec<Msg> = vec![Msg::Assistant(WELCOME.to_string())];
    let mut scroll = ScrollState { offset: 0, follow: true };
    let mut streaming = false;
    let mut reply_idx = 0usize;
    let mut exit = false;
    let start = std::time::Instant::now();
    let mut last_content_h = 0usize;
    let mut last_hist_h = 1usize;

    while !exit {
        match rx.recv_timeout(Duration::from_millis(80)) {
            Ok(Event::Key(k)) if k.is_ctrl('c') => {
                if editor.is_empty() {
                    break;
                }
                editor.clear();
            }
            Ok(Event::Key(k)) => {
                // History scroll keys don't reach the editor
                match k.code {
                    KeyCode::PageUp => {
                        scroll.scroll_up(10);
                        continue;
                    }
                    KeyCode::PageDown => {
                        scroll.scroll_down(10, last_content_h, last_hist_h);
                        continue;
                    }
                    _ => {}
                }
                let submitted = if streaming && k.code == KeyCode::Enter && k.modifiers.is_empty() {
                    InputAction::None // submissions are not accepted while a reply is streaming
                } else {
                    editor.handle_key(k)
                };
                if let InputAction::Submitted(text) = submitted {
                    msgs.push(Msg::User(text.clone()));
                    if let Some(cmd) = text.strip_prefix('/').map(str::trim) {
                        match cmd {
                            "clear" => msgs.clear(),
                            "exit" => exit = true,
                            "help" => msgs.push(Msg::Assistant(HELP.to_string())),
                            "diff" => {
                                msgs.push(Msg::Assistant("Here is `stilt`'s diff rendering:".to_string()));
                                msgs.push(Msg::Diff(SAMPLE_DIFF.to_string()));
                            }
                            other => {
                                msgs.push(Msg::Assistant(format!("Unknown command `/{}`, try `/help`.", other)));
                            }
                        }
                    } else {
                        // Simulate a streaming reply (demonstrates streaming content driving Text/Viewport)
                        let reply = if reply_idx % 2 == 0 { REPLY_CODE } else { REPLY_INLINE };
                        reply_idx += 1;
                        streaming = true;
                        msgs.push(Msg::Assistant(String::new()));
                        let stx = stx.clone();
                        let reply = reply.to_string();
                        std::thread::spawn(move || {
                            std::thread::sleep(Duration::from_millis(500));
                            let chars: Vec<char> = reply.chars().collect();
                            let mut i = 0;
                            while i < chars.len() {
                                let n = 2 + (i as u64 * 7919 % 6) as usize;
                                let end = (i + n).min(chars.len());
                                if stx.send(StreamMsg::Chunk(chars[i..end].iter().collect())).is_err() {
                                    return;
                                }
                                std::thread::sleep(Duration::from_millis(45));
                                i = end;
                            }
                            let _ = stx.send(StreamMsg::Done);
                        });
                    }
                }
            }
            Ok(Event::Paste(t)) => editor.handle_paste(&t),
            Ok(Event::Mouse(m)) => {
                match m.kind {
                    MouseEventKind::ScrollUp => scroll.scroll_up(3),
                    MouseEventKind::ScrollDown => scroll.scroll_down(3, last_content_h, last_hist_h),
                    _ => {}
                }
            }
            Ok(Event::Resize(..)) => {}
            Ok(_) => {}
            Err(RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }

        // Consume streaming chunks
        while let Ok(sm) = srx.try_recv() {
            match sm {
                StreamMsg::Chunk(s) => {
                    if let Some(Msg::Assistant(md)) = msgs.last_mut() {
                        md.push_str(&s);
                    }
                }
                StreamMsg::Done => streaming = false,
            }
        }

        term.draw(24, |frame| {
            let t = start.elapsed();
            let area = frame.area;
            let input_h = editor.desired_height();
            let menu_h = editor.menu_height();
            let status_h = 1u16;
            let hist_h = area.height.saturating_sub(input_h + menu_h + status_h).max(1);
            let rows = vsplit(
                area,
                &[
                    Constraint::Length(hist_h),
                    Constraint::Length(menu_h),
                    Constraint::Length(input_h),
                    Constraint::Length(status_h),
                ],
            );

            // Message history
            let lines = history_lines(&msgs, &theme, user_mark, rows[0].width as usize);
            last_content_h = lines.len();
            last_hist_h = rows[0].height as usize;
            scroll.sync(lines.len(), rows[0].height as usize);
            Viewport::new(&lines, scroll.offset).render(rows[0], frame.buffer);

            // Slash command menu
            if let Some((items, sel)) = editor.menu_state() {
                Menu::new(items)
                    .selected(sel)
                    .normal_style(theme.dim)
                    .selected_style(accent)
                    .render(rows[1], frame.buffer);
            }

            // Input box + cursor
            if let Some((cx, cy)) = editor.render_editor(rows[2], frame.buffer) {
                frame.set_cursor(cx, cy);
            }

            // Status bar
            let key_st = theme.status_hint_key;
            let label_st = theme.status_hint;
            let sep_st = theme.dim;
            let mut spans = vec![
                Span::styled("✻".to_string(), accent),
                Span::styled(" chat   ".to_string(), label_st),
            ];
            spans.extend(StatusBar::hint_spans(
                &[("/ commands", "menu"), ("pgup/dn", "history"), ("ctrl+c", "quit")],
                key_st,
                label_st,
                sep_st,
            ));
            if streaming {
                let frame_ch = STARS[(t.as_millis() / 100) as usize % STARS.len()];
                let dots = (t.as_millis() / 400) % 4;
                spans.push(Span::styled(
                    format!("   {} replying{}", frame_ch, "·".repeat(dots as usize)),
                    theme.spinner,
                ));
            }
            StatusBar::new(spans).render(rows[3], frame.buffer);
        })?;
    }
    Ok(())
}

/// Renders the message list into lines for the history viewport (app-level assembly:
/// the user prefix style is defined here).
fn history_lines(msgs: &[Msg], theme: &Theme, user_mark: Style, width: usize) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    for msg in msgs {
        match msg {
            Msg::User(text) => {
                let body = wrap_line(&Line::raw(text.clone()), width.saturating_sub(2));
                for (i, row) in body.iter().enumerate() {
                    let mut l = Line::empty();
                    if i == 0 {
                        l.push_span(Span::styled("› ".to_string(), user_mark));
                    } else {
                        l.spans.push(Span::raw("  ".to_string()));
                    }
                    l.spans.extend(row.spans.clone());
                    lines.push(l);
                }
                lines.push(Line::empty());
            }
            Msg::Assistant(md) => {
                let t = markdown::render(md, theme, width);
                lines.extend(t.lines);
                lines.push(Line::empty());
            }
            Msg::Diff(raw) => {
                lines.extend(diff_lines(raw, theme.diff_add, theme.diff_del, theme.diff_hunk, theme.diff_meta));
                lines.push(Line::empty());
            }
        }
    }
    if lines.is_empty() {
        lines.push(Line::empty());
    }
    lines
}
