//! sandbox: rendering capability showcase — borders, colors, styles, widths, wrapping,
//! animations.
//!
//! Run `cargo run --example sandbox`; q / Esc / Ctrl+C to quit.

use stilt::layout::{vsplit, Constraint, Rect};
use stilt::style::{Color, Modifier, Style};
use stilt::text::{Line, Text};
use stilt::widgets::anim::{LoadingDots, ProgressBar, Shimmer, Ticker, Wave};
use stilt::widgets::{Block, BorderType, Paragraph, Spinner, StatusBar, Widget};
use stilt::Terminal;
use std::time::Instant;

fn main() -> stilt::Result<()> {
    let mut term = Terminal::inline(37)?;
    let rx = stilt::app::spawn_input_thread();
    let start = Instant::now();
    let spinner = Spinner::new("rendering, press q to quit…").style(Style::new().fg(Color::Rgb(215, 119, 87)));

    loop {
        if let Ok(ev) = rx.try_recv() {
            match ev {
                stilt::Event::Key(k) if k.is_ctrl('c') => break,
                stilt::Event::Key(k) if k.code == stilt::event::KeyCode::Char('q') => break,
                stilt::Event::Key(k) if k.code == stilt::event::KeyCode::Esc => break,
                _ => {}
            }
        }

        let t = start.elapsed();
        term.draw(37, |frame| {
            let area = frame.area;
            let rows = vsplit(
                area,
                &[
                    Constraint::Length(7),  // borders
                    Constraint::Length(9),  // palette
                    Constraint::Length(6),  // styles
                    Constraint::Length(7),  // animations
                    Constraint::Fill(1),    // wrapping
                    Constraint::Length(1),  // status bar
                ],
            );

            // 1. Four border types
            let cols = stilt::layout::hsplit(
                rows[0],
                &[Constraint::Fill(1), Constraint::Fill(1), Constraint::Fill(1), Constraint::Fill(1)],
            );
            let names = [("Plain", BorderType::Plain), ("Rounded", BorderType::Rounded), ("Thick", BorderType::Thick), ("Double", BorderType::Double)];
            for (i, (name, bt)) in names.iter().enumerate() {
                Block::new()
                    .border_type(*bt)
                    .border_style(Style::new().fg(Color::Rgb(215, 119, 87)))
                    .title(Line::styled(format!(" {} ", name), Style::new().fg(Color::White).add_modifier(Modifier::BOLD)))
                    .render(cols[i], frame.buffer);
            }

            // 2. 16-color palette
            let swatch_y = rows[1].y;
            let swatches = [
                Color::Black, Color::Red, Color::Green, Color::Yellow, Color::Blue, Color::Magenta, Color::Cyan, Color::Gray,
                Color::DarkGray, Color::LightRed, Color::LightGreen, Color::LightYellow, Color::LightBlue, Color::LightMagenta, Color::LightCyan, Color::White,
            ];
            for (i, c) in swatches.iter().enumerate() {
                let x = rows[1].x + (i as u16 % 8) * 4;
                let y = swatch_y + (i as u16 / 8) * 2;
                frame.buffer.set_string(x, y, "████████", Style::new().fg(*c));
                frame.buffer.set_string(x, y + 1, &format!("{:?}", c), Style::new().fg(*c).add_modifier(Modifier::DIM));
            }
            // True color
            for (i, (r, g, bl)) in [(215u8, 119u8, 87u8), (80, 160, 255), (120, 220, 130)].iter().enumerate() {
                frame.buffer.set_string(rows[1].x + 32, swatch_y + i as u16 * 2, "████████", Style::new().fg(Color::Rgb(*r, *g, *bl)));
            }

            // 3. Styles and widths
            let mut styles = Text::default();
            styles.push_line(Line::styled("Bold / Italic / Underline / Strike", Style::new().add_modifier(Modifier::BOLD)));
            styles.push_line(Line::styled("中文与 ASCII 混排宽度：中文占 2 格 → │中文││AB│ 对齐检查", Style::new()));
            styles.push_line(Line::styled("😀 emoji width = 2, ✻ symbol width = 1: │😀││✻│", Style::new()));
            styles.push_line(Line::styled("e\u{0301} combining char width = 1 (é)", Style::new().add_modifier(Modifier::ITALIC)));
            Paragraph::new(styles).render(rows[2], frame.buffer);

            // 4. Animation showcase (all pure functions of time, driven by .at(elapsed))
            Wave::new("✻ stilt animation showcase — color wave text").at(t).render(rows[3], frame.buffer);
            Shimmer::new("shimmer: a highlight band sweeping across loading placeholder text")
                .at(t)
                .render(Rect::new(rows[3].x, rows[3].y + 1, rows[3].width, 1), frame.buffer);
            let demo_progress = (t.as_millis() % 4000) as f32 / 4000.0;
            ProgressBar::new(demo_progress)
                .width(30)
                .at(t)
                .render(Rect::new(rows[3].x, rows[3].y + 2, 36, 1), frame.buffer);
            LoadingDots::new("Thinking")
                .at(t)
                .render(Rect::new(rows[3].x, rows[3].y + 3, 20, 1), frame.buffer);
            Ticker::new("ticker: stilt — zero deps · diff rendering · inline mode · streaming markdown · CJK width alignment ✻")
                .at(t)
                .render(Rect::new(rows[3].x, rows[3].y + 4, rows[3].width, 1), frame.buffer);

            // 5. Automatic wrapping (wide chars + word level)
            let wrapped = "Word-wrap demo: The quick brown fox jumps over the lazy dog, plus a very long English sentence used to verify word-level wrapping and that wide characters are never split in half.";
            Paragraph::new(Text::raw(wrapped)).render(rows[4], frame.buffer);

            spinner.clone().render(Rect::new(area.x, area.bottom().saturating_sub(1), 30, 1), frame.buffer);
            StatusBar::hints(&[("q/Esc", "quit"), ("resize", "try dragging the window")]).render(
                Rect::new(area.x + 32, area.bottom().saturating_sub(1), area.width.saturating_sub(32), 1),
                frame.buffer,
            );
        })?;
    }
    Ok(())
}
