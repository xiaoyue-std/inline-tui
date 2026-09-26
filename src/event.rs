//! Input event model: keys, mouse, paste, resize, focus.

/// Keyboard key code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Backspace,
    Enter,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Tab,
    BackTab,
    Delete,
    Insert,
    F(u8),
    Char(char),
    Null,
    Esc,
    CapsLock,
    Menu,
}

/// Keyboard modifier bit set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct KeyModifiers(u8);

impl KeyModifiers {
    pub const NONE: KeyModifiers = KeyModifiers(0);
    pub const SHIFT: KeyModifiers = KeyModifiers(1 << 0);
    pub const CONTROL: KeyModifiers = KeyModifiers(1 << 1);
    pub const ALT: KeyModifiers = KeyModifiers(1 << 2);
    pub const SUPER: KeyModifiers = KeyModifiers(1 << 3);
    pub const HYPER: KeyModifiers = KeyModifiers(1 << 4);
    pub const META: KeyModifiers = KeyModifiers(1 << 5);

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn contains(self, other: KeyModifiers) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn intersects(self, other: KeyModifiers) -> bool {
        self.0 & other.0 != 0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// xterm modifier parameter bits (parameter value - 1) conversion.
    pub(crate) fn from_xterm(mask: u8) -> KeyModifiers {
        let mut m = KeyModifiers::NONE;
        if mask & 1 != 0 {
            m.0 |= KeyModifiers::SHIFT.0;
        }
        if mask & 2 != 0 {
            m.0 |= KeyModifiers::ALT.0;
        }
        if mask & 4 != 0 {
            m.0 |= KeyModifiers::CONTROL.0;
        }
        if mask & 8 != 0 {
            m.0 |= KeyModifiers::SUPER.0;
        }
        if mask & 16 != 0 {
            m.0 |= KeyModifiers::HYPER.0;
        }
        if mask & 32 != 0 {
            m.0 |= KeyModifiers::META.0;
        }
        m
    }
}

impl std::ops::BitOr for KeyModifiers {
    type Output = KeyModifiers;
    fn bitor(self, rhs: KeyModifiers) -> KeyModifiers {
        KeyModifiers(self.0 | rhs.0)
    }
}

/// A key press event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyEvent {
    pub fn new(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent { code, modifiers }
    }

    /// A plain character key with no modifiers.
    pub fn char(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    /// Whether this is a Ctrl+X combination.
    pub fn is_ctrl(&self, c: char) -> bool {
        self.code == KeyCode::Char(c) && self.modifiers == KeyModifiers::CONTROL
    }
}

/// Mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

/// Mouse event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseEventKind {
    Down(MouseButton),
    Up(MouseButton),
    Drag(MouseButton),
    Moved,
    ScrollDown,
    ScrollUp,
    ScrollLeft,
    ScrollRight,
}

/// Mouse event (coordinates relative to the terminal screen, 0-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MouseEvent {
    pub kind: MouseEventKind,
    pub column: u16,
    pub row: u16,
    pub modifiers: KeyModifiers,
}

/// Terminal event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Key(KeyEvent),
    Mouse(MouseEvent),
    /// Text received via bracketed paste.
    Paste(String),
    /// Terminal size change (columns, rows).
    Resize(u16, u16),
    FocusGained,
    FocusLost,
    /// Cursor position report response (row, column), 0-based.
    CursorPosition(u16, u16),
}
