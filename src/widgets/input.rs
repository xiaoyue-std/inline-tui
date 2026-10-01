//! Multi-line input editor: the core input widget for modern CLI applications.
//!
//! - Cursor movement: char-level, word-level (Ctrl+←/→, Alt+B/F), line start/end
//!   (Home/End, Ctrl+A/E)
//! - Editing: backspace/delete, Ctrl+K/U (delete to end/start of line),
//!   Ctrl+W/Alt+Backspace (delete word), Alt+Enter (newline)
//! - Selection: Shift+arrows/Home/End to select (rendered reversed); typing
//!   replaces it; Backspace/Delete/Ctrl+W delete it; paste replaces it
//! - Undo/redo: Ctrl+Z / Ctrl+Y (snapshot history; consecutive typing or
//!   backspacing groups into a single undo step)
//! - History: browse submitted history with ↑/↓ in single-line state (with draft saving)
//! - Slash command completion: typing `/` opens a menu, ↑/↓ to select,
//!   Enter/Tab to accept
//! - Placeholder, automatic wrapping with in-line scrolling, wide-char cursor positioning

use crate::buffer::Buffer;
use crate::event::{KeyCode, KeyModifiers, KeyEvent};
use crate::layout::Rect;
use crate::style::{Modifier, Style};
use crate::text::wrap_plain;
use crate::widgets::menu::menu_height;
use crate::widgets::{Block, Widget};
use crate::width::str_width;

/// The result of handling a key in the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputAction {
    /// No change.
    None,
    /// The content or menu changed; a redraw is needed.
    Edited,
    /// The user pressed Enter, submitting the current text.
    Submitted(String),
}

#[derive(Debug, Clone)]
struct MenuState {
    /// Indices into the completion list.
    indices: Vec<usize>,
    selected: usize,
}

/// Kind of a mutating edit; drives undo grouping (run coalescing).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditKind {
    /// Single character insertion: consecutive inserts at the running cursor
    /// position merge into one undo step.
    InsertChar,
    /// Backspace: consecutive backspaces merge into one undo step.
    DeleteBack,
    /// Any other mutation (never coalesced).
    Other,
}

/// One undo step: a full snapshot of text, cursor and scroll.
#[derive(Debug, Clone)]
struct UndoEntry {
    lines: Vec<String>,
    cursor: (usize, usize),
    scroll_row: usize,
}

/// Multi-line input editor.
#[derive(Debug, Clone)]
pub struct Editor {
    lines: Vec<String>,
    /// (logical line index, char index within the line).
    cursor: (usize, usize),
    scroll_row: usize,
    /// Number of visual rows from the last render (used for dynamic height).
    visible_rows: usize,
    placeholder: String,
    placeholder_style: Style,
    text_style: Style,
    block: Block,
    history: Vec<String>,
    history_pos: Option<usize>,
    draft: Option<String>,
    completions: Vec<String>,
    menu: Option<MenuState>,
    max_menu_visible: usize,
    /// Maximum number of logical lines the input box grows to.
    max_grow_lines: usize,
    /// Selection anchor; the selected range spans anchor..cursor.
    selection: Option<(usize, usize)>,
    /// Style applied to selected characters.
    selection_style: Style,
    /// Single-line mode: Enter/Alt+Enter submit, newlines from paste become
    /// spaces, multiline `set_text` input is squashed.
    single_line: bool,
    /// Undo snapshots (most recent last), capped at `max_history`.
    undo_stack: Vec<UndoEntry>,
    redo_stack: Vec<UndoEntry>,
    max_history: usize,
    /// (kind, post-edit cursor) of the previous mutation, for run coalescing.
    last_edit: Option<(EditKind, usize, usize)>,
}

impl Default for Editor {
    fn default() -> Editor {
        Editor::new()
    }
}

impl Editor {
    pub fn new() -> Editor {
        Editor {
            lines: vec![String::new()],
            cursor: (0, 0),
            scroll_row: 0,
            visible_rows: 1,
            placeholder: String::new(),
            placeholder_style: Style::new(),
            text_style: Style::new(),
            block: Block::rounded(),
            history: Vec::new(),
            history_pos: None,
            draft: None,
            completions: Vec::new(),
            menu: None,
            max_menu_visible: 6,
            max_grow_lines: 5,
            selection: None,
            selection_style: Style::new().add_modifier(Modifier::REVERSED),
            single_line: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_history: 100,
            last_edit: None,
        }
    }

    // —— Configuration ——

    pub fn with_placeholder(mut self, p: impl Into<String>) -> Editor {
        self.placeholder = p.into();
        self
    }

    pub fn with_completions(mut self, commands: Vec<String>) -> Editor {
        self.completions = commands;
        self
    }

    pub fn with_block(mut self, b: Block) -> Editor {
        self.block = b;
        self
    }

    /// Replaces the outer frame every frame (for dynamic border styles / title animation).
    pub fn set_block(&mut self, b: Block) {
        self.block = b;
    }

    /// Replaces the selection style at runtime (e.g. when the theme changes).
    pub fn set_selection_style(&mut self, st: Style) {
        self.selection_style = st;
    }

    pub fn placeholder_style(mut self, st: Style) -> Editor {
        self.placeholder_style = st;
        self
    }

    pub fn text_style(mut self, st: Style) -> Editor {
        self.text_style = st;
        self
    }

    pub fn max_grow_lines(mut self, n: usize) -> Editor {
        self.max_grow_lines = n.max(1);
        self
    }

    /// Maximum number of undo steps kept (default 100).
    pub fn with_max_history(mut self, n: usize) -> Editor {
        self.max_history = n.max(1);
        self.undo_stack.truncate(self.max_history);
        self
    }

    /// Style applied to selected characters (default: reversed).
    pub fn selection_style(mut self, st: Style) -> Editor {
        self.selection_style = st;
        self
    }

    /// Enables single-line mode: Enter/Alt+Enter submit, pasted newlines become
    /// spaces, and multiline text set via [`Editor::set_text`] is squashed.
    pub fn single_line(mut self, on: bool) -> Editor {
        self.single_line = on;
        self
    }

    pub fn set_single_line(&mut self, on: bool) {
        self.single_line = on;
    }

    // —— State queries ——

    /// Current text (multiple lines joined with `\n`).
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn is_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines[0].is_empty()
    }

    /// Cursor logical position (line, char index).
    pub fn cursor_pos(&self) -> (usize, usize) {
        self.cursor
    }

    /// Suggested input box height (borders included): grows with content, capped at `max_grow_lines`.
    pub fn desired_height(&self) -> u16 {
        self.visible_rows.clamp(1, self.max_grow_lines) as u16 + 2
    }

    /// The currently open completion menu: (item texts, selected index).
    pub fn menu_state(&self) -> Option<(Vec<&str>, usize)> {
        self.menu.as_ref().map(|m| {
            (
                m.indices.iter().map(|&i| self.completions[i].as_str()).collect(),
                m.selected,
            )
        })
    }

    /// Height occupied by the completion menu (0 when closed).
    pub fn menu_height(&self) -> u16 {
        self.menu
            .as_ref()
            .map(|m| menu_height(m.indices.len(), self.max_menu_visible))
            .unwrap_or(0)
    }

    // —— Undo / redo (public API) ——

    /// Undoes the most recent edit group. Returns `true` if state changed.
    pub fn undo(&mut self) -> bool {
        let Some(entry) = self.undo_stack.pop() else {
            return false;
        };
        self.redo_stack.push(self.snapshot());
        self.apply_entry(entry);
        true
    }

    /// Redoes the most recently undone edit group. Returns `true` if state changed.
    pub fn redo(&mut self) -> bool {
        let Some(entry) = self.redo_stack.pop() else {
            return false;
        };
        self.undo_stack.push(self.snapshot());
        self.apply_entry(entry);
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    // —— Selection (public API) ——

    /// Whether a non-empty selection exists.
    pub fn has_selection(&self) -> bool {
        matches!(self.ordered_selection(), Some((s, e)) if s != e)
    }

    /// The ordered selection range: `((start line, start char), (end line, end char))`.
    pub fn selection_range(&self) -> Option<((usize, usize), (usize, usize))> {
        self.ordered_selection().filter(|(s, e)| s != e)
    }

    /// Clears the selection without touching the text.
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    fn ordered_selection(&self) -> Option<((usize, usize), (usize, usize))> {
        let anchor = self.selection?;
        let c = self.cursor;
        Some(if (anchor.0, anchor.1) <= (c.0, c.1) {
            (anchor, c)
        } else {
            (c, anchor)
        })
    }

    fn snapshot(&self) -> UndoEntry {
        UndoEntry {
            lines: self.lines.clone(),
            cursor: self.cursor,
            scroll_row: self.scroll_row,
        }
    }

    fn apply_entry(&mut self, e: UndoEntry) {
        self.lines = e.lines;
        self.cursor = e.cursor;
        self.scroll_row = e.scroll_row;
        self.last_edit = None;
        self.after_edit();
    }

    /// Opens an undo group before a mutation. Called once per mutating key press;
    /// skips the snapshot when the mutation continues the previous run (same kind,
    /// cursor exactly adjacent), so consecutive typing/backspacing is one undo step.
    fn begin_edit(&mut self, kind: EditKind) {
        let (li, ci) = self.cursor;
        let merge = matches!(self.last_edit, Some((k, pl, pc)) if k == kind && pl == li && pc == ci);
        if !merge {
            self.undo_stack.push(self.snapshot());
            if self.undo_stack.len() > self.max_history {
                self.undo_stack.remove(0);
            }
            self.redo_stack.clear();
        }
    }

    /// Closes the group opened by [`Editor::begin_edit`]; records the post-edit
    /// cursor so the next same-kind mutation can coalesce.
    fn end_edit(&mut self, kind: EditKind) {
        self.last_edit = Some((kind, self.cursor.0, self.cursor.1));
    }

    /// Breaks the current run so the next mutation starts a fresh undo step.
    fn break_run(&mut self) {
        self.last_edit = None;
    }

    /// Deletes the selected range (if any) and moves the cursor to its start.
    /// Returns `true` when a non-empty selection was removed.
    fn delete_selection(&mut self) -> bool {
        let Some(((sl, sc), (el, ec))) = self.ordered_selection().filter(|(s, e)| s != e) else {
            self.selection = None;
            return false;
        };
        if sl == el {
            let sb = byte_index_at(&self.lines[sl], sc);
            let eb = byte_index_at(&self.lines[sl], ec);
            self.lines[sl].replace_range(sb..eb, "");
        } else {
            let head = self.lines[sl][..byte_index_at(&self.lines[sl], sc)].to_string();
            let tail = self.lines[el][byte_index_at(&self.lines[el], ec)..].to_string();
            self.lines[sl] = format!("{head}{tail}");
            self.lines.drain(sl + 1..=el);
        }
        self.cursor = (sl, sc);
        self.selection = None;
        true
    }

    // —— Text operations (public API) ——

    /// Overwrites all text and moves the cursor to the end.
    ///
    /// Not recorded in undo history (used for history navigation and
    /// programmatic setup); it only breaks the current undo run.
    pub fn set_text(&mut self, s: &str) {
        let squashed;
        let s = if self.single_line {
            squashed = s.replace(['\r', '\n'], " ");
            squashed.as_str()
        } else {
            s
        };
        self.lines = s.split('\n').map(str::to_string).collect();
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        let li = self.lines.len() - 1;
        self.cursor = (li, self.lines[li].chars().count());
        self.selection = None;
        self.break_run();
        self.after_edit();
    }

    /// Clears the input.
    ///
    /// Not recorded in undo history (submit uses it); it only breaks the
    /// current undo run.
    pub fn clear(&mut self) {
        self.lines = vec![String::new()];
        self.cursor = (0, 0);
        self.scroll_row = 0;
        self.history_pos = None;
        self.menu = None;
        self.selection = None;
        self.break_run();
        self.after_edit();
    }

    /// Inserts pasted text (`\r\n` normalized, newlines split into lines).
    ///
    /// Replaces the selection when one exists. Recorded as a single undo step.
    pub fn handle_paste(&mut self, text: &str) {
        self.begin_edit(EditKind::Other);
        self.delete_selection();
        for ch in text.chars() {
            match ch {
                '\r' => {}
                '\n' => {
                    if self.single_line {
                        self.insert_char(' ');
                    } else {
                        self.insert_newline();
                    }
                }
                '\t' => {
                    self.insert_char(' ');
                    self.insert_char(' ');
                    self.insert_char(' ');
                    self.insert_char(' ');
                }
                c => self.insert_char(c),
            }
        }
        self.end_edit(EditKind::Other);
        self.after_edit();
    }

    // —— Key handling ——

    /// Handles a single key event.
    pub fn handle_key(&mut self, key: KeyEvent) -> InputAction {
        let m = key.modifiers;
        match key.code {
            KeyCode::Enter if m.is_empty() => self.submit(),
            KeyCode::Enter if m == KeyModifiers::ALT => {
                if self.single_line {
                    self.submit()
                } else {
                    self.begin_edit(EditKind::Other);
                    self.insert_newline();
                    self.end_edit(EditKind::Other);
                    self.after_edit();
                    InputAction::Edited
                }
            }
            KeyCode::Tab if self.menu.is_some() => {
                self.begin_edit(EditKind::Other);
                self.accept_menu();
                self.end_edit(EditKind::Other);
                self.after_edit();
                InputAction::Edited
            }
            KeyCode::Esc => {
                if self.menu.take().is_some() {
                    InputAction::Edited
                } else {
                    InputAction::None
                }
            }
            KeyCode::Up => {
                if self.menu.is_some() {
                    self.menu_up();
                    InputAction::Edited
                } else if m.contains(KeyModifiers::SHIFT) {
                    self.break_run();
                    self.selection.get_or_insert(self.cursor);
                    self.move_cursor_line(-1);
                    InputAction::Edited
                } else {
                    self.break_run();
                    self.selection = None;
                    if self.lines.len() == 1 && self.history_up() {
                        InputAction::Edited
                    } else {
                        self.move_cursor_line(-1);
                        InputAction::Edited
                    }
                }
            }
            KeyCode::Down => {
                if self.menu.is_some() {
                    self.menu_down();
                    InputAction::Edited
                } else if m.contains(KeyModifiers::SHIFT) {
                    self.break_run();
                    self.selection.get_or_insert(self.cursor);
                    self.move_cursor_line(1);
                    InputAction::Edited
                } else {
                    self.break_run();
                    self.selection = None;
                    if self.lines.len() == 1 && self.history_down() {
                        InputAction::Edited
                    } else {
                        self.move_cursor_line(1);
                        InputAction::Edited
                    }
                }
            }
            KeyCode::Left => {
                self.break_run();
                if m.contains(KeyModifiers::SHIFT) {
                    self.selection.get_or_insert(self.cursor);
                } else {
                    self.selection = None;
                }
                if m.intersects(KeyModifiers::CONTROL) || m == KeyModifiers::ALT {
                    self.word_left();
                } else {
                    self.char_left();
                }
                InputAction::Edited
            }
            KeyCode::Right => {
                self.break_run();
                if m.contains(KeyModifiers::SHIFT) {
                    self.selection.get_or_insert(self.cursor);
                } else {
                    self.selection = None;
                }
                if m.intersects(KeyModifiers::CONTROL) || m == KeyModifiers::ALT {
                    self.word_right();
                } else {
                    self.char_right();
                }
                InputAction::Edited
            }
            KeyCode::Home => {
                self.break_run();
                if m.contains(KeyModifiers::SHIFT) {
                    self.selection.get_or_insert(self.cursor);
                } else {
                    self.selection = None;
                }
                self.cursor.1 = 0;
                InputAction::Edited
            }
            KeyCode::End => {
                self.break_run();
                if m.contains(KeyModifiers::SHIFT) {
                    self.selection.get_or_insert(self.cursor);
                } else {
                    self.selection = None;
                }
                self.cursor.1 = self.lines[self.cursor.0].chars().count();
                InputAction::Edited
            }
            KeyCode::Backspace if m == KeyModifiers::ALT => {
                self.begin_edit(EditKind::Other);
                self.delete_word_back();
                self.end_edit(EditKind::Other);
                self.after_edit();
                InputAction::Edited
            }
            KeyCode::Backspace if m.is_empty() => {
                self.break_run();
                if self.has_selection() {
                    self.begin_edit(EditKind::Other);
                    self.delete_selection();
                    self.end_edit(EditKind::Other);
                } else {
                    self.begin_edit(EditKind::DeleteBack);
                    self.backspace();
                    self.end_edit(EditKind::DeleteBack);
                }
                self.after_edit();
                InputAction::Edited
            }
            KeyCode::Delete if m.is_empty() => {
                self.break_run();
                self.begin_edit(EditKind::Other);
                if !self.delete_selection() {
                    self.delete_forward();
                }
                self.end_edit(EditKind::Other);
                self.after_edit();
                InputAction::Edited
            }
            KeyCode::Char(c) if m == KeyModifiers::CONTROL => match c {
                'a' => {
                    self.break_run();
                    self.cursor.1 = 0;
                    InputAction::Edited
                }
                'e' => {
                    self.break_run();
                    self.cursor.1 = self.lines[self.cursor.0].chars().count();
                    InputAction::Edited
                }
                'z' => {
                    if self.undo() {
                        InputAction::Edited
                    } else {
                        InputAction::None
                    }
                }
                'y' => {
                    if self.redo() {
                        InputAction::Edited
                    } else {
                        InputAction::None
                    }
                }
                'k' => {
                    self.begin_edit(EditKind::Other);
                    let (li, ci) = self.cursor;
                    let byte = byte_index_at(&self.lines[li], ci);
                    self.lines[li].truncate(byte);
                    self.end_edit(EditKind::Other);
                    self.after_edit();
                    InputAction::Edited
                }
                'u' => {
                    self.begin_edit(EditKind::Other);
                    let (li, ci) = self.cursor;
                    let byte = byte_index_at(&self.lines[li], ci);
                    self.lines[li].replace_range(..byte, "");
                    self.cursor.1 = 0;
                    self.end_edit(EditKind::Other);
                    self.after_edit();
                    InputAction::Edited
                }
                'w' => {
                    self.break_run();
                    self.begin_edit(EditKind::Other);
                    if !self.delete_selection() {
                        self.delete_word_back();
                    }
                    self.end_edit(EditKind::Other);
                    self.after_edit();
                    InputAction::Edited
                }
                _ => InputAction::None,
            },
            KeyCode::Char(c) if m == KeyModifiers::ALT => match c {
                'b' => {
                    self.break_run();
                    self.word_left();
                    InputAction::Edited
                }
                'f' => {
                    self.break_run();
                    self.word_right();
                    InputAction::Edited
                }
                _ => InputAction::None,
            },
            KeyCode::Char(c) if m.is_empty() || m == KeyModifiers::SHIFT => {
                self.break_run();
                if self.has_selection() {
                    // Typing replaces the selection (one undo step for the replace).
                    self.begin_edit(EditKind::Other);
                    self.delete_selection();
                    self.insert_char(c);
                    self.end_edit(EditKind::Other);
                } else {
                    self.begin_edit(EditKind::InsertChar);
                    self.insert_char(c);
                    self.end_edit(EditKind::InsertChar);
                }
                self.after_edit();
                InputAction::Edited
            }
            _ => InputAction::None,
        }
    }

    /// Handles Enter: accepts an open completion menu, otherwise submits the
    /// text (pushes it to history and clears the input).
    fn submit(&mut self) -> InputAction {
        if self.menu.is_some() {
            self.begin_edit(EditKind::Other);
            self.accept_menu();
            self.end_edit(EditKind::Other);
            self.after_edit();
            return InputAction::Edited;
        }
        let t = self.text();
        self.push_history(&t);
        self.clear();
        InputAction::Submitted(t)
    }

    // —— Rendering ——

    /// Renders the editor and returns the screen coordinates where the cursor should be
    /// placed (relative to the buffer).
    pub fn render_editor(&mut self, area: Rect, buf: &mut Buffer) -> Option<(u16, u16)> {
        self.block.clone().render(area, buf);
        let inner = self.block.inner(area);
        if inner.is_empty() {
            self.visible_rows = 1;
            return None;
        }

        // Build visual rows: (text, logical line, starting char index)
        let mut rows: Vec<(String, usize, usize)> = Vec::new();
        for (li, line) in self.lines.iter().enumerate() {
            for (seg, start) in wrap_plain(line, inner.width as usize) {
                rows.push((seg, li, start));
            }
        }
        if rows.is_empty() {
            rows.push((String::new(), 0, 0));
        }

        // Locate the visual row containing the cursor
        let cur_row = rows
            .iter()
            .position(|(seg, li, st)| {
                *li == self.cursor.0
                    && *st <= self.cursor.1
                    && self.cursor.1 < *st + seg.chars().count()
            })
            .or_else(|| rows.iter().rposition(|(_, li, _)| *li == self.cursor.0))
            .unwrap_or(0);

        // Scroll: keep the cursor row visible
        let vis = inner.height as usize;
        self.visible_rows = rows.len();
        if vis == 0 {
            return None;
        }
        if cur_row < self.scroll_row {
            self.scroll_row = cur_row;
        }
        if cur_row >= self.scroll_row + vis {
            self.scroll_row = cur_row + 1 - vis;
        }
        if self.scroll_row >= rows.len() {
            self.scroll_row = rows.len().saturating_sub(1);
        }

        let empty = self.is_empty();
        let sel = self.ordered_selection().filter(|(s, e)| s != e);
        for (i, (seg, li, st)) in rows.iter().enumerate().skip(self.scroll_row).take(vis) {
            let y = inner.y + (i - self.scroll_row) as u16;
            if empty && *li == 0 && *st == 0 && i == 0 {
                if !self.placeholder.is_empty() {
                    buf.set_string(inner.x, y, &self.placeholder, self.placeholder_style);
                }
            } else {
                // Render the row as styled runs: selected chars get selection_style.
                let mut line = crate::text::Line::empty();
                let mut run = String::new();
                let mut run_sel = false;
                let mut run_started = false;
                for (k, ch) in seg.chars().enumerate() {
                    let ci = *st + k;
                    let in_sel = sel
                        .map(|((sl, sc), (el, ec))| {
                            (li > &sl || (li == &sl && ci >= sc))
                                && (li < &el || (li == &el && ci < ec))
                        })
                        .unwrap_or(false);
                    if run_started && in_sel != run_sel {
                        let st_run = if run_sel { self.selection_style } else { self.text_style };
                        line.spans
                            .push(crate::text::Span::styled(std::mem::take(&mut run), st_run));
                    }
                    run_started = true;
                    run_sel = in_sel;
                    run.push(ch);
                }
                if !run.is_empty() {
                    let st_run = if run_sel { self.selection_style } else { self.text_style };
                    line.spans.push(crate::text::Span::styled(run, st_run));
                }
                buf.set_line(inner.x, y, &line);
            }
        }

        // Cursor screen coordinates
        let (seg, _, st) = &rows[cur_row];
        let off = self.cursor.1.saturating_sub(*st);
        let col = str_width(&seg.chars().take(off).collect::<String>()) as u16;
        let row_y = inner.y + (cur_row - self.scroll_row) as u16;
        Some((inner.x + col, row_y))
    }

    // —— Internals ——

    fn after_edit(&mut self) {
        // Clamp the cursor
        let (li, ci) = self.cursor;
        let li = li.min(self.lines.len() - 1);
        let len = self.lines[li].chars().count();
        self.cursor = (li, ci.min(len));
        self.update_menu();
    }

    fn insert_char(&mut self, c: char) {
        let (li, ci) = self.cursor;
        let byte = byte_index_at(&self.lines[li], ci);
        self.lines[li].insert(byte, c);
        self.cursor.1 += 1;
    }

    fn insert_newline(&mut self) {
        if self.single_line {
            return; // suppressed in single-line mode
        }
        let (li, ci) = self.cursor;
        let byte = byte_index_at(&self.lines[li], ci);
        let tail = self.lines[li].split_off(byte);
        self.lines.insert(li + 1, tail);
        self.cursor = (li + 1, 0);
    }

    fn backspace(&mut self) {
        let (li, ci) = self.cursor;
        if ci > 0 {
            let start = byte_index_at(&self.lines[li], ci - 1);
            let end = byte_index_at(&self.lines[li], ci);
            self.lines[li].replace_range(start..end, "");
            self.cursor.1 -= 1;
        } else if li > 0 {
            let prev_len = self.lines[li - 1].chars().count();
            let cur = self.lines.remove(li);
            self.lines[li - 1].push_str(&cur);
            self.cursor = (li - 1, prev_len);
        }
    }

    fn delete_forward(&mut self) {
        let (li, ci) = self.cursor;
        let len = self.lines[li].chars().count();
        if ci < len {
            let start = byte_index_at(&self.lines[li], ci);
            let end = byte_index_at(&self.lines[li], ci + 1);
            self.lines[li].replace_range(start..end, "");
        } else if li + 1 < self.lines.len() {
            let next = self.lines.remove(li + 1);
            self.lines[li].push_str(&next);
        }
    }

    fn char_left(&mut self) {
        let (li, ci) = self.cursor;
        if ci > 0 {
            self.cursor.1 = ci - 1;
        } else if li > 0 {
            self.cursor = (li - 1, self.lines[li - 1].chars().count());
        }
    }

    fn char_right(&mut self) {
        let (li, ci) = self.cursor;
        let len = self.lines[li].chars().count();
        if ci < len {
            self.cursor.1 = ci + 1;
        } else if li + 1 < self.lines.len() {
            self.cursor = (li + 1, 0);
        }
    }

    fn word_left(&mut self) {
        let (li, ci) = self.cursor;
        if ci == 0 {
            if li > 0 {
                self.cursor = (li - 1, self.lines[li - 1].chars().count());
            }
            return;
        }
        let chars: Vec<char> = self.lines[li].chars().collect();
        let mut i = ci;
        while i > 0 && chars[i - 1].is_whitespace() {
            i -= 1;
        }
        while i > 0 && !chars[i - 1].is_whitespace() {
            i -= 1;
        }
        self.cursor.1 = i;
    }

    fn word_right(&mut self) {
        let (li, ci) = self.cursor;
        let len = self.lines[li].chars().count();
        if ci >= len {
            if li + 1 < self.lines.len() {
                self.cursor = (li + 1, 0);
            }
            return;
        }
        let chars: Vec<char> = self.lines[li].chars().collect();
        let mut i = ci;
        while i < len && chars[i].is_whitespace() {
            i += 1;
        }
        while i < len && !chars[i].is_whitespace() {
            i += 1;
        }
        self.cursor.1 = i;
    }

    fn delete_word_back(&mut self) {
        let (li, ci) = self.cursor;
        if ci == 0 {
            self.backspace();
            return;
        }
        let chars: Vec<char> = self.lines[li].chars().collect();
        let mut i = ci;
        while i > 0 && chars[i - 1].is_whitespace() {
            i -= 1;
        }
        while i > 0 && !chars[i - 1].is_whitespace() {
            i -= 1;
        }
        let start = byte_index_at(&self.lines[li], i);
        let end = byte_index_at(&self.lines[li], ci);
        self.lines[li].replace_range(start..end, "");
        self.cursor.1 = i;
    }

    fn move_cursor_line(&mut self, delta: i32) {
        let (li, ci) = self.cursor;
        let new_li = li as i32 + delta;
        if new_li < 0 || new_li as usize >= self.lines.len() {
            return;
        }
        let new_li = new_li as usize;
        self.cursor = (new_li, ci.min(self.lines[new_li].chars().count()));
    }

    // —— History ——

    fn push_history(&mut self, t: &str) {
        if t.is_empty() || self.history.last() == Some(&t.to_string()) {
            return;
        }
        self.history.push(t.to_string());
    }

    fn history_up(&mut self) -> bool {
        if self.history.is_empty() {
            return false;
        }
        let pos = match self.history_pos {
            None => {
                self.draft = Some(self.text());
                self.history.len() - 1
            }
            Some(p) => {
                if p == 0 {
                    return false;
                }
                p - 1
            }
        };
        self.history_pos = Some(pos);
        let t = self.history[pos].clone();
        self.set_text(&t);
        true
    }

    fn history_down(&mut self) -> bool {
        let Some(p) = self.history_pos else {
            return false;
        };
        let next = p + 1;
        if next >= self.history.len() {
            self.history_pos = None;
            let t = self.draft.take().unwrap_or_default();
            self.set_text(&t);
        } else {
            self.history_pos = Some(next);
            let t = self.history[next].clone();
            self.set_text(&t);
        }
        true
    }

    // —— Slash command completion ——

    fn update_menu(&mut self) {
        self.menu = None;
        if self.cursor.0 != 0 || self.lines.len() != 1 || self.completions.is_empty() {
            return;
        }
        let text = &self.lines[0];
        if !text.starts_with('/') || text.contains(' ') {
            return;
        }
        let prefix = text[1..].to_ascii_lowercase();
        let indices: Vec<usize> = self
            .completions
            .iter()
            .enumerate()
            .filter(|(_, c)| c.to_ascii_lowercase().starts_with(&prefix))
            .map(|(i, _)| i)
            .collect();
        if !indices.is_empty() {
            self.menu = Some(MenuState { indices, selected: 0 });
        }
    }

    fn menu_up(&mut self) {
        if let Some(m) = &mut self.menu {
            let n = m.indices.len();
            m.selected = (m.selected + n - 1) % n;
        }
    }

    fn menu_down(&mut self) {
        if let Some(m) = &mut self.menu {
            let n = m.indices.len();
            m.selected = (m.selected + 1) % n;
        }
    }

    fn accept_menu(&mut self) {
        let Some(m) = self.menu.take() else { return };
        let cmd = self.completions[m.indices[m.selected]].clone();
        self.lines[0] = format!("/{} ", cmd);
        self.cursor = (0, self.lines[0].chars().count());
        self.scroll_row = 0;
    }
}

impl Widget for Editor {
    fn render(mut self, area: Rect, buf: &mut Buffer) {
        self.render_editor(area, buf);
    }
}

/// Char index → byte index (returns the end-of-line byte position when ci exceeds the line length).
fn byte_index_at(s: &str, ci: usize) -> usize {
    s.char_indices()
        .nth(ci)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}
