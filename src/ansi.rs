//! ANSI/VT escape sequence generation primitives.

use crate::style::{Color, Modifier, Style};

/// Cursor positioning (1-based).
pub fn move_to(out: &mut String, x: u16, y: u16) {
    out.push_str("\x1b[");
    out.push_str(&y.to_string());
    out.push(';');
    out.push_str(&x.to_string());
    out.push('H');
}

pub fn cursor_show(out: &mut String) {
    out.push_str("\x1b[?25h");
}

pub fn cursor_hide(out: &mut String) {
    out.push_str("\x1b[?25l");
}

/// Enter/leave the alternate screen (fullscreen mode).
pub fn enter_alt_screen(out: &mut String) {
    out.push_str("\x1b[?1049h");
}

pub fn leave_alt_screen(out: &mut String) {
    out.push_str("\x1b[?1049l");
}

/// Mouse capture: normal + button-event tracking + SGR extended coordinates.
pub fn enable_mouse(out: &mut String) {
    out.push_str("\x1b[?1000h\x1b[?1002h\x1b[?1006h");
}

pub fn disable_mouse(out: &mut String) {
    out.push_str("\x1b[?1006l\x1b[?1002l\x1b[?1000l");
}

/// Bracketed paste mode.
pub fn enable_paste(out: &mut String) {
    out.push_str("\x1b[?2004h");
}

pub fn disable_paste(out: &mut String) {
    out.push_str("\x1b[?2004l");
}

pub fn sgr_reset(out: &mut String) {
    out.push_str("\x1b[0m");
}

/// Clears the entire line.
pub fn clear_line(out: &mut String) {
    out.push_str("\x1b[2K");
}

fn modifier_sgr_on(m: Modifier, out: &mut String) {
    let bits = m.bits();
    let mut codes = String::new();
    if bits & Modifier::BOLD.bits() != 0 {
        codes.push_str("1;");
    }
    if bits & Modifier::DIM.bits() != 0 {
        codes.push_str("2;");
    }
    if bits & Modifier::ITALIC.bits() != 0 {
        codes.push_str("3;");
    }
    if bits & Modifier::UNDERLINE.bits() != 0 {
        codes.push_str("4;");
    }
    if bits & Modifier::SLOW_BLINK.bits() != 0 {
        codes.push_str("5;");
    }
    if bits & Modifier::RAPID_BLINK.bits() != 0 {
        codes.push_str("6;");
    }
    if bits & Modifier::REVERSED.bits() != 0 {
        codes.push_str("7;");
    }
    if bits & Modifier::HIDDEN.bits() != 0 {
        codes.push_str("8;");
    }
    if bits & Modifier::CROSSED_OUT.bits() != 0 {
        codes.push_str("9;");
    }
    out.push_str(codes.trim_end_matches(';'));
}

/// Incremental style switching: compares against the previously emitted style and
/// produces the minimal SGR sequence.
///
/// When colors change or modifier bits are removed, performs a reset followed by a
/// full re-application; otherwise only appends the newly added bits.
pub fn push_style(out: &mut String, last: &mut Option<Style>, next: Style) {
    if *last == Some(next) {
        return;
    }
    let next_eff = Style {
        fg: next.fg,
        bg: next.bg,
        add_modifier: next.add_modifier,
        sub_modifier: Modifier::EMPTY,
    };
    let prev = last.unwrap_or_default();
    let prev_eff = Style {
        fg: prev.fg,
        bg: prev.bg,
        add_modifier: prev.add_modifier,
        sub_modifier: Modifier::EMPTY,
    };
    let colors_changed = prev_eff.fg != next_eff.fg || prev_eff.bg != next_eff.bg;
    let removed = prev_eff.add_modifier.bits() & !next_eff.add_modifier.bits() != 0;

    if colors_changed || removed || last.is_none() {
        out.push_str("\x1b[0m");
        match next_eff.fg {
            Some(Color::Reset) | None => {}
            Some(c) => {
                out.push_str("\x1b[");
                c.sgr_fg(out);
                out.push('m');
            }
        }
        match next_eff.bg {
            Some(Color::Reset) | None => {}
            Some(c) => {
                out.push_str("\x1b[");
                c.sgr_bg(out);
                out.push('m');
            }
        }
        let m = next_eff.add_modifier;
        if !m.is_empty() {
            out.push_str("\x1b[");
            modifier_sgr_on(m, out);
            out.push('m');
        }
    } else {
        let added = Modifier::from_bits(next_eff.add_modifier.bits() & !prev_eff.add_modifier.bits());
        if !added.is_empty() {
            out.push_str("\x1b[");
            modifier_sgr_on(added, out);
            out.push('m');
        }
    }
    *last = Some(next);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_to_sequence() {
        let mut s = String::new();
        move_to(&mut s, 1, 1);
        assert_eq!(s, "\x1b[1;1H");
    }

    #[test]
    fn style_minimal_output() {
        let mut s = String::new();
        let mut last = None;
        push_style(&mut s, &mut last, Style::new().fg(Color::Red));
        assert_eq!(s, "\x1b[0m\x1b[31m");
        let n = s.len();
        push_style(&mut s, &mut last, Style::new().fg(Color::Red).add_modifier(Modifier::BOLD));
        assert_eq!(&s[n..], "\x1b[1m");
        let n = s.len();
        push_style(&mut s, &mut last, Style::new().fg(Color::Blue));
        // Color change → reset + reapply
        assert_eq!(&s[n..], "\x1b[0m\x1b[34m");
    }
}
