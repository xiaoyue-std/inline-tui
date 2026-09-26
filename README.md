# stilt

A **general-purpose Rust TUI widget library** — built entirely from scratch with **zero
third-party dependencies** (std + hand-written FFI only). Component model in the spirit of
ratatui/FTXUI, with two signature features: **inline rendering** (the UI lives at the bottom
of the terminal while scrollback history stays intact) and **double-buffered differential
rendering**.

```
╭ ✻ chat ───────────────────────────────────────────────╮
│ Message… ('/' commands · Alt+Enter newline · Ctrl+C)  │
╰───────────────────────────────────────────────────────╯
 ✻ chat   / commands · pgup/dn history · ctrl+c quit
```

## Widgets

| Category | Components |
|---|---|
| **Layout** | `Rect`, constraint-based `vsplit`/`hsplit` (Length / Min / Max / Percentage / Fill) |
| **Basics** | `Block` borders (Plain/Rounded/Thick/Double + titles), `Paragraph` with word wrap, `Viewport` (follow-bottom scrolling), `Scrollbar` (vertical/horizontal), `Spinner`, `StatusBar` |
| **Collections** | `List` (selection + auto-follow), `Table` (constraint column widths + row selection), `Tabs`, popup `Menu` |
| **Forms** | `Checkbox`, `RadioGroup`, `Editor` — a multi-line input with word-wise cursor motion, Ctrl shortcuts, undo/redo (Ctrl+Z/Y, grouped typing), input history, and slash-command completion |
| **Data** | `Sparkline` (block-character bar chart), `DiffView` (colored unified diff), `Collapsible` panels |
| **Content** | Markdown rendering (headings/lists/quotes/fenced code with borders; streaming-friendly), syntax highlighting for 10 languages |
| **Animation** | `Wave`, `Shimmer`, `ProgressBar` (gradient / indeterminate), `LoadingDots`, `Typewriter`, `Ticker`, `pulse_color` — every animation is a pure function of time, driven by `.at(elapsed)`, fully deterministic to test |
| **Styling** | `Color` (16/256/RGB), `Modifier` bitflags, `Style` (patch semantics), `Theme` |

### Rendering core

- **Inline mode**: the region is bottom-anchored; height changes clear exposed rows or scroll
  in new lines. A fullscreen alternate-screen mode is also provided.
- **Differential rendering**: prev/next cell-grid diff → minimal ANSI output with incremental
  SGR; flicker-free even under streaming updates.
- **Unicode width**: a built-in wcwidth table (CJK/fullwidth/emoji = 2 columns, combining
  marks = 0) is used everywhere — wrapping, cursors, alignment. Wide characters are never
  split in half.
- **Terminal state safety**: raw-mode save/restore via `Drop` plus a panic hook; on Unix a
  signal handler restores termios on SIGHUP/SIGINT/SIGTERM.

### Event system

A background input thread feeds an `mpsc` channel: full VT key decoding (modifiers,
Shift+Tab, F-keys, UTF-8), SGR mouse (wheel/drag), bracketed paste (safe across chunk
boundaries), focus events, resize polling. `recv_timeout` doubles as the frame tick for
animations.

## Quick start

```toml
[dependencies]
stilt = "0.1"
```

```rust
use stilt::widgets::{List, ListState, Widget};
use stilt::text::Line;
use stilt::{Event, KeyCode, Terminal};
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

fn main() -> stilt::Result<()> {
    let mut term = Terminal::inline(12)?;       // 12-row inline region at the bottom
    term.enable_mouse().enable_paste();
    let rx = stilt::app::spawn_input_thread();  // background input thread

    let items = vec![Line::raw("apple"), Line::raw("banana"), Line::raw("cherry")];
    let mut state = ListState { offset: 0, selected: Some(0) };

    loop {
        match rx.recv_timeout(Duration::from_millis(80)) {
            Ok(Event::Key(k)) if k.is_ctrl('c') => break,
            Ok(Event::Key(k)) => match k.code {
                KeyCode::Down => state.next(items.len()),
                KeyCode::Up => state.previous(),
                _ => {}
            },
            Ok(_ev) => {}                        // mouse / paste / resize
            Err(RecvTimeoutError::Timeout) => {} // tick: drive animations
            Err(_) => break,
        }
        term.draw(12, |frame| {
            List::new(&items, &state).render(frame.area, frame.buffer);
        })?;
    }
    Ok(()) // Terminal's Drop restores the terminal automatically
}
```

The component model is ratatui-like: interaction state lives in your app, and
`Widget::render(self, area, &mut Buffer)` rebuilds the frame every tick.

## Examples

| Command | What it shows |
|---|---|
| `cargo run --example quickstart` | The smallest useful app: inline region, event loop, `List` + `StatusBar` |

## Documentation

| Doc | Contents |
|---|---|
| [docs/DESIGN.md](docs/DESIGN.md) | Technical design: rendering pipeline, inline anchoring, width system, VT parser, undo architecture, testing strategy |
| [AGENT.md](AGENT.md) | Hard rules, invariants, pitfalls and definition-of-done for AI coding agents |

## Architecture

```
src/
├── style.rs      Color(16/256/RGB) · Modifier · Style
├── width.rs      wcwidth table (CJK/emoji/combining marks)
├── text.rs       Span/Line/Text · styled word wrapping (wide chars never split)
├── buffer.rs     Cell/Buffer · cell-level diff
├── ansi.rs       SGR/cursor/mode sequences · incremental style switching
├── sys/          platform layer (the only place with `unsafe`)
│   ├── windows.rs  kernel32 FFI: VT toggles, raw input, size
│   └── unix.rs     libc FFI: termios save/restore, /dev/tty fallback, signal handlers
├── input.rs      VT byte-stream parser (CSI/SS3/mouse/paste/focus, resumable across reads)
├── event.rs      Event/Key/Mouse model
├── layout.rs     Rect · constraint layout
├── terminal.rs   inline/fullscreen sessions · differential render loop · panic hook
├── app.rs        background input thread + size polling
├── markdown.rs   block + inline parsing → rich text (streaming-friendly)
├── highlight.rs  hand-written tokenizer × 10 languages
├── theme.rs      semantic roles → styles (default dark theme)
└── widgets/      anim · block · checkbox · collapsible · diff · input(editor)
                  list · menu · paragraph · scrollbar · sparkline · spinner
                  statusbar · table · tabs · viewport
```

Data flow: **input thread** → (VT parsing) → `Event` channel → **main loop** (update) →
widgets paint into a `Buffer` → diff against the previous frame → minimal ANSI output.

## Platform support

| Platform | Status |
|---|---|
| Windows 10+ (Windows Terminal) | ✅ Primary development/test platform |
| Linux (GNOME Terminal/Konsole/Alacritty/kitty, incl. WSL2) | ✅ First-class |
| macOS | ✅ Same POSIX path (covered by cross-compile checks) |

Linux/macOS robustness: falls back to `/dev/tty` when stdin is not a TTY (IDE/task-runner
pipes work fine); SIGHUP/INT/TERM handlers restore termios before abnormal termination;
raw mode keeps ISIG off so Ctrl+C arrives as byte `0x03` and the app decides what it means.

Cross-compile checks pass for `x86_64-unknown-linux-gnu` and `aarch64-apple-darwin`;
runtime behavior has been verified end-to-end under WSL2 (Ubuntu 26.04, kali).

## Known limits (honest roadmap)

- Markdown tables render as plain paragraphs; no nested bold-italic inline parsing
- Editor history/completion is in-memory only, no persistence
- Markdown is re-parsed fully each frame (fine at demo scale)
- Not yet: single-line `Input`, `Gauge`, `Canvas`, image protocols (sixel/kitty)

See [AGENT.md](AGENT.md) for AI coding-agent guidelines.

## License

MIT
