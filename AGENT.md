# AGENT.md — Guidelines for AI Coding Agents

This file gives AI coding agents (Claude Code / ZCode / Codex / etc.) working in this repo
the context and hard rules they need. Read it before changing any code.

## What this project is

**cctui**: a general-purpose Rust TUI **widget library** (component model in the spirit of
ratatui/FTXUI, zero dependencies). It is not a chat app and not a clone of any specific
product — `examples/chat.rs` is merely a composition demo. Signature features: inline
rendering (the UI occupies the bottom rows of the terminal while scrollback stays intact)
and double-buffered differential rendering, plus a built-in wcwidth system.

## Hard rules (violations get reverted)

1. **Zero third-party dependencies.** `[dependencies]` in Cargo.toml must stay empty. Need
   FFI? Hand-write the `extern` declarations in `src/sys/` (std already links
   kernel32/libc). Need bitflags? Use the existing hand-rolled bitflag pattern (see
   `Modifier` in `style.rs`).
2. **`unsafe` is only allowed inside `src/sys/`.** Everything else must be safe Rust.
3. **Edition 2021.** No let-chains (`if x && let Some(y) = ...`), no edition-2024-only
   syntax, no std APIs newer than 1.70 (`rust-version = "1.70"`).
4. **Comments and doc comments in English.** Identifiers in English. Match the existing
   style.
5. **Widget-library positioning.** The library must not absorb application semantics (no
   user/assistant roles, no chat-specific styles). Application-level assembly belongs in
   examples. `Theme` carries generic semantic roles only (border/text/dim/accent/keyword/
   diff_* etc.).

## Build & verification commands

Windows side (Git Bash; cargo is not on the default PATH):

```bash
export PATH="/c/Users/Ash/.cargo/bin:$PATH"
cargo test                              # 107 tests must be green
cargo clippy --all-targets              # 0 warnings is the bar
cargo check --target x86_64-unknown-linux-gnu --all-targets   # required after touching sys/unix.rs
cargo check --target aarch64-apple-darwin --all-targets       # same
cargo run --example gallery             # widget tour (needs a real terminal)
```

Linux side (local WSL2: both **Ubuntu-26.04** and kali distros are verified; Rust is
installed offline in each distro's `~/.rust-linux` — rustup direct downloads time out, so
use the dist tarball downloaded on the Windows side):

```bash
MSYS_NO_PATHCONV=1 wsl.exe -d Ubuntu-26.04 -- bash /mnt/d/Dev_Project/C/.wsl-test.sh     # full: sync → cargo test → PTY scenarios
MSYS_NO_PATHCONV=1 wsl.exe -d Ubuntu-26.04 -- bash /mnt/d/Dev_Project/C/.wsl-test.sh signal   # SIGTERM restore scenario only
```

**Gotchas**: Git Bash rewrites `/mnt/...` into Windows paths — always pass
`MSYS_NO_PATHCONV=1` when invoking wsl.exe. Multi-line inline shell scripts get mangled by
quote layers; write complex logic to a script file and execute that instead.

After touching rendering/input/platform code, run at minimum `cargo test` + `clippy` +
`cargo check` for the relevant target; after touching `sys/unix.rs` or example interaction
flows, also run `.wsl-test.sh`.

## Architecture map

```
src/
├── style.rs      Color(16/256/RGB)·Modifier·Style (patch semantics); hand-rolled bitflags
├── width.rs      wcwidth table: char_width/str_width (CJK/fullwidth/emoji=2, combining=0)
├── text.rs       Span/Line/Text; wrap_line (styled word wrap), wrap_plain (char offsets, editor)
├── buffer.rs     Cell/Buffer: set_string stops at bounds, wide chars write placeholder
│                 cells (symbol=""), diff compares cell-by-cell
├── ansi.rs       escape primitives + push_style (incremental SGR: color change/bit removal
│                 → reset + full restyle, otherwise only append added bits)
├── sys/          platform layer (the only place allowed to use unsafe)
│   ├── windows.rs  kernel32 FFI: VT on output, pure VT byte input (ENABLE_VIRTUAL_TERMINAL_INPUT),
│   │               ReadFile for raw bytes
│   └── unix.rs     libc FFI: termios saved/restored opaquely as [u64;12] (cfmakeraw),
│                   /dev/tty fallback, SIGHUP/SIGINT/SIGTERM handlers restoring termios,
│                   global INPUT_FD
├── input.rs      stateful VT byte-stream parser: CSI/SS3/SGR mouse/bracketed paste/focus,
│                 state survives across feed() calls
├── event.rs      Event/KeyCode/KeyEvent/MouseEvent model
├── layout.rs     Rect + vsplit/hsplit constraint solving (solve is pub(crate); Table reuses it)
├── terminal.rs   Terminal(inline/fullscreen): draw(height, f) per frame; compile_frame diff
│                 compiler; region_height_change handles bottom-anchored height changes;
│                 panic hook as last-resort restore
├── app.rs        spawn_input_thread(): input reader thread + size poller (300ms) → mpsc<Event>
├── markdown.rs   block+inline parsing → Text; unterminated fences render as code blocks
│                 (critical for streaming)
├── highlight.rs  hand-written tokenizer (at() prefix matching, block comments span lines);
│                 keyword tables for 10 languages
├── theme.rs      semantic roles → Style (generic roles only, no app semantics)
└── widgets/      Widget trait (render(self, area, buf)); all render-only with external state
    ├── anim.rs   animations (Wave/Shimmer/ProgressBar/LoadingDots/Typewriter/Ticker +
    │             lerp_rgb/pulse_color/wave_spans); pure functions of time, driven by .at(elapsed)
    ├── block.rs / paragraph.rs / viewport.rs / menu.rs / spinner.rs / statusbar.rs
    ├── list.rs   List + ListState (offset/selected, sync clamps visibility)
    ├── table.rs  Table + TableState (constraint column widths, reuses layout::solve)
    ├── tabs.rs   tab bar
    ├── checkbox.rs Checkbox / RadioGroup (render-only, state owned by the app)
    ├── scrollbar.rs Scrollbar (render_with_metrics needs content metrics)
    ├── sparkline.rs block-character bar chart (multi-row, stacked from the bottom)
    ├── collapsible.rs / diff.rs (diff_lines reused by line-based viewports)
    └── input.rs  multi-line editor (Vec<String> per line, cursor=(line, char index),
                  char→byte conversion for edits)
```

Data flow: input thread → (VT parsing) → `Event` channel → main loop `recv_timeout`
(timeout = tick) → widgets paint into `Buffer` → diff vs. previous frame → minimal ANSI.

**Widget conventions** (mandatory for new widgets):
- External state: interaction state (selection/scroll/checked) is owned by the app; widgets
  borrow it for rendering. `State::sync` clamps offsets before rendering (List/Table).
- Animations are pure functions of time: `.at(Duration)` pins the moment; same input, same
  output (testable).
- Builder-style configuration; consuming `render(self, area, buf)`.
- **Editor mutations must be undoable**: every new mutating branch in `Editor::handle_key`
  wraps its mutation in `begin_edit(kind)` / `end_edit(kind)` (grouping: `InsertChar` /
  `DeleteBack` coalesce consecutive same-kind edits at the running cursor; `Other` never
  coalesces), and cursor-movement branches call `break_run()` first. Pure mutations
  (`insert_char`, `backspace`, …) must not record history themselves.

## Key invariants (read before touching the render path)

1. **All visible-width math must go through `width::str_width`/`char_width`** — never use
   `chars().count()` as width. CJK users are a first-class audience.
2. **`Cell.symbol == ""` means a wide-char continuation cell**: diff/compile_frame must skip
   them; `set_string` drops a wide char entirely when it doesn't fit in the last column
   (never split in half).
3. **Inline regions are bottom-anchored**: region row r maps to screen row
   `rows - height + r` (0-based); any height change forces a full redraw; shrinking clears
   the exposed top rows; growth beyond the reserved space scrolls in new lines first.
4. **Always emit `\r\n`** (raw mode turns off OPOST/ONLCR; a bare `\n` won't return the
   carriage).
5. **Terminal state must always be restorable**: sys layers save the original state with
   triple protection — `Drop` + panic hook + (Unix) signal handlers. After changing `sys/`,
   run the `pty_smoke.py signal` scenario.
6. **Never emit clear-screen sequences** (flicker-free means writing only changed cells);
   fullscreen mode only switches to the alternate screen.
7. **`push_style` semantics**: on color change or modifier-bit removal → `\x1b[0m` + full
   restyle; otherwise only append newly added modifier bits.
8. **The editor's Enter has dual semantics**: with the completion menu open, Enter accepts;
   otherwise it submits. When adding key handling, think through priority vs. menu/history.

## Common pitfalls

- **Don't include borders in Buffer test assertions**: read text starting at
  `block.inner(area)`, otherwise the right border `│` pollutes the row string and
  `ends_with`-style asserts fail (`trim_end` doesn't strip borders).
- **`Terminal::draw` rebuilds `self.cur` every frame**; prev comes from `mem::swap` — never
  cache Buffer references elsewhere.
- **The input thread blocks in `read`**: the main loop exits via process exit; don't expect
  to join it.
- **Markdown re-parses the accumulated text every frame** — intentional (streaming
  simplicity and correctness). Don't add incremental caching without perf evidence.
- **JSON key coloring in highlight** relies on the `cfg.json` flag; add a `LangCfg` entry
  for new languages.
- **Span concatenation and ownership**: use `l.spans.extend(row.spans.clone())`; never move
  fields out of borrowed values.
- **`frame.buffer` is `&mut` in examples**: binding it to a local moves it and breaks later
  uses — call `frame.buffer.xxx` directly.
- **PTY assertions on editor echo**: per-character input renders one frame per character, so
  the ANSI stream is not contiguous — assert per character (`all(c in out ...)`), never
  `b"hello" in out`.
- **PTY child-exit race**: EIO (pty hangup) can arrive before the child fully terminates
  (multi-thread teardown timing; slower on Ubuntu than kali) — reap with a blocking retry
  for a few seconds before declaring a timeout, or a clean exit gets misjudged as a hang.
- **Never hand-decode UTF-8 from `cat -v` output**: multi-byte sequences get misread and
  produce phantom FFFDs — use python byte-level checks (`data.decode('utf-8')`) to detect
  lost bytes.
- **Paint the Block before its content in example layouts** (content goes in
  `block.inner`), otherwise the border covers the content.
- **Match ergonomics on key events**: after `match k.code { ... other => ... }` rebuild the
  event — `editor.handle_key(KeyEvent::new(other, k.modifiers))`.

## Testing conventions

- Unit tests live in each module's `#[cfg(test)]`; widget tests render into an in-memory
  `Buffer` and assert on row text/colors (see `widgets/table.rs::tests`).
- Integration tests live in `tests/integration.rs` (including a full-frame pipeline sim).
- Headless PTY acceptance: `tools/pty_smoke.py` (5 scenarios: sandbox / gallery / chat /
  diff / signal). New user-visible interaction → add a scenario assertion.
- Test names and assertion messages in English.
- **Keep CJK test fixtures Chinese**: strings in width/text/buffer/table/input tests that
  exist to exercise wide-char behavior are feature data, not documentation — do not
  translate them.

## Definition of done

A change is done when and only when:

1. `cargo test` is fully green (currently 107 tests)
2. `cargo clippy --all-targets` reports 0 warnings
3. For platform/render changes: both cross-compile targets pass `cargo check`; for Unix
   path changes: `.wsl-test.sh` reports ALL PASS
4. New user-visible behavior has test coverage, and README is updated when needed
