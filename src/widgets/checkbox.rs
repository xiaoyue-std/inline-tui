//! Form controls: checkbox and radio group (render-only widgets; interaction state is held by the application).

use crate::buffer::Buffer;
use crate::layout::Rect;
use crate::style::Style;
use crate::text::{Span, Line};
use crate::widgets::Widget;

/// Checkbox. Default symbols are ASCII (`[x]` / `[ ]`); swap in ☑/☐ yourself on terminals
/// where their width is ambiguous.
#[derive(Debug, Clone)]
pub struct Checkbox<'a> {
    label: &'a str,
    checked: bool,
    checked_symbol: String,
    unchecked_symbol: String,
    style: Style,
    checked_style: Style,
}

impl<'a> Checkbox<'a> {
    pub fn new(label: &'a str, checked: bool) -> Checkbox<'a> {
        Checkbox {
            label,
            checked,
            checked_symbol: "[x] ".to_string(),
            unchecked_symbol: "[ ] ".to_string(),
            style: Style::new(),
            checked_style: Style::new(),
        }
    }

    pub fn symbols(mut self, checked: impl Into<String>, unchecked: impl Into<String>) -> Checkbox<'a> {
        self.checked_symbol = checked.into();
        self.unchecked_symbol = unchecked.into();
        self
    }

    pub fn styles(mut self, normal: Style, checked: Style) -> Checkbox<'a> {
        self.style = normal;
        self.checked_style = checked;
        self
    }
}

impl Widget for Checkbox<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let mut line = Line::empty();
        if self.checked {
            line.spans.push(Span::styled(self.checked_symbol.clone(), self.checked_style));
            line.spans.push(Span::styled(self.label.to_string(), self.checked_style));
        } else {
            line.spans.push(Span::styled(self.unchecked_symbol.clone(), self.style));
            line.spans.push(Span::styled(self.label.to_string(), self.style));
        }
        buf.set_line(area.x, area.y, &crate::text::truncate_line(&line, area.width as usize));
    }
}

/// Radio group. `selected` is the index of the selected option.
#[derive(Debug, Clone)]
pub struct RadioGroup {
    options: Vec<String>,
    selected: usize,
    selected_symbol: String,
    unselected_symbol: String,
    style: Style,
    selected_style: Style,
}

impl RadioGroup {
    pub fn new<'a>(options: impl IntoIterator<Item = &'a str>, selected: usize) -> RadioGroup {
        RadioGroup {
            options: options.into_iter().map(str::to_string).collect(),
            selected,
            selected_symbol: "(*) ".to_string(),
            unselected_symbol: "( ) ".to_string(),
            style: Style::new(),
            selected_style: Style::new(),
        }
    }

    pub fn symbols(mut self, selected: impl Into<String>, unselected: impl Into<String>) -> RadioGroup {
        self.selected_symbol = selected.into();
        self.unselected_symbol = unselected.into();
        self
    }

    pub fn styles(mut self, normal: Style, selected: Style) -> RadioGroup {
        self.style = normal;
        self.selected_style = selected;
        self
    }
}

impl Widget for RadioGroup {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        for (i, opt) in self.options.iter().enumerate() {
            let y = area.y + i as u16;
            if y >= area.bottom() {
                break;
            }
            let mut line = Line::empty();
            if i == self.selected {
                line.spans.push(Span::styled(self.selected_symbol.clone(), self.selected_style));
                line.spans.push(Span::styled(opt.clone(), self.selected_style));
            } else {
                line.spans.push(Span::styled(self.unselected_symbol.clone(), self.style));
                line.spans.push(Span::styled(opt.clone(), self.style));
            }
            buf.set_line(area.x, y, &crate::text::truncate_line(&line, area.width as usize));
        }
    }
}
