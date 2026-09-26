//! Terminal session management: inline / fullscreen modes and the diff render loop.

use crate::ansi;
use crate::buffer::Buffer;
use crate::layout::Rect;
use crate::style::Style;
use crate::sys;
use crate::widgets::Widget;
use crate::{Error, Result};
use std::io::Write;

/// Per-frame rendering context: draw content into `buffer`, optionally set the cursor position.
pub struct Frame<'a> {
    /// This frame's render area (starts at (0,0), width = terminal width, height = frame height).
    pub area: Rect,
    pub buffer: &'a mut Buffer,
    cursor: Option<(u16, u16)>,
}

impl<'a> Frame<'a> {
    /// Renders a widget into the given area.
    pub fn render<W: Widget>(&mut self, widget: W, area: Rect) {
        widget.render(area, self.buffer);
    }

    /// Sets the visible cursor position after rendering (area-relative coordinates, 0-based).
    pub fn set_cursor(&mut self, x: u16, y: u16) {
        self.cursor = Some((x, y));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Inline,
    Fullscreen,
}

/// Terminal session. Restores the terminal state automatically on Drop.
pub struct Terminal {
    mode: Mode,
    state: sys::TerminalState,
    size: (u16, u16),
    prev: Buffer,
    cur: Buffer,
    prev_height: u16,
    /// Number of reserved bottom scroll rows (inline mode).
    reserved: u16,
    first: bool,
    need_full: bool,
    mouse: bool,
    paste: bool,
    closed: bool,
}

impl Terminal {
    /// Inline mode: the UI renders within `initial_height` rows at the bottom of the
    /// terminal, preserving the scrollback history above.
    pub fn inline(initial_height: u16) -> Result<Terminal> {
        install_panic_hook();
        let state = sys::enable_raw()?;
        let size = sys::size().ok_or_else(|| {
            Error::TerminalUnavailable("cannot query terminal size (output redirected?)".into())
        })?;
        let initial = initial_height.max(1).min(size.1);
        // Fill a full screen of newlines so the cursor lands on the last screen row
        // (the region stays anchored to the bottom)
        let mut out = String::new();
        for _ in 0..size.1 {
            out.push_str("\r\n");
        }
        write_out(&out);
        Ok(Terminal {
            mode: Mode::Inline,
            state,
            size,
            prev: Buffer::empty(Rect::new(0, 0, 0, 0)),
            cur: Buffer::empty(Rect::new(0, 0, 0, 0)),
            prev_height: 0,
            reserved: initial,
            first: true,
            need_full: false,
            mouse: false,
            paste: false,
            closed: false,
        })
    }

    /// Fullscreen mode: enters the alternate screen and fills the whole terminal.
    pub fn fullscreen() -> Result<Terminal> {
        install_panic_hook();
        let state = sys::enable_raw()?;
        let size = sys::size().ok_or_else(|| {
            Error::TerminalUnavailable("cannot query terminal size (output redirected?)".into())
        })?;
        let mut out = String::new();
        ansi::enter_alt_screen(&mut out);
        ansi::cursor_hide(&mut out);
        write_out(&out);
        Ok(Terminal {
            mode: Mode::Fullscreen,
            state,
            size,
            prev: Buffer::empty(Rect::new(0, 0, 0, 0)),
            cur: Buffer::empty(Rect::new(0, 0, 0, 0)),
            prev_height: 0,
            reserved: 0,
            first: true,
            need_full: false,
            mouse: false,
            paste: false,
            closed: false,
        })
    }

    pub fn enable_mouse(&mut self) -> &mut Terminal {
        self.mouse = true;
        let mut out = String::new();
        ansi::enable_mouse(&mut out);
        write_out(&out);
        self
    }

    pub fn enable_paste(&mut self) -> &mut Terminal {
        self.paste = true;
        let mut out = String::new();
        ansi::enable_paste(&mut out);
        write_out(&out);
        self
    }

    /// Last queried terminal size (columns, rows).
    pub fn size(&self) -> (u16, u16) {
        self.size
    }

    /// Renders one frame.
    ///
    /// `height` is the region height the app wants (clamped to the screen in inline
    /// mode); the `render` closure draws widgets into the `Frame`. Drawing is
    /// diff-based and incremental.
    pub fn draw(&mut self, height: u16, render: impl FnOnce(&mut Frame)) -> Result<()> {
        if let Some(s) = sys::size() {
            if s != self.size {
                self.size = s;
                self.need_full = true;
            }
        }
        let (cols, rows) = self.size;
        if cols == 0 || rows == 0 {
            return Ok(());
        }
        let height = match self.mode {
            Mode::Fullscreen => rows,
            Mode::Inline => height.clamp(1, rows.saturating_sub(1).max(1)),
        };
        let area = Rect::new(0, 0, cols, height);
        let mut out = String::new();

        // inline: handle region height changes (anchored to the bottom)
        if self.mode == Mode::Inline && !self.first && height != self.prev_height {
            self.reserved = region_height_change(&mut out, self.prev_height, height, rows, self.reserved);
            self.need_full = true;
        }

        if self.prev.area != area {
            self.prev = Buffer::empty(area);
            self.need_full = true;
        }

        self.cur = Buffer::empty(area);
        let mut frame = Frame { area, buffer: &mut self.cur, cursor: None };
        render(&mut frame);
        let cursor = frame.cursor;

        // Diff output
        let full = self.first || self.need_full;
        let base_row = match self.mode {
            Mode::Fullscreen => 0,
            Mode::Inline => rows - height,
        };
        compile_frame(&mut out, &self.prev, &self.cur, base_row, full);

        match cursor {
            Some((cx, cy)) => {
                ansi::move_to(&mut out, cx + 1, base_row + cy + 1);
                ansi::cursor_show(&mut out);
            }
            None => ansi::cursor_hide(&mut out),
        }
        write_out(&out);

        std::mem::swap(&mut self.cur, &mut self.prev);
        self.first = false;
        self.need_full = false;
        self.prev_height = height;
        Ok(())
    }

    /// Teardown: restores the terminal state (also called automatically on Drop).
    pub fn close(&mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        let mut out = String::new();
        ansi::sgr_reset(&mut out);
        ansi::cursor_show(&mut out);
        if self.mouse {
            ansi::disable_mouse(&mut out);
        }
        if self.paste {
            ansi::disable_paste(&mut out);
        }
        match self.mode {
            Mode::Fullscreen => ansi::leave_alt_screen(&mut out),
            Mode::Inline => {
                let (_, rows) = self.size;
                ansi::move_to(&mut out, 1, rows);
                out.push_str("\r\n");
            }
        }
        write_out(&out);
        self.state.restore();
        Ok(())
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

/// Inline region height change (anchored to the bottom):
/// - shrink: clear the old rows exposed at the top
/// - beyond the reserved space: scroll in new scrollback lines
///
/// Returns the new reserved row count.
fn region_height_change(out: &mut String, prev_height: u16, height: u16, rows: u16, reserved: u16) -> u16 {
    if height < prev_height {
        let prev_top = rows.saturating_sub(prev_height);
        let new_top = rows.saturating_sub(height);
        for r in prev_top..new_top {
            ansi::move_to(out, 1, r + 1);
            ansi::clear_line(out);
        }
    } else if height > reserved {
        ansi::move_to(out, 1, rows);
        for _ in 0..(height - reserved) {
            out.push_str("\r\n");
        }
        return height;
    }
    reserved
}

/// Compiles the difference between the current and previous frames into minimal ANSI
/// output (the key path for flicker-free rendering).
fn compile_frame(out: &mut String, prev: &Buffer, cur: &Buffer, base_row: u16, full: bool) {
    let (cols, height) = (cur.area.width, cur.area.height);
    let mut last_style: Option<Style> = None;
    let mut last_pos: Option<(u16, u16)> = None;
    for y in 0..height {
        for x in 0..cols {
            let cell = &cur.content[(y * cols + x) as usize];
            if cell.symbol.is_empty() {
                continue; // wide-char placeholder cell
            }
            if !full && prev.get(x, y) == Some(cell) {
                continue;
            }
            let srow = base_row + y;
            if last_pos != Some((x, srow)) {
                ansi::move_to(out, x + 1, srow + 1);
            }
            ansi::push_style(
                out,
                &mut last_style,
                Style::new().fg(cell.fg).bg(cell.bg).add_modifier(cell.modifier),
            );
            out.push_str(&cell.symbol);
            last_pos = Some((x + cell.width as u16, srow));
        }
    }
}


fn write_out(s: &str) {
    let mut o = std::io::stdout().lock();
    let _ = o.write_all(s.as_bytes());
    let _ = o.flush();
}

/// Wraps the existing panic hook: restores the terminal before printing on panic.
fn install_panic_hook() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let mut out = String::new();
            ansi::sgr_reset(&mut out);
            ansi::cursor_show(&mut out);
            ansi::disable_mouse(&mut out);
            ansi::disable_paste(&mut out);
            ansi::leave_alt_screen(&mut out);
            out.push_str("\r\n");
            let _ = std::io::stdout().lock().write_all(out.as_bytes());
            sys::panic_restore();
            prev(info);
        }));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Color;

    #[test]
    fn region_shrink_clears_exposed_rows() {
        let mut out = String::new();
        let reserved = region_height_change(&mut out, 10, 8, 30, 10);
        assert_eq!(reserved, 10);
        // 0-based rows 20, 21 were exposed → 1-based positioning at 21, 22 and clear
        assert_eq!(out, "\x1b[21;1H\x1b[2K\x1b[22;1H\x1b[2K");
    }

    #[test]
    fn region_growth_scrolls_when_exceeding_reserved() {
        let mut out = String::new();
        let reserved = region_height_change(&mut out, 8, 12, 30, 10);
        assert_eq!(reserved, 12);
        assert_eq!(out, "\x1b[30;1H\r\n\r\n");
    }

    #[test]
    fn region_growth_within_reserved_is_free() {
        let mut out = String::new();
        let reserved = region_height_change(&mut out, 8, 10, 30, 10);
        assert_eq!(reserved, 10);
        assert!(out.is_empty());
    }

    #[test]
    fn compile_frame_emits_only_changes_with_bottom_anchor() {
        let area = Rect::new(0, 0, 4, 2);
        let prev = Buffer::empty(area);
        let mut cur = Buffer::empty(area);
        cur.set_string(0, 0, "ab", Style::new().fg(Color::Red));
        cur.set_string(3, 1, "c", Style::new());
        let mut out = String::new();
        compile_frame(&mut out, &prev, &cur, 10, false);
        // base_row=10 → region rows 0/1 map to screen rows 11/12 (1-based)
        assert!(out.contains("\x1b[11;1H"));
        assert!(out.contains("\x1b[12;4H"));
        assert!(out.contains("\x1b[0m\x1b[31mab"));
        assert!(out.ends_with("c"));
    }

    #[test]
    fn compile_frame_skips_wide_placeholders() {
        let area = Rect::new(0, 0, 4, 1);
        let prev = Buffer::empty(area);
        let mut cur = Buffer::empty(area);
        cur.set_string(0, 0, "中", Style::new());
        let mut out = String::new();
        compile_frame(&mut out, &prev, &cur, 0, false);
        assert!(out.ends_with("中"));
        assert_eq!(out.matches('H').count(), 1); // only one cursor positioning
    }
}

