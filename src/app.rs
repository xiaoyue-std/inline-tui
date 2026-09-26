//! Application skeleton: background input thread and event channel.

use crate::event::Event;
use crate::input::Parser;
use crate::sys;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

/// Starts a background thread that reads terminal input and polls terminal size.
///
/// Use the returned event receiver with `recv_timeout`: each timeout is one
/// frame tick, which can drive animations such as Spinner.
///
/// ```no_run
/// use std::time::Duration;
/// let rx = cctui::app::spawn_input_thread();
/// loop {
///     match rx.recv_timeout(Duration::from_millis(80)) {
///         Ok(ev) => { /* handle key/mouse/paste/resize */ }
///         Err(_) => { /* tick: redraw animation frame */ }
///     }
/// }
/// ```
pub fn spawn_input_thread() -> Receiver<Event> {
    let (tx, rx) = mpsc::channel();
    let tx_size = tx.clone();
    spawn_thread("cctui-input", move || input_loop(tx));
    spawn_thread("cctui-size", move || size_loop(tx_size));
    rx
}

fn spawn_thread<F: FnOnce() + Send + 'static>(name: &str, f: F) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name(name.to_string())
        .spawn(f)
        .expect("cctui: spawn thread failed")
}

fn input_loop(tx: Sender<Event>) {
    let mut parser = Parser::new();
    let mut buf = [0u8; 1024];
    loop {
        match sys::read_input(&mut buf) {
            Ok(0) => break, // EOF
            Ok(n) => {
                for ev in parser.feed(&buf[..n]) {
                    if tx.send(ev).is_err() {
                        return;
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
}

fn size_loop(tx: Sender<Event>) {
    let mut last = sys::size();
    loop {
        std::thread::sleep(Duration::from_millis(300));
        let cur = sys::size();
        if cur != last {
            last = cur;
            if let Some((w, h)) = cur {
                if tx.send(Event::Resize(w, h)).is_err() {
                    return;
                }
            }
        }
    }
}
