# stilt — Technical Design

This document explains how stilt works internally: the architecture layers, the rendering
pipeline, the inline-mode algorithm, the width system, the VT input parser, the editor's
undo design, and the testing strategy. For usage see the [README](../README.md); for
contribution rules see [AGENT.md](../AGENT.md).

## 1. Design goals

1. **Zero third-party dependencies** — only `std` plus hand-written FFI declarations
   (`kernel32` / libc). Every capability that other stacks pull in as separate crates
   (crossterm, unicode-width, editors, markdown) is implemented in-tree.
2. **Flicker-free streaming** — the frame contract is *emit only what changed*; no
   clear-screen sequences ever.
3. **Inline-first** — the UI is bottom-anchored inside the normal terminal flow so
   scrollback stays intact (the shape of modern AI CLIs), with fullscreen as the secondary
   mode.
4. **CJK-correct by construction** — a built-in wcwidth table feeds every width-sensitive
   path (wrap, cursor, diff, scrollbars); wide characters are never split.
5. **Crash-safe terminal state** — raw-mode state is saved and restored through three
   independent mechanisms (`Drop`, panic hook, Unix signal handlers).

## 2. Layer map

```
┌─────────────────────────────────────────────────────────┐
│ examples / apps          (model.update / view assembly) │
├─────────────────────────────────────────────────────────┤
│ terminal.rs   inline/fullscreen session, draw loop,     │
│               diff compiler, panic hook                 │
│ app.rs        input reader thread + size poller         │
├─────────────────────────────────────────────────────────┤
│ widgets/*     state-external render-only components     │
│ markdown.rs highlight.rs  (content layer)               │
│ layout.rs     Rect + constraint solving                 │
├─────────────────────────────────────────────────────────┤
│ text.rs       Span/Line/Text + styled wrapping          │
│ width.rs      wcwidth table                             │
│ style.rs/theme.rs  Color/Modifier/Style/Theme           │
│ buffer.rs     Cell grid + cell-level diff               │
│ ansi.rs       escape primitives + incremental SGR       │
├─────────────────────────────────────────────────────────┤
│ input.rs      stateful VT byte-stream parser            │
│ event.rs      Event model                               │
│ sys/{windows,unix}.rs   FFI backends (the only unsafe)  │
└─────────────────────────────────────────────────────────┘
```

Data flow: input thread → (VT parsing) → `Event` channel → app main loop
(`recv_timeout`, timeout = tick) → widgets paint into a `Buffer` → diff against the
previous frame → minimal ANSI byte string → stdout.

## 3. Rendering pipeline

### 3.1 Frame lifecycle (`Terminal::draw`)

Each call to `draw(height, render)` runs:

1. **Size poll** — query the OS for the current size; if it differs from the cached one,
   mark the frame for full redraw (`need_full = true`). A background thread also emits
   `Event::Resize` (300 ms poll) so apps can react.
2. **Height clamp** — inline mode clamps the requested height into `1..=rows-1`
   (one row always remains for the shell prompt line); fullscreen uses `rows`.
3. **Region height change** (inline only, see §4).
4. **Buffer rebuild** — if the area changed shape, the previous buffer is reset. A fresh
   `cur` buffer is allocated, the render closure paints widgets into it through a
   `Frame` (which can also request a cursor position).
5. **Diff compilation** — `compile_frame` walks every cell of `cur`:
   - skip continuation cells of wide characters (`Cell.symbol == ""`);
   - skip cells identical to `prev` unless a full redraw is due;
   - emit `\x1b[<row>;<col>H` only when the output position is not contiguous with the
     last write;
   - emit styles through `push_style` (§3.2);
   - append the cell symbol.
6. **Cursor** — `move_to` + show, or hide.
7. **Swap** — `cur` becomes `prev` via `mem::swap`; nothing is reallocated per frame
   beyond the output string.

### 3.2 Incremental SGR (`push_style`)

Style switches are the largest hidden cost of naive TUI renderers. `push_style` keeps the
style of the last emitted cell and produces the *minimum* SGR suffix:

- color changed, or a modifier bit was removed → `\x1b[0m` reset + set fg + set bg + set
  added modifiers (four short sequences, each omitted when unused);
- otherwise → only the newly added modifier bits, e.g. `\x1b[1m`.

Zero-dep consequence: `Color`/`Style` are plain data, so the whole state machine is a pure
function and unit-testable (`ansi.rs` tests assert exact byte output).

### 3.3 Why no clear-screen, ever

Clearing then repainting causes visible flicker on every frame. Diffing against the
previous buffer means a static UI costs zero output bytes: a spinner frame touches a
handful of cells, so the entire frame is one cursor move, one SGR, a few characters.
Resize/height-change frames are the only full repaints (unavoidable, still flicker-free
since cells that keep their value are skipped).

## 4. Inline mode: bottom anchoring

Inline mode renders into the last `height` rows of the visible screen:

```
screen row (0-based)  =  (rows - height) + region_row
```

`region_height_change` handles the three cases when the requested height changes between
frames:

| change | action |
|---|---|
| shrink | the region top moves down; clear the newly exposed rows at the top |
| grow within reserved rows | nothing — the space is already ours |
| grow beyond reserved | park the cursor on the last screen row and emit `\r\n` to scroll new lines in; `reserved` grows |

All height changes also force a full redraw for that frame because every cell's absolute
screen row shifts.

The one-line budget (`1..=rows-1`) keeps a shell prompt row visible after exit; `close()`
moves the cursor to the last row and emits a single `\r\n` so the shell continues below
the UI.

## 5. Unicode width system

`width.rs` embeds a wcwidth-style table: ranges for zero-width (combining marks,
variation selectors, ZWSP…) and two-width (CJK, fullwidth, emoji) code points.

Invariants that keep wide characters safe:

- `Buffer::set_cell` writes the glyph into cell *x* and placeholder continuation cells
  (`symbol == ""`) into *x+1* for double-width glyphs;
- `set_string` **drops** a wide character that would straddle the last column instead of
  splitting it;
- the diff compiler skips placeholder cells (they are not addressable);
- the editor's cursor is a *char index* per line; visual columns are derived through
  `str_width`, so a CJK character moves the cursor by one char but two columns.

Every wrapping function (`wrap_line`, `wrap_plain`, `truncate_line`) counts width, never
`chars().count()`.

## 6. Input pipeline

### 6.1 Threads

`spawn_input_thread()` starts two threads:

- **input reader** — blocking `read` on the TTY fd; bytes are fed to the parser; events go
  into the mpsc channel. EOF or error terminates the thread.
- **size poller** — 300 ms loop comparing `sys::size()`; changes emit `Event::Resize`.

Apps poll with `recv_timeout` — the timeout doubles as the animation tick, so there is no
dedicated render thread and no locking between UI state and painting.

### 6.2 VT parser state machine (`input.rs`)

A single stateful `Parser` consumes a byte stream and survives chunk splits (a 6-byte
mouse sequence can arrive as 2+4 bytes across reads):

```
Normal ──0x1b──▶ Esc ──'['──▶ Csi{params} ──0x40..0x7e──▶ dispatch ──▶ Normal
                   │ 'O' ──▶ Ss3                    │
                   │ ']' ──▶ OscEsc (skip to ESC \) │
                   └ other → Alt+key
Csi: '<' cb;x;y M/m  → SGR mouse          200~ / 201~ → bracketed paste
     1;mod  letter    → modified keys     n; m R      → cursor position report
     n~               → function keys
```

- **ESC-alone heuristic**: a lone `ESC` at the *end* of a read batch is `KeyCode::Esc`
  (Alt-combos arrive in the same batch, so this resolves the classic ESC-vs-Alt race
  without timers).
- **UTF-8 accumulator** carries a partial sequence across feeds and resyncs on invalid
  bytes (emitting U+FFFD) instead of desynchronizing.
- Windows and Unix share this parser: Windows uses `ENABLE_VIRTUAL_TERMINAL_INPUT` so the
  console emits the same VT byte stream a pty would.

## 7. Platform backends (`sys/` — the only `unsafe`)

| concern | Windows | Unix |
|---|---|---|
| raw mode | `SetConsoleMode`: output `\|` `ENABLE_VIRTUAL_TERMINAL_PROCESSING`, input ← `ENABLE_VIRTUAL_TERMINAL_INPUT` (drops line/echo/procsignal) | `tcgetattr` → `cfmakeraw` → `tcsetattr`; termios saved as an opaque `[u64; 12]` buffer (no struct layout assumptions across glibc/musl/Darwin) |
| input read | `ReadFile` on the console input handle | `read(2)` on the TTY fd |
| TTY fallback | `GetConsoleMode` failure ⇒ "terminal unavailable" error | stdin not a TTY ⇒ open `/dev/tty` (IDE pipes keep working) |
| size | `GetConsoleScreenBufferInfo` window rect | `TIOCGWINSZ` on stdout, then the TTY fd |
| crash restore | saved modes restored in `Drop`, panic hook | same, plus SIGHUP/SIGINT/SIGTERM handlers |

The Unix signal handler is async-signal-safe by construction: it calls only `tcsetattr`,
`signal(SIG_DFL)` and `raise(sig)` — it restores the terminal, then re-raises so the
process still dies *of* the signal (observed exit status: `WIFSIGNALED`, term 15).

## 8. Editor internals (`widgets/input.rs`)

- **Text model**: `Vec<String>` (one per logical line); the cursor is
  `(line, char_index)`; edits convert char→byte indices per line (`byte_index_at`).
- **Visual mapping**: `wrap_plain(line, width)` yields `(segment, char_offset)` rows; the
  renderer locates the visual row containing the cursor, scrolls a window over it, and
  converts the char offset to a column via width summation (wide chars land between
  cells, never mid-cell).
- **History**: submitted texts live in `history`; ↑/↓ (single-line state only) walks it,
  saving the in-progress draft. Navigation via `set_text`, which deliberately does **not**
  enter the undo history.
- **Undo/redo** (`Ctrl+Z` / `Ctrl+Y`): snapshot-based — each mutating key press wraps its
  mutation in `begin_edit(kind)` / `end_edit(kind)`:

  - `begin_edit` pushes a full snapshot (`lines`, `cursor`, `scroll_row`) **unless** the
    previous mutation had the same kind and ended exactly where the new one starts — that
    coalescing rule makes a burst of typing or backspacing a single undo step without
    timers;
  - `end_edit` records `(kind, post-edit cursor)` for the next coalescing check;
  - movement keys call `break_run()` so resumed typing starts a fresh step;
  - undo pops a snapshot (pushing the current state to the redo stack), redo mirrors it;
    any new edit clears the redo stack;
  - snapshots are capped (`with_max_history`, default 100). For a chat-sized input the
    per-snapshot cost is a few hundred bytes — snapshots beat delta chains on simplicity
    and robustness (no inverse-operation bookkeeping, trivially correct across line
    splits/merges).
  - `clear()` (used by submit) and `set_text` (used by history navigation) only
    `break_run()`; they are not undo steps.

## 9. Widget conventions

- **State external**: selection/scroll/checked state lives in the app (`ListState`,
  `TableState`, `ScrollState`); widgets borrow it. `State::sync` clamps offsets before
  rendering. This makes widgets trivially testable: render into an in-memory `Buffer` and
  assert on rows.
- **Pure-time animations** (`anim.rs`): every animated widget takes `.at(elapsed)` and is
  a pure function of that instant — determinism makes animation tests plain assertions,
  and the tick comes free from `recv_timeout`.
- **Consuming render**: `Widget::render(self, area, buf)`; builders return owned values,
  cheap to clone (`Style`, `Block` are `Copy`/small).

## 10. Content layer

- **markdown.rs** — block parser (headings, fenced code with rounded borders + language
  label, lists with hanging indent, quotes with `▌` bar, rules, paragraphs) + inline
  parser (bold/italic/strike/code/links/escapes) producing `Text`. Unterminated fences
  render as code blocks with an ellipsis footer, so a partially-streamed reply is laid out
  stably. The accumulated text is re-parsed each frame — bounded by chat-scale inputs and
  dwarfed by I/O cost; incremental parsing is a non-goal until evidence demands it.
- **highlight.rs** — hand-written tokenizer (no regex): comments (line + block, state
  carried across lines), strings with escapes, numbers, keywords (tables for 10
  languages), capitalized identifiers as types, `name(` as functions. JSON gets key
  coloring via a `LangCfg` flag.

## 11. Testing strategy

Three tiers, all runnable headless:

1. **Unit** (~100 tests) — exact-byte ANSI output; width fixtures (CJK/emoji/combining
   chars are feature data, kept verbatim in tests); wrap/roundtrip; VT parser including
   chunk-split sequences; editor operations and undo grouping; widget snapshots rendered
   into memory buffers.
2. **Integration** — a full-frame pipeline simulation (history + editor + status bar in
   one layout) asserting cursor bounds and no-split invariants.
3. **PTY smoke** (`tools/pty_smoke.py`) — forks the example binaries under a real pty
   with a set window size, feeds scripted keystrokes on a timeline, captures the ANSI
   stream and asserts on rendered content, cursor restore, absence of alt-screen in
   inline mode, and (the `signal` scenario) that SIGTERM restores ICANON/ECHO/ISIG. A
   driver subtlety: on pty hangup EIO can arrive before the child is fully reaped, so the
   reaper retries for a few seconds before declaring a timeout.

Verification matrix: Windows (primary), WSL2 Ubuntu 26.04 + kali (runtime), cross-compile
checks for `x86_64-unknown-linux-gnu` and `aarch64-apple-darwin`.

## 12. Known limitations

- resize relies on polling (no SIGWINCH handler); latency ≤ 300 ms
- markdown tables render as paragraphs; no nested bold-italic
- no selection/clipboard inside the editor; no single-line mode
- inline mode assumes a VT-capable terminal (Win10+ / any modern Linux/macOS terminal)
