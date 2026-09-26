//! VT/ANSI input byte stream parser.
//!
//! Parses the byte stream sent by the terminal into `Event`s: arrow keys /
//! function keys (CSI, SS3), Ctrl combos, UTF-8 characters, modifier combos
//! (`1;5C` form), Shift+Tab, SGR mouse, bracketed paste, focus events.
//! On Windows, once `ENABLE_VIRTUAL_TERMINAL_INPUT` is enabled it shares the
//! same code path as Unix.

use crate::event::{
    Event, KeyCode, KeyModifiers, KeyEvent, MouseButton, MouseEvent, MouseEventKind,
};

#[derive(Debug)]
enum State {
    Normal,
    Esc,
    Csi { params: String },
    Ss3,
    OscEsc,
    Paste { buf: Vec<u8> },
}

#[derive(Debug, Default)]
struct Utf8Acc {
    bytes: Vec<u8>,
    remaining: u8,
}

/// Stateful input parser. Keeps intermediate state across `feed` calls (handles split packets).
#[derive(Debug)]
pub struct Parser {
    state: State,
    utf8: Utf8Acc,
}

impl Default for Parser {
    fn default() -> Parser {
        Parser::new()
    }
}

impl Parser {
    pub fn new() -> Parser {
        Parser { state: State::Normal, utf8: Utf8Acc::default() }
    }

    /// Feeds a batch of bytes and returns the complete events parsed from it.
    pub fn feed(&mut self, input: &[u8]) -> Vec<Event> {
        let mut events = Vec::new();
        for &b in input {
            self.byte(b, &mut events);
        }
        // Standalone ESC: short Esc press (Alt+letter usually arrives in the same batch)
        if matches!(self.state, State::Esc) {
            self.state = State::Normal;
            events.push(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
        }
        events
    }

    fn byte(&mut self, b: u8, events: &mut Vec<Event>) {
        match std::mem::replace(&mut self.state, State::Normal) {
            State::Normal => self.normal(b, events),
            State::Esc => {
                self.state = State::Normal;
                self.esc(b, events);
            }
            State::Csi { params } => self.csi(params, b, events),
            State::Ss3 => {
                self.ss3(b, events);
            }
            State::OscEsc => {
                // OSC terminates with ESC \; swallow the '\\'
                if b != b'\\' {
                    self.state = State::Normal;
                    self.byte(b, events);
                }
            }
            State::Paste { mut buf } => {
                buf.push(b);
                if buf.ends_with(b"\x1b[201~") {
                    buf.truncate(buf.len() - 6);
                    let text = String::from_utf8_lossy(&buf).into_owned();
                    events.push(Event::Paste(text));
                    self.state = State::Normal;
                } else {
                    self.state = State::Paste { buf };
                }
            }
        }
    }

    fn normal(&mut self, b: u8, events: &mut Vec<Event>) {
        match b {
            0x1b => self.state = State::Esc,
            b'\r' | b'\n' => key(events, KeyCode::Enter, KeyModifiers::NONE),
            b'\t' => key(events, KeyCode::Tab, KeyModifiers::NONE),
            0x7f | 0x08 => key(events, KeyCode::Backspace, KeyModifiers::NONE),
            0x00 => key(events, KeyCode::Char(' '), KeyModifiers::CONTROL),
            0x01..=0x1a => {
                let c = (b'a' + b - 1) as char;
                key(events, KeyCode::Char(c), KeyModifiers::CONTROL);
            }
            0x1c..=0x1f => {
                let c = (b'4' + b - 0x1c) as char;
                key(events, KeyCode::Char(c), KeyModifiers::CONTROL);
            }
            0x20..=0x7e => key(events, KeyCode::Char(b as char), KeyModifiers::NONE),
            _ => self.utf8_step(b, events), // >= 0x80
        }
    }

    fn esc(&mut self, b: u8, events: &mut Vec<Event>) {
        match b {
            b'[' => self.state = State::Csi { params: String::new() },
            b']' => self.state = State::OscEsc, // simplified: skip OSC until ESC \
            b'O' => self.state = State::Ss3,
            0x1b => key(events, KeyCode::Esc, KeyModifiers::NONE),
            0x7f => key(events, KeyCode::Backspace, KeyModifiers::ALT),
            b'\r' => key(events, KeyCode::Enter, KeyModifiers::ALT),
            0x01..=0x1a => {
                let c = (b'a' + b - 1) as char;
                key(events, KeyCode::Char(c), KeyModifiers::ALT | KeyModifiers::CONTROL);
            }
            _ if (0x20..=0x7e).contains(&b) => {
                key(events, KeyCode::Char(b as char), KeyModifiers::ALT)
            }
            _ => {} // ignore the rest
        }
    }

    fn csi(&mut self, mut params: String, b: u8, events: &mut Vec<Event>) {
        match b {
            0x30..=0x3f => {
                // Parameter bytes: digits ; : < = > ?
                params.push(b as char);
                self.state = State::Csi { params };
            }
            0x20..=0x2f => {
                // Intermediate bytes, ignored
                self.state = State::Csi { params };
            }
            0x40..=0x7e => {
                // Final byte, dispatch
                self.dispatch_csi(&params, b, events);
            }
            _ => {
                // Sequence interrupted, dropped
            }
        }
    }

    fn dispatch_csi(&mut self, params: &str, final_byte: u8, events: &mut Vec<Event>) {
        let mods = modifier_from_params(params);
        let p = params.strip_prefix('<').unwrap_or(params);
        let mut nums = p.split(';').map(|s| s.parse::<u32>().ok());
        let first = nums.next().flatten();

        match final_byte {
            b'A' => key(events, KeyCode::Up, mods),
            b'B' => key(events, KeyCode::Down, mods),
            b'C' => key(events, KeyCode::Right, mods),
            b'D' => key(events, KeyCode::Left, mods),
            b'H' => key(events, KeyCode::Home, mods),
            b'F' => key(events, KeyCode::End, mods),
            b'Z' => key(events, KeyCode::BackTab, KeyModifiers::SHIFT),
            b'P' => key(events, KeyCode::F(1), mods),
            b'Q' => key(events, KeyCode::F(2), mods),
            b'R' if !params.contains(';') => key(events, KeyCode::F(3), mods),
            b'S' => key(events, KeyCode::F(4), mods),
            b'M' | b'm' if params.starts_with('<') => {
                self.mouse_event(p, final_byte == b'M', events);
            }
            b'I' => events.push(Event::FocusGained),
            b'O' => events.push(Event::FocusLost),
            b'R' => {
                // Cursor position report: row;col
                let row = first.unwrap_or(1).saturating_sub(1) as u16;
                let col = nums.next().flatten().unwrap_or(1).saturating_sub(1) as u16;
                events.push(Event::CursorPosition(col, row));
            }
            b'~' => {
                let code = match first.unwrap_or(0) {
                    1 | 7 => KeyCode::Home,
                    2 => KeyCode::Insert,
                    3 => KeyCode::Delete,
                    4 | 8 => KeyCode::End,
                    5 => KeyCode::PageUp,
                    6 => KeyCode::PageDown,
                    11 => KeyCode::F(1),
                    12 => KeyCode::F(2),
                    13 => KeyCode::F(3),
                    14 => KeyCode::F(4),
                    15 => KeyCode::F(5),
                    17 => KeyCode::F(6),
                    18 => KeyCode::F(7),
                    19 => KeyCode::F(8),
                    20 => KeyCode::F(9),
                    21 => KeyCode::F(10),
                    23 => KeyCode::F(11),
                    24 => KeyCode::F(12),
                    200 => {
                        self.state = State::Paste { buf: Vec::new() };
                        return;
                    }
                    201 => return, // orphaned paste-end marker
                    _ => return,
                };
                key(events, code, mods);
            }
            _ => {}
        }
    }

    fn mouse_event(&mut self, params: &str, press_or_move: bool, events: &mut Vec<Event>) {
        let mut it = params.split(';').filter_map(|s| s.parse::<u16>().ok());
        let (Some(cb), Some(x), Some(y)) = (it.next(), it.next(), it.next()) else {
            return;
        };
        let mut mods = KeyModifiers::NONE;
        if cb & 4 != 0 {
            mods = mods | KeyModifiers::SHIFT;
        }
        if cb & 8 != 0 {
            mods = mods | KeyModifiers::ALT;
        }
        if cb & 16 != 0 {
            mods = mods | KeyModifiers::CONTROL;
        }
        let button = |b: u16| match b & 3 {
            0 => MouseButton::Left,
            1 => MouseButton::Middle,
            2 => MouseButton::Right,
            _ => MouseButton::Left,
        };
        let kind = if cb & 64 != 0 {
            match cb & 3 {
                0 => MouseEventKind::ScrollUp,
                1 => MouseEventKind::ScrollDown,
                2 => MouseEventKind::ScrollLeft,
                _ => MouseEventKind::ScrollRight,
            }
        } else if !press_or_move {
            MouseEventKind::Up(button(cb))
        } else if cb & 32 != 0 {
            if cb & 3 == 3 {
                MouseEventKind::Moved
            } else {
                MouseEventKind::Drag(button(cb))
            }
        } else {
            MouseEventKind::Down(button(cb))
        };
        events.push(Event::Mouse(MouseEvent {
            kind,
            column: x.saturating_sub(1),
            row: y.saturating_sub(1),
            modifiers: mods,
        }));
    }

    fn ss3(&mut self, b: u8, events: &mut Vec<Event>) {
        let code = match b {
            b'A' => KeyCode::Up,
            b'B' => KeyCode::Down,
            b'C' => KeyCode::Right,
            b'D' => KeyCode::Left,
            b'H' => KeyCode::Home,
            b'F' => KeyCode::End,
            b'P' => KeyCode::F(1),
            b'Q' => KeyCode::F(2),
            b'R' => KeyCode::F(3),
            b'S' => KeyCode::F(4),
            _ => return,
        };
        key(events, code, KeyModifiers::NONE);
    }

    fn utf8_step(&mut self, b: u8, events: &mut Vec<Event>) {
        let acc = &mut self.utf8;
        if acc.remaining == 0 {
            if (0xC2..=0xDF).contains(&b) {
                acc.bytes = vec![b];
                acc.remaining = 1;
            } else if (0xE0..=0xEF).contains(&b) {
                acc.bytes = vec![b];
                acc.remaining = 2;
            } else if (0xF0..=0xF4).contains(&b) {
                acc.bytes = vec![b];
                acc.remaining = 3;
            } else {
                // Orphan continuation byte or invalid byte
                key(events, KeyCode::Char('\u{FFFD}'), KeyModifiers::NONE);
            }
            return;
        }
        if b & 0xC0 != 0x80 {
            // Invalid sequence
            acc.bytes.clear();
            acc.remaining = 0;
            key(events, KeyCode::Char('\u{FFFD}'), KeyModifiers::NONE);
            return;
        }
        acc.bytes.push(b);
        acc.remaining -= 1;
        if acc.remaining == 0 {
            let bytes = std::mem::take(&mut acc.bytes);
            if let Ok(s) = std::str::from_utf8(&bytes) {
                for ch in s.chars() {
                    key(events, KeyCode::Char(ch), KeyModifiers::NONE);
                }
            }
        }
    }
}

fn key(events: &mut Vec<Event>, code: KeyCode, modifiers: KeyModifiers) {
    events.push(Event::Key(KeyEvent::new(code, modifiers)));
}

/// Parses the CSI modifier parameter (`5` in `1;5C` → Ctrl; value-1 is the bit mask).
fn modifier_from_params(params: &str) -> KeyModifiers {
    let p = params.strip_prefix('<').unwrap_or(params);
    p.split(';')
        .filter_map(|s| s.parse::<u8>().ok())
        .nth(1)
        .map(|v| KeyModifiers::from_xterm(v.saturating_sub(1)))
        .unwrap_or(KeyModifiers::NONE)
}
