//! Windows console backend.
//!
//! Hand-written kernel32 FFI (zero dependencies): enables VT processing on output,
//! VT byte-stream mode on input, and reads raw bytes directly from the console
//! handle via ReadFile, handing them to the generic VT parser.

#![allow(non_snake_case)]

use std::sync::Mutex;

type Handle = *mut std::ffi::c_void;

const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
const STD_INPUT_HANDLE: u32 = -10i32 as u32;

const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
const ENABLE_VIRTUAL_TERMINAL_INPUT: u32 = 0x0200;

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Coord {
    x: i16,
    y: i16,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct SmallRect {
    left: i16,
    top: i16,
    right: i16,
    bottom: i16,
}

#[repr(C)]
struct ConsoleScreenBufferInfo {
    dw_size: Coord,
    dw_cursor_position: Coord,
    w_attributes: u16,
    sr_window: SmallRect,
    dw_maximum_window_size: Coord,
}

#[link(name = "kernel32")]
extern "system" {
    fn GetStdHandle(n_std_handle: u32) -> Handle;
    fn GetConsoleMode(h_console_handle: Handle, lp_mode: *mut u32) -> i32;
    fn SetConsoleMode(h_console_handle: Handle, dw_mode: u32) -> i32;
    fn GetConsoleScreenBufferInfo(
        h_console_output: Handle,
        lp_console_screen_buffer_info: *mut ConsoleScreenBufferInfo,
    ) -> i32;
    fn ReadFile(
        h_file: Handle,
        lp_buffer: *mut u8,
        n_number_of_bytes_to_read: u32,
        lp_number_of_bytes_read: *mut u32,
        lp_overlapped: *mut std::ffi::c_void,
    ) -> i32;
}

/// The saved console modes, used for restoration.
#[derive(Debug, Clone, Copy)]
pub struct TerminalState {
    out_saved: u32,
    in_saved: u32,
}

impl TerminalState {
    pub fn restore(&mut self) {
        unsafe { restore_modes(*self) }
    }
}

static SAVED: Mutex<Option<TerminalState>> = Mutex::new(None);

unsafe fn restore_modes(s: TerminalState) {
    let out = GetStdHandle(STD_OUTPUT_HANDLE);
    let inp = GetStdHandle(STD_INPUT_HANDLE);
    SetConsoleMode(out, s.out_saved);
    SetConsoleMode(inp, s.in_saved);
}

/// Enables raw mode + VT sequence support.
pub fn enable_raw() -> crate::Result<TerminalState> {
    unsafe {
        let out = GetStdHandle(STD_OUTPUT_HANDLE);
        let inp = GetStdHandle(STD_INPUT_HANDLE);
        if out.is_null() || inp.is_null() {
            return Err(unavailable());
        }
        let mut out_mode = 0u32;
        let mut in_mode = 0u32;
        // GetConsoleMode failing means stdout/stdin is not a console (redirected, etc.)
        if GetConsoleMode(out, &mut out_mode) == 0 || GetConsoleMode(inp, &mut in_mode) == 0 {
            return Err(unavailable());
        }
        // Output: enable VT processing (keep the other default bits)
        if SetConsoleMode(out, out_mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING) == 0 {
            return Err(unavailable());
        }
        // Input: pure VT byte stream (disables line input/echo/processing/Ctrl+C system handling)
        SetConsoleMode(inp, ENABLE_VIRTUAL_TERMINAL_INPUT);
        let st = TerminalState { out_saved: out_mode, in_saved: in_mode };
        if let Ok(mut g) = SAVED.lock() {
            *g = Some(st);
        }
        Ok(st)
    }
}

fn unavailable() -> crate::Error {
        crate::Error::TerminalUnavailable(
            "requires a Windows 10+ terminal with VT sequence support (Windows Terminal recommended)"
                .into(),
        )
}

/// Queries the visible terminal window size (columns, rows).
pub fn size() -> Option<(u16, u16)> {
    unsafe {
        let out = GetStdHandle(STD_OUTPUT_HANDLE);
        let mut info: ConsoleScreenBufferInfo = std::mem::zeroed();
        if GetConsoleScreenBufferInfo(out, &mut info) == 0 {
            return None;
        }
        let w = info.sr_window.right - info.sr_window.left + 1;
        let h = info.sr_window.bottom - info.sr_window.top + 1;
        if w <= 0 || h <= 0 {
            None
        } else {
            Some((w as u16, h as u16))
        }
    }
}

/// Blocking read of raw bytes from the console input handle.
pub fn read_input(buf: &mut [u8]) -> std::io::Result<usize> {
    unsafe {
        let inp = GetStdHandle(STD_INPUT_HANDLE);
        let mut read = 0u32;
        let ok = ReadFile(
            inp,
            buf.as_mut_ptr(),
            buf.len() as u32,
            &mut read,
            std::ptr::null_mut(),
        );
        if ok == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(read as usize)
    }
}

/// Best-effort console mode restoration in the panic hook.
pub fn panic_restore() {
    if let Ok(mut g) = SAVED.try_lock() {
        if let Some(s) = g.take() {
            unsafe { restore_modes(s) }
        }
    }
}
