//! # stilt
//!
//! A general-purpose Rust TUI widget library: inline rendering, double-buffered
//! diffing, full input events, ready-made widgets and animations — implemented
//! entirely from scratch with zero third-party dependencies.
//!
//! Features:
//! - **Inline rendering**: the UI occupies only a few lines at the bottom of the
//!   terminal, preserving the scrollback history above (fullscreen alternate
//!   screen mode is also supported)
//! - **Diff rendering**: double-buffer cell comparison emits only changed
//!   characters each frame, without flicker
//! - **In-house terminal backend**: Windows VT / POSIX termios dual platform,
//!   hand-written FFI, no dependencies
//! - **Full widget set**: List / Table / Tabs / Form / Sparkline / Markdown /
//!   Diff / multi-line editor / slash completion / scrollbar / animations
//!   (color wave, glow, pulse, progress bar…)
//!
//! See `examples/gallery.rs` for typical usage:
//!
//! ```no_run
//! use stilt::widgets::{Paragraph, Widget};
//! use stilt::{Event, KeyCode, KeyModifiers, Terminal};
//! use std::sync::mpsc::RecvTimeoutError;
//! use std::time::Duration;
//!
//! fn main() -> stilt::Result<()> {
//!     let mut term = Terminal::inline(24)?;
//!     term.enable_mouse().enable_paste();
//!     let rx = stilt::app::spawn_input_thread();
//!     loop {
//!         match rx.recv_timeout(Duration::from_millis(80)) {
//!             Ok(Event::Key(k))
//!                 if k.code == KeyCode::Char('c') && k.modifiers == KeyModifiers::CONTROL =>
//!             {
//!                 break;
//!             }
//!             Ok(_ev) => { /* model.update(ev) */ }
//!             Err(RecvTimeoutError::Timeout) => { /* model.tick(): drive animations */ }
//!             Err(_) => break,
//!         }
//!         term.draw(24, |frame| {
//!             Paragraph::new("hello stilt").render(frame.area, frame.buffer);
//!         })?;
//!     }
//!     Ok(())
//! }
//! ```

pub mod ansi;
pub mod app;
pub mod buffer;
pub mod event;
pub mod input;
pub mod layout;
pub mod markdown;
pub mod highlight;
pub mod style;
pub mod sys;
pub mod terminal;
pub mod text;
pub mod theme;
pub mod width;
pub mod widgets;

pub use buffer::{Buffer, Cell};
pub use event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
pub use layout::{Constraint, Rect};
pub use style::{Color, Modifier, Style};
pub use terminal::Terminal;
pub use text::{Line, Span, Text};

/// The library's unified error type.
#[derive(Debug)]
pub enum Error {
    /// Underlying I/O error.
    Io(std::io::Error),
    /// The current environment is not a supported terminal (e.g. output is
    /// redirected, or VT is unavailable).
    TerminalUnavailable(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "io error: {}", e),
            Error::TerminalUnavailable(m) => write!(f, "terminal unavailable: {}", m),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

/// The library's unified Result type.
pub type Result<T> = std::result::Result<T, Error>;
