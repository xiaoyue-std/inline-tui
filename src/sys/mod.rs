//! Platform terminal backends: raw mode, VT toggling, size query, raw input reading.
//!
//! Both platforms expose identically named APIs, called uniformly by `terminal.rs`.

#[cfg(windows)]
pub mod windows;
#[cfg(unix)]
pub mod unix;

#[cfg(windows)]
pub use windows::{enable_raw, panic_restore, read_input, size, TerminalState};

#[cfg(unix)]
pub use unix::{enable_raw, panic_restore, read_input, size, TerminalState};
