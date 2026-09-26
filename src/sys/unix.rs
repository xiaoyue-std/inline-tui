//! POSIX terminal backend (Linux / macOS / WSL, first-class support).
//!
//! Declares libc symbols directly (std already links libc, no libc crate needed).
//! The termios struct layout differs per platform, so it is saved/restored verbatim
//! in an aligned byte buffer and only modified via `cfmakeraw`, avoiding hardcoded
//! field offsets.
//!
//! Robustness design:
//! - Falls back to `/dev/tty` when stdin is not a TTY (IDEs / task runners often
//!   pipe stdin)
//! - Size query tries stdout first, then the TTY fd
//! - Installs SIGHUP/SIGINT/SIGTERM handlers: restores termios before abnormal
//!   termination to avoid a corrupted terminal
//! - Raw mode disables ISIG, so Ctrl+C enters the parser as byte `0x03` and the
//!   application decides its semantics

use std::ffi::c_ulong;
use std::sync::atomic::{AtomicI32, AtomicPtr, Ordering};

/// Opaque byte buffer for the termios struct (glibc/musl 60 bytes, Darwin 56 bytes;
/// 8-byte alignment leaves ample room).
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct TermiosBuf(pub [u64; 12]);

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct WinSize {
    ws_row: u16,
    ws_col: u16,
    ws_xpixel: u16,
    ws_ypixel: u16,
}

/// Restore info accessible from the signal handler (written once by enable_raw, read-only afterwards).
#[repr(C)]
struct RestoreInfo {
    fd: i32,
    saved: TermiosBuf,
}

extern "C" {
    fn tcgetattr(fd: i32, termios_p: *mut TermiosBuf) -> i32;
    fn tcsetattr(fd: i32, optional_actions: i32, termios_p: *const TermiosBuf) -> i32;
    fn cfmakeraw(termios_p: *mut TermiosBuf);
    fn ioctl(fd: i32, request: c_ulong, ...) -> i32;
    fn open(path: *const u8, flags: i32) -> i32;
    fn close(fd: i32) -> i32;
    fn read(fd: i32, buf: *mut u8, count: usize) -> isize;
    fn signal(sig: i32, handler: usize) -> usize;
    fn raise(sig: i32) -> i32;
}

#[cfg(target_os = "linux")]
const TIOCGWINSZ: c_ulong = 0x5413;
#[cfg(not(target_os = "linux"))]
const TIOCGWINSZ: c_ulong = 0x40087468;

const TCSANOW: i32 = 0;
const O_RDWR: i32 = 2;

// Linux and macOS share the same signal numbers (BSD tradition)
const SIGHUP: i32 = 1;
const SIGINT: i32 = 2;
const SIGTERM: i32 = 15;
const SIG_DFL: usize = 0;

static RESTORE: AtomicPtr<RestoreInfo> = AtomicPtr::new(std::ptr::null_mut());
/// The TTY fd used for input reading (set by enable_raw; 0 = stdin).
static INPUT_FD: AtomicI32 = AtomicI32::new(0);

/// Signal handler: only performs syscalls (tcsetattr / signal / raise), async-signal-safe.
extern "C" fn restore_on_signal(sig: i32) {
    unsafe {
        let p = RESTORE.load(Ordering::Acquire);
        if !p.is_null() {
            let info = &*p;
            tcsetattr(info.fd, TCSANOW, &info.saved);
        }
        signal(sig, SIG_DFL);
        raise(sig);
    }
}

fn install_signal_handlers() {
    let handler = restore_on_signal as extern "C" fn(i32) as usize;
    unsafe {
        signal(SIGHUP, handler);
        signal(SIGINT, handler);
        signal(SIGTERM, handler);
    }
}

fn uninstall_signal_handlers() {
    unsafe {
        signal(SIGHUP, SIG_DFL);
        signal(SIGINT, SIG_DFL);
        signal(SIGTERM, SIG_DFL);
    }
}

/// The saved terminal settings, used for restoration.
#[derive(Debug, Clone, Copy)]
pub struct TerminalState {
    fd: i32,
    owns_fd: bool,
    saved: TermiosBuf,
}

impl TerminalState {
    pub fn restore(&mut self) {
        unsafe {
            tcsetattr(self.fd, TCSANOW, &self.saved);
            if self.owns_fd {
                // On close the input thread may still be blocked in read on this fd;
                // read exits with EBADF/EOF and the thread ends. The fd-reuse window
                // is tiny and acceptable.
                close(self.fd);
            }
        }
        uninstall_signal_handlers();
        let p = RESTORE.swap(std::ptr::null_mut(), Ordering::AcqRel);
        if !p.is_null() {
            drop(unsafe { Box::from_raw(p) });
        }
        INPUT_FD.store(0, Ordering::Release);
    }
}

/// Enables raw mode.
///
/// Uses fd 0 directly when stdin is a TTY; otherwise opens `/dev/tty` (IDE pipe scenarios).
pub fn enable_raw() -> crate::Result<TerminalState> {
    unsafe {
        let mut buf = TermiosBuf([0; 12]);
        let mut fd = 0i32;
        let mut owns_fd = false;
        if tcgetattr(0, &mut buf) != 0 {
            let tty = open(b"/dev/tty\0".as_ptr(), O_RDWR);
            if tty < 0 || tcgetattr(tty, &mut buf) != 0 {
                if tty >= 0 {
                    close(tty);
                }
                return Err(crate::Error::TerminalUnavailable(
                    "no usable TTY (stdin is not a TTY and /dev/tty cannot be opened)".into(),
                ));
            }
            fd = tty;
            owns_fd = true;
        }
        let saved = buf;
        cfmakeraw(&mut buf);
        if tcsetattr(fd, TCSANOW, &buf) != 0 {
            if owns_fd {
                close(fd);
            }
            return Err(crate::Error::Io(std::io::Error::last_os_error()));
        }
        INPUT_FD.store(fd, Ordering::Release);
        RESTORE.store(
            Box::into_raw(Box::new(RestoreInfo { fd, saved })),
            Ordering::Release,
        );
        install_signal_handlers();
        Ok(TerminalState { fd, owns_fd, saved })
    }
}

/// Queries the terminal size (columns, rows): tries stdout first, then the TTY fd.
pub fn size() -> Option<(u16, u16)> {
    let mut ws = WinSize::default();
    let input_fd = INPUT_FD.load(Ordering::Acquire);
    for fd in [1, input_fd] {
        if fd < 0 {
            continue;
        }
        let r = unsafe { ioctl(fd, TIOCGWINSZ, &mut ws as *mut WinSize) };
        if r == 0 && ws.ws_col > 0 && ws.ws_row > 0 {
            return Some((ws.ws_col, ws.ws_row));
        }
    }
    None
}

/// Blocking read of raw bytes from the TTY.
pub fn read_input(buf: &mut [u8]) -> std::io::Result<usize> {
    let fd = INPUT_FD.load(Ordering::Acquire);
    loop {
        let n = unsafe { read(fd, buf.as_mut_ptr(), buf.len()) };
        if n < 0 {
            let e = std::io::Error::last_os_error();
            if e.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(e);
        }
        return Ok(n as usize);
    }
}

/// Best-effort termios restoration in the panic hook.
pub fn panic_restore() {
    let p = RESTORE.swap(std::ptr::null_mut(), Ordering::AcqRel);
    if !p.is_null() {
        unsafe {
            let info = &*p;
            tcsetattr(info.fd, TCSANOW, &info.saved);
            drop(Box::from_raw(p));
        }
    }
}
