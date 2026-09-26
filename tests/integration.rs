//! Integration tests: end-to-end simulation (pure logic, no real terminal needed).

use cctui::buffer::Buffer;
use cctui::event::{KeyCode, KeyModifiers, KeyEvent};
use cctui::layout::{vsplit, Constraint, Rect};
use cctui::markdown;
use cctui::text::Text;
use cctui::theme::Theme;
use cctui::widgets::{Editor, InputAction, Paragraph, Widget};
use cctui::width::str_width;

/// Renders into a Buffer and collects all line texts.
fn render_lines(w: u16, h: u16, f: impl FnOnce(Rect, &mut Buffer)) -> Vec<String> {
    let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
    f(Rect::new(0, 0, w, h), &mut buf);
    (0..h)
        .map(|y| {
            (0..w)
                .map(|x| buf.get(x, y).map(|c| c.symbol.clone()).unwrap_or_default())
                .collect::<String>()
        })
        .collect()
}

#[test]
fn markdown_fits_width() {
    let theme = Theme::default();
    let src = "# 标题\n\n一段**中文**与 English 混排的段落，需要自动换行处理，宽字符不能被切断。\n\n```rust\nfn main() { println!(\"hi\"); }\n```\n";
    let t = markdown::render(src, &theme, 40);
    for (i, line) in t.lines.iter().enumerate() {
        let w: usize = line.spans.iter().map(|s| s.width()).sum();
        assert!(
            w <= 40,
            "line {} too wide: {} ({} > 40)",
            i,
            line.spans.iter().map(|s| s.content.as_str()).collect::<String>(),
            w
        );
    }
}

#[test]
fn editor_full_flow() {
    // Input → completion → submit → history
    let mut e = Editor::new().with_completions(vec!["help".into(), "exit".into()]);
    for c in "/he".chars() {
        e.handle_key(KeyEvent::char(c));
    }
    assert!(e.menu_state().is_some());
    e.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(e.text(), "/help ");
    for c in "how to use".chars() {
        e.handle_key(KeyEvent::char(c));
    }
    let act = e.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(
        act,
        InputAction::Submitted("/help how to use".to_string())
    );
    assert!(e.is_empty());
    // History
    assert!(e.history_up_public());
    assert_eq!(e.text(), "/help how to use");
}

#[test]
fn editor_layout_splits() {
    let area = Rect::new(0, 0, 80, 24);
    let input_h = 5u16;
    let rows = vsplit(
        area,
        &[
            Constraint::Length(24 - input_h - 1),
            Constraint::Length(0),
            Constraint::Length(input_h),
            Constraint::Length(1),
        ],
    );
    assert_eq!(rows[2].height, input_h);
    assert_eq!(rows[3].y, 23);
    assert_eq!(rows[3].height, 1);
}

#[test]
fn paragraph_cjk_wrapping_roundtrip() {
    let lines = render_lines(12, 4, |area, buf| {
        Paragraph::new(Text::raw("中文自动换行测试：宽字符占两格且不会被从中间切断")).render(area, buf);
    });
    let joined: String = lines.join("\n");
    // Wrapping loses no content
    let content: String = joined.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(content.contains("中文自动换行"));
    assert!(content.contains("切断"));
    // No line exceeds the width
    for l in &lines {
        assert!(str_width(l) <= 12);
    }
}

// Provides a test entry point for Editor's history_up (wrapper around an internal action).
trait HistoryProbe {
    fn history_up_public(&mut self) -> bool;
}
impl HistoryProbe for Editor {
    fn history_up_public(&mut self) -> bool {
        // When single-line and non-empty, Up should load history
        use cctui::event::KeyEvent;
        matches!(
            self.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
            InputAction::Edited
        ) && !self.text().is_empty()
    }
}

/// Simulates the full frame pipeline (an equivalent of the chat example's draw closure):
/// history viewport + input box + status bar rendered under the same layout,
/// asserting the cursor lands inside the input box and no content overflows.
#[test]
fn full_frame_pipeline() {
    let theme = Theme::default();
    let width = 80u16;
    let mut editor = Editor::new()
        .with_completions(vec!["help".into(), "exit".into()])
        .with_placeholder("给 chat 发消息…");
    for c in "你好 cctui".chars() {
        editor.handle_key(KeyEvent::char(c));
    }

    let msgs_md = "# 标题\n\n一段 **Markdown** 正文，含 `code`。\n\n```rust\nlet x = 1;\n```\n- 列表项一\n- 列表项二";
    let user_md = "用户输入的一行中文";

    for frame in 0..3 {
        let mut lines: Vec<cctui::text::Line> = Vec::new();
        let user_mark = cctui::style::Style::new().fg(cctui::style::Color::Rgb(120, 180, 255));
        let t = markdown::render(msgs_md, &theme, width as usize);
        lines.extend(t.lines);
        for row in cctui::text::wrap_line(&cctui::text::Line::raw(user_md), (width - 2) as usize) {
            let mut l = cctui::text::Line::empty();
            l.push_span(cctui::text::Span::styled("› ".to_string(), user_mark));
            l.spans.extend(row.spans.clone());
            lines.push(l);
        }

        let mut buf = Buffer::empty(Rect::new(0, 0, width, 24));
        let rows = vsplit(
            Rect::new(0, 0, width, 24),
            &[
                Constraint::Length(24 - editor.desired_height() - 1),
                Constraint::Length(0),
                Constraint::Length(editor.desired_height()),
                Constraint::Length(1),
            ],
        );

        // History
        for (i, line) in lines.iter().rev().take(rows[0].height as usize).enumerate() {
            let y = rows[0].bottom() - 1 - i as u16;
            buf.set_line(0, y, &cctui::text::truncate_line(line, width as usize));
        }
        // Input box (alternating empty/non-empty between frames, simulating editing)
        if frame == 1 {
            editor.clear();
        }
        let cursor = editor.render_editor(rows[2], &mut buf);
        let inner = cctui::widgets::Block::rounded().inner(rows[2]);
        if let Some((cx, cy)) = cursor {
            assert!(cx >= inner.x && cx < inner.right(), "cursor column {} out of bounds", cx);
            assert!(cy >= inner.y && cy < inner.bottom(), "cursor row {} out of bounds", cy);
        }
        // Status bar
        cctui::widgets::StatusBar::new(vec![cctui::text::Span::raw("status")])
            .render(rows[3], &mut buf);

        // No overflow: the whole frame's content stays within the buffer width
        // (wide chars can never be cut in half; drawing stops at the boundary)
        for y in 0..24 {
            let mut x = 0;
            while x < width {
                let cell = buf.get(x, y).unwrap();
                assert!(cell.width <= 1 || x + 1 < width, "wide char split in half at the last column");
                x += cell.width as u16;
            }
        }
    }
}
