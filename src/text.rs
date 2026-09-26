//! Rich text model: `Span` / `Line` / `Text`, plus styled word-level wrapping.

use crate::style::Style;
use crate::width::{char_width, str_width};

/// A run of text with a single style.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Span {
    pub content: String,
    pub style: Style,
}

impl Span {
    pub fn raw(s: impl Into<String>) -> Span {
        Span { content: s.into(), style: Style::new() }
    }

    pub fn styled(s: impl Into<String>, style: Style) -> Span {
        Span { content: s.into(), style }
    }

    pub fn width(&self) -> usize {
        str_width(&self.content)
    }
}

/// A line of rich text: several spans + a line-level base style (span styles patch on top).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Line {
    pub spans: Vec<Span>,
    pub style: Style,
}

impl Line {
    pub fn raw(s: impl Into<String>) -> Line {
        Line::from_spans(vec![Span::raw(s)])
    }

    pub fn styled(s: impl Into<String>, style: Style) -> Line {
        Line::from_spans(vec![Span::styled(s, style)])
    }

    pub fn from_spans(spans: Vec<Span>) -> Line {
        Line { spans, style: Style::new() }
    }

    pub fn empty() -> Line {
        Line::default()
    }

    pub fn push_span(&mut self, span: Span) -> &mut Line {
        self.spans.push(span);
        self
    }

    pub fn width(&self) -> usize {
        self.spans.iter().map(Span::width).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.spans.iter().all(|s| s.content.is_empty())
    }
}

impl From<&str> for Line {
    fn from(s: &str) -> Line {
        Line::raw(s)
    }
}

impl From<String> for Line {
    fn from(s: String) -> Line {
        Line::raw(s)
    }
}

/// Multi-line rich text.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Text {
    pub lines: Vec<Line>,
}

impl Text {
    pub fn raw(s: &str) -> Text {
        Text { lines: s.split('\n').map(Line::raw).collect() }
    }

    pub fn styled(s: &str, style: Style) -> Text {
        Text { lines: s.split('\n').map(|l| Line::styled(l, style)).collect() }
    }

    pub fn from_lines(lines: Vec<Line>) -> Text {
        Text { lines }
    }

    pub fn push_line(&mut self, line: Line) -> &mut Text {
        self.lines.push(line);
        self
    }

    pub fn extend(&mut self, other: Text) -> &mut Text {
        self.lines.extend(other.lines);
        self
    }

    pub fn width(&self) -> usize {
        self.lines.iter().map(Line::width).max().unwrap_or(0)
    }

    pub fn height(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

impl From<&str> for Text {
    fn from(s: &str) -> Text {
        Text::raw(s)
    }
}

impl From<String> for Text {
    fn from(s: String) -> Text {
        Text::raw(&s)
    }
}

impl From<Vec<Line>> for Text {
    fn from(lines: Vec<Line>) -> Text {
        Text { lines }
    }
}

impl<'a> IntoIterator for &'a Line {
    type Item = &'a Span;
    type IntoIter = std::slice::Iter<'a, Span>;
    fn into_iter(self) -> Self::IntoIter {
        self.spans.iter()
    }
}

/// Flattens a line into a sequence of (char, effective style) (line style patches span style).
pub fn glyphs(line: &Line) -> Vec<(char, Style)> {
    let mut out = Vec::new();
    for span in &line.spans {
        let st = line.style.patch(span.style);
        for ch in span.content.chars() {
            out.push((ch, st));
        }
    }
    out
}

fn glyphs_width(g: &[(char, Style)]) -> usize {
    g.iter().map(|(c, _)| char_width(*c)).sum()
}

fn line_from_glyphs(g: &[(char, Style)], base: Style) -> Line {
    let mut spans: Vec<Span> = Vec::new();
    for &(ch, st) in g {
        match spans.last_mut() {
            Some(last) if last.style == st => last.content.push(ch),
            _ => spans.push(Span::styled(ch.to_string(), st)),
        }
    }
    Line { spans, style: base }
}

/// Word-level wrapping: folds a rich text line into several lines, each no wider
/// than `max` display columns.
///
/// Rules: spaces are word separators and are dropped at line breaks; overlong
/// words wider than a whole line are hard-cut per character; wide characters are
/// never split in half.
pub fn wrap_line(line: &Line, max: usize) -> Vec<Line> {
    if max == 0 {
        return vec![line.clone()];
    }
    let g = glyphs(line);
    let mut out: Vec<Line> = Vec::new();
    let mut cur: Vec<(char, Style)> = Vec::new();
    let mut cur_w = 0usize;

    let mut i = 0;
    while i < g.len() {
        // Collect leading spaces
        let sp_start = i;
        while i < g.len() && g[i].0 == ' ' {
            i += 1;
        }
        let spaces = &g[sp_start..i];
        // Collect the word
        let w_start = i;
        while i < g.len() && g[i].0 != ' ' {
            i += 1;
        }
        let word = &g[w_start..i];
        let ww = glyphs_width(word);
        if ww == 0 && spaces.is_empty() {
            break;
        }
        if cur.is_empty() {
            // Start of line: place the word directly (hard-cut if needed)
            push_word(&mut out, &mut cur, &mut cur_w, word, max, line.style);
        } else if cur_w + spaces.len() + ww <= max {
            cur.extend_from_slice(spaces);
            cur.extend_from_slice(word);
            cur_w += spaces.len() + ww;
        } else {
            // Wrap (drop the spaces)
            out.push(line_from_glyphs(&cur, line.style));
            cur.clear();
            cur_w = 0;
            push_word(&mut out, &mut cur, &mut cur_w, word, max, line.style);
        }
    }
    out.push(line_from_glyphs(&cur, line.style));
    out
}

fn push_word(out: &mut Vec<Line>, cur: &mut Vec<(char, Style)>, cur_w: &mut usize, word: &[(char, Style)], max: usize, base: Style) {
    let ww = glyphs_width(word);
    if ww <= max {
        if *cur_w + ww > max && !cur.is_empty() {
            out.push(line_from_glyphs(cur, base));
            cur.clear();
            *cur_w = 0;
        }
        cur.extend_from_slice(word);
        *cur_w += ww;
        return;
    }
    // Hard-cut an overlong word
    if !cur.is_empty() {
        out.push(line_from_glyphs(cur, base));
        cur.clear();
        *cur_w = 0;
    }
    let mut chunk: Vec<(char, Style)> = Vec::new();
    let mut w = 0;
    for &(ch, st) in word {
        let cw = char_width(ch);
        if w + cw > max {
            out.push(line_from_glyphs(&chunk, base));
            chunk.clear();
            w = 0;
        }
        chunk.push((ch, st));
        w += cw;
    }
    *cur = chunk;
    *cur_w = w;
}

/// Plain-text wrapping, returns (line content, index of the first char in the original line).
///
/// Used by the editor for cursor mapping.
pub fn wrap_plain(s: &str, max: usize) -> Vec<(String, usize)> {
    if max == 0 {
        return vec![(s.to_string(), 0)];
    }
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut cur_w = 0usize;
    let mut cur_start = 0usize;
    let mut i = 0;

    while i < chars.len() {
        let sp_start = i;
        while i < chars.len() && chars[i] == ' ' {
            i += 1;
        }
        let spaces = i - sp_start;
        let w_start = i;
        while i < chars.len() && chars[i] != ' ' {
            i += 1;
        }
        let word = &chars[w_start..i];
        let ww: usize = word.iter().map(|c| char_width(*c)).sum();

        if cur.is_empty() {
            // Start of line: leading spaces are dropped; end if no word remains
            if ww == 0 {
                continue;
            }
            cur_start = w_start;
            if ww <= max {
                cur.extend(word.iter());
                cur_w = ww;
            } else {
                // Hard-cut an overlong word
                let mut w = 0;
                let mut start = w_start;
                for k in w_start..i {
                    let cw = char_width(chars[k]);
                    if w + cw > max {
                        out.push((chars[start..k].iter().collect(), start));
                        start = k;
                        w = 0;
                    }
                    w += cw;
                }
                cur.extend(chars[start..i].iter());
                cur_start = start;
                cur_w = w;
            }
        } else if cur_w + spaces + ww <= max {
            for _ in 0..spaces {
                cur.push(' ');
            }
            cur.extend(word.iter());
            cur_w += spaces + ww;
        } else {
            out.push((std::mem::take(&mut cur), cur_start));
            if ww == 0 {
                continue;
            }
            cur_start = w_start;
            if ww <= max {
                cur.extend(word.iter());
                cur_w = ww;
            } else {
                let mut w = 0;
                let mut start = w_start;
                for k in w_start..i {
                    let cw = char_width(chars[k]);
                    if w + cw > max {
                        out.push((chars[start..k].iter().collect(), start));
                        start = k;
                        w = 0;
                    }
                    w += cw;
                }
                cur.extend(chars[start..i].iter());
                cur_start = start;
                cur_w = w;
            }
        }
    }
    if !cur.is_empty() || out.is_empty() {
        out.push((cur, cur_start));
    }
    out
}

/// Truncates a rich text line to a display width (span styles preserved).
pub fn truncate_line(line: &Line, max: usize) -> Line {
    if line.width() <= max {
        return line.clone();
    }
    let mut out = Line { spans: Vec::new(), style: line.style };
    let mut w = 0;
    for span in &line.spans {
        let st = line.style.patch(span.style);
        for ch in span.content.chars() {
            let cw = char_width(ch);
            if w + cw > max {
                return out;
            }
            match out.spans.last_mut() {
                Some(last) if last.style == st => last.content.push(ch),
                _ => out.spans.push(Span::styled(ch.to_string(), st)),
            }
            w += cw;
        }
    }
    out
}

/// Pads a line with spaces to the given display width.
pub fn pad_line(line: &Line, min: usize) -> Line {
    let mut out = line.clone();
    let w = out.width();
    if w < min {
        out.spans.push(Span::raw(" ".repeat(min - w)));
    }
    out
}
