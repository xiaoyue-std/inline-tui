//! Markdown → rich text rendering.
//!
//! Supports block-level: headings (#~###), fenced code blocks (with language
//! highlighting and rounded borders), unordered/ordered lists, quotes,
//! horizontal rules, tables (with per-column alignment and inline styles in
//! cells), paragraphs; inline: **bold**, *italic*, ***bold-italic***,
//! ~~strikethrough~~, `inline code`, [links](url), `\` escapes (including
//! `\|` inside table cells).
//!
//! Streaming friendly: unclosed code fences are rendered as code blocks, and the
//! accumulated text is fully re-parsed every frame.

use crate::highlight;
use crate::style::Style;
use crate::text::{pad_line, truncate_line, wrap_line, Line, Span, Text};
use crate::theme::Theme;
use crate::width::str_width;

/// Renders markdown source text as rich text. `width` is the available display width.
///
/// # Tables
///
/// GFM-style tables (header row + `|---|` separator) render as a box-drawing
/// grid with per-column alignment (`:---:` center, `---:` right):
///
/// ```
/// use inline_tui::theme::Theme;
/// let theme = Theme::default();
/// let t = inline_tui::markdown::render("| a | b |\n|---|---|\n| 1 | 2 |", &theme, 40);
/// let joined: String = t
///     .lines
///     .iter()
///     .flat_map(|l| l.spans.iter().map(|s| s.content.clone()))
///     .collect();
/// assert!(joined.contains('┼'));
/// assert!(joined.contains('│'));
/// ```
pub fn render(src: &str, theme: &Theme, width: usize) -> Text {
    let width = width.max(4);
    let mut lines: Vec<Line> = Vec::new();
    let src_lines: Vec<&str> = src.split('\n').collect();
    let mut i = 0;

    let mut para: Vec<&str> = Vec::new();
    let flush_para = |para: &mut Vec<&str>, lines: &mut Vec<Line>| {
        if para.is_empty() {
            return;
        }
        let joined = para.join(" ");
        if lines.last().map(|l| !l.is_empty()).unwrap_or(false) {
            lines.push(Line::empty());
        }
        for row in wrap_line(&inline(&joined, theme.text, theme), width) {
            lines.push(row);
        }
        para.clear();
    };

    while i < src_lines.len() {
        let raw = src_lines[i];
        let trimmed = raw.trim_start();

        // Fenced code block
        if let Some(fence) = trimmed.strip_prefix("```") {
            flush_para(&mut para, &mut lines);
            let lang = fence.trim().to_string();
            i += 1;
            let mut code = String::new();
            let mut closed = false;
            while i < src_lines.len() {
                if src_lines[i].trim_start().starts_with("```") {
                    closed = true;
                    i += 1;
                    break;
                }
                if !code.is_empty() {
                    code.push('\n');
                }
                code.push_str(src_lines[i]);
                i += 1;
            }
            push_code_block(&code, &lang, theme, width, &mut lines, closed);
            continue;
        }

        // Blank line
        if trimmed.is_empty() {
            flush_para(&mut para, &mut lines);
            i += 1;
            continue;
        }

        // Horizontal rule
        if trimmed.len() >= 3 && trimmed.chars().all(|c| c == '-') {
            flush_para(&mut para, &mut lines);
            lines.push(Line::styled("─".repeat(width), theme.hr));
            i += 1;
            continue;
        }

        // Heading
        if let Some(rest) = trimmed.strip_prefix("# ") {
            flush_para(&mut para, &mut lines);
            push_heading(rest, theme.heading1, theme, width, &mut lines);
            i += 1;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("## ") {
            flush_para(&mut para, &mut lines);
            push_heading(rest, theme.heading2, theme, width, &mut lines);
            i += 1;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("### ") {
            flush_para(&mut para, &mut lines);
            push_heading(rest, theme.heading3, theme, width, &mut lines);
            i += 1;
            continue;
        }

        // Quote (consecutive > lines are merged)
        if let Some(rest) = trimmed.strip_prefix("> ") {
            flush_para(&mut para, &mut lines);
            let mut quote = vec![rest.to_string()];
            i += 1;
            while i < src_lines.len() {
                match src_lines[i].trim_start().strip_prefix("> ") {
                    Some(more) => {
                        quote.push(more.to_string());
                        i += 1;
                    }
                    None => break,
                }
            }
            let joined = quote.join("\n");
            for qline in joined.split('\n') {
                for row in wrap_line(&inline(qline, theme.quote, theme), width.saturating_sub(2)) {
                    let mut l = Line::empty();
                    l.push_span(Span::styled("▌ ", theme.quote_bar));
                    l.spans.extend(row.spans.clone());
                    lines.push(l);
                }
            }
            continue;
        }

        // List item
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            if let Some(item) = trimmed.get(2..) {
                flush_para(&mut para, &mut lines);
                push_list_item("•\u{2002}", item, theme, width, &mut lines);
                i += 1;
                continue;
            }
        }
        // Ordered list
        if let Some(dot) = trimmed.find(". ") {
            if dot > 0 && trimmed[..dot].chars().all(|c| c.is_ascii_digit()) {
                flush_para(&mut para, &mut lines);
                let num = &trimmed[..dot];
                let item = &trimmed[dot + 2..];
                push_list_item(&format!("{}\u{2002}", num), item, theme, width, &mut lines);
                i += 1;
                continue;
            }
        }

        // Table (header row followed by an alignment separator)
        if trimmed.starts_with('|')
            && i + 1 < src_lines.len()
            && is_table_separator(src_lines[i + 1])
        {
            flush_para(&mut para, &mut lines);
            let header = split_table_row(trimmed);
            let aligns = parse_aligns(src_lines[i + 1], header.len());
            i += 2;
            let mut body = Vec::new();
            while i < src_lines.len() && src_lines[i].trim_start().starts_with('|') {
                body.push(split_table_row(src_lines[i].trim_start()));
                i += 1;
            }
            push_table(&header, &aligns, &body, theme, width, &mut lines);
            continue;
        }

        para.push(raw.trim_start());
        i += 1;
    }
    flush_para(&mut para, &mut lines);

    Text::from_lines(lines)
}

/// Splits a table row into cells on unescaped `|` pipes (`\|` stays literal).
fn split_table_row(line: &str) -> Vec<String> {
    let s = line.trim();
    let s = s.strip_prefix('|').unwrap_or(s);
    let s = s.strip_suffix('|').unwrap_or(s);
    let mut cells: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut escaped = false;
    for c in s.chars() {
        if escaped {
            if c != '|' {
                cur.push('\\');
            }
            cur.push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '|' {
            cells.push(cur.trim().to_string());
            cur.clear();
        } else {
            cur.push(c);
        }
    }
    if escaped {
        cur.push('\\');
    }
    cells.push(cur.trim().to_string());
    cells
}

/// True when the line is a table alignment separator like `|---|:---:|--:|`.
fn is_table_separator(line: &str) -> bool {
    let s = line.trim();
    if !s.contains('|') {
        return false;
    }
    let cells = split_table_row(s);
    !cells.is_empty()
        && cells.iter().all(|c| {
            let c = c.trim().trim_matches(':');
            !c.is_empty() && c.chars().all(|ch| ch == '-')
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Align {
    Left,
    Center,
    Right,
}

fn parse_aligns(sep: &str, cols: usize) -> Vec<Align> {
    let mut v: Vec<Align> = split_table_row(sep)
        .into_iter()
        .map(|c| match (c.trim().starts_with(':'), c.trim().ends_with(':')) {
            (true, true) => Align::Center,
            (false, true) => Align::Right,
            _ => Align::Left,
        })
        .collect();
    v.resize(cols, Align::Left);
    v
}

/// Renders a table as a box-drawing grid: column widths fit the widest cell,
/// shrunk proportionally when the table exceeds `width`; cells keep their
/// inline styles and honor the per-column alignment.
fn push_table(
    header: &[String],
    aligns: &[Align],
    body: &[Vec<String>],
    theme: &Theme,
    width: usize,
    lines: &mut Vec<Line>,
) {
    let n = header.len();
    if n == 0 {
        return;
    }
    // Per column: "│ " + content + " " ; plus the closing "│".
    let avail = width.saturating_sub(n * 3 + 1).max(n);

    let parse_cells = |cells: &[String]| -> Vec<Line> {
        (0..n)
            .map(|c| inline(cells.get(c).map(String::as_str).unwrap_or(""), theme.text, theme))
            .collect()
    };
    let mut rows: Vec<Vec<Line>> = vec![parse_cells(header)];
    for r in body {
        rows.push(parse_cells(r));
    }

    let mut widths: Vec<usize> = vec![1; n];
    for row in &rows {
        for (c, cell) in row.iter().enumerate() {
            widths[c] = widths[c].max(cell.width());
        }
    }
    // Shrink the widest columns until the table fits.
    loop {
        let total: usize = widths.iter().sum();
        if total <= avail {
            break;
        }
        let Some(max_i) = (0..n).max_by_key(|&i| widths[i]) else {
            break;
        };
        if widths[max_i] <= 1 {
            break;
        }
        widths[max_i] -= 1;
    }

    let border = |l: char, m: char, r: char| -> Line {
        let mut s = String::new();
        s.push(l);
        for (i, w) in widths.iter().enumerate() {
            if i > 0 {
                s.push(m);
            }
            s.push_str(&"─".repeat(w + 2));
        }
        s.push(r);
        Line::styled(s, theme.hr)
    };

    lines.push(border('┌', '┬', '┐'));
    for (ri, row) in rows.iter().enumerate() {
        if ri == 1 {
            lines.push(border('├', '┼', '┤'));
        }
        let mut l = Line::empty();
        l.push_span(Span::styled("│", theme.hr));
        for (c, cell) in row.iter().enumerate() {
            let w = widths[c];
            let content = truncate_line(cell, w);
            let pad = w.saturating_sub(content.width());
            let (left, right) = match aligns.get(c).copied().unwrap_or(Align::Left) {
                Align::Left => (0, pad),
                Align::Center => (pad / 2, pad - pad / 2),
                Align::Right => (pad, 0),
            };
            // one space of cell padding on each side, plus alignment padding
            let left_total = 1 + left;
            let right_total = right + 1;
            if left_total > 0 {
                l.spans.push(Span::raw(" ".repeat(left_total)));
            }
            l.spans.extend(content.spans.clone());
            if right_total > 0 {
                l.spans.push(Span::raw(" ".repeat(right_total)));
            }
        }
        l.push_span(Span::styled("│", theme.hr));
        lines.push(l);
    }
    lines.push(border('└', '┴', '┘'));
}

fn push_heading(text: &str, st: Style, theme: &Theme, width: usize, lines: &mut Vec<Line>) {
    if !lines.is_empty() {
        lines.push(Line::empty());
    }
    for row in wrap_line(&inline(text, st, theme), width) {
        lines.push(row);
    }
}

/// List item: `bullet_span` is a prefix such as "• " or "1. "; wrapped lines get a hanging indent.
fn push_list_item(bullet: &str, item: &str, theme: &Theme, width: usize, lines: &mut Vec<Line>) {
    let bullet_disp = if bullet.starts_with('•') {
        Span::styled(bullet.to_string(), theme.list_bullet)
    } else {
        Span::styled(bullet.to_string(), theme.accent)
    };
    let bw = str_width(bullet);
    let body = inline(item, theme.text, theme);
    let rows = wrap_line(&body, width.saturating_sub(bw));
    let mut l = Line::empty();
    l.spans.push(bullet_disp);
    if let Some(first) = rows.first() {
        l.spans.extend(first.spans.clone());
    }
    lines.push(l);
    for row in rows.iter().skip(1) {
        let mut cont = Line::empty();
        cont.spans.push(Span::raw(" ".repeat(bw)));
        cont.spans.extend(row.spans.clone());
        lines.push(cont);
    }
}

/// Code block: rounded border + language label + syntax highlighting.
fn push_code_block(
    code: &str,
    lang: &str,
    theme: &Theme,
    width: usize,
    lines: &mut Vec<Line>,
    closed: bool,
) {
    if !lines.is_empty() {
        lines.push(Line::empty());
    }
    let inner = width.saturating_sub(4); // │ + space + content + space
    let label = if lang.is_empty() { String::new() } else { format!(" {} ", lang) };
    let label_w = str_width(&label);
    let fill = inner.saturating_sub(label_w);

    let mut top = Line::empty();
    top.push_span(Span::styled("╭", theme.code_border));
    top.push_span(Span::styled("─", theme.code_border));
    top.push_span(Span::styled(label, theme.code_border));
    top.push_span(Span::styled("─".repeat(fill), theme.code_border));
    top.push_span(Span::styled("─╮", theme.code_border));
    lines.push(truncate_line(&top, width));

    let highlighted = highlight::highlight(code, lang, theme);
    for hline in highlighted {
        let content = truncate_line(&hline, inner);
        let mut l = Line::empty();
        l.push_span(Span::styled("│ ", theme.code_border));
        l.spans.extend(pad_line(&content, inner).spans);
        l.push_span(Span::styled(" │", theme.code_border));
        lines.push(l);
    }

    let mut bottom = Line::empty();
    bottom.push_span(Span::styled("╰", theme.code_border));
    bottom.push_span(Span::styled("─".repeat(width.saturating_sub(3)), theme.code_border));
    if !closed {
        bottom.push_span(Span::styled(" …", theme.code_border));
    } else {
        bottom.push_span(Span::styled("╯", theme.code_border));
    }
    lines.push(truncate_line(&bottom, width));
}

/// Inline markdown parsing: returns a sequence of styled spans.
pub fn inline(src: &str, base: Style, theme: &Theme) -> Line {
    let spans = parse_inline(src, base, theme);
    Line { spans, style: Style::new() }
}

fn parse_inline(src: &str, base: Style, theme: &Theme) -> Vec<Span> {
    let chars: Vec<char> = src.chars().collect();
    let mut spans: Vec<Span> = Vec::new();
    let mut plain = String::new();
    let mut i = 0;

    macro_rules! flush {
        () => {
            if !plain.is_empty() {
                spans.push(Span::styled(std::mem::take(&mut plain), base));
            }
        };
    }

    while i < chars.len() {
        let rest: String = chars[i..].iter().collect();

        // Escape
        if chars[i] == '\\' && i + 1 < chars.len() {
            let next = chars[i + 1];
            if "*_`~[]\\|".contains(next) {
                plain.push(next);
                i += 2;
                continue;
            }
        }

        // Bold+italic ***x***
        if rest.starts_with("***") {
            if let Some(end) = find_from(&chars, i + 3, "***") {
                flush!();
                let inner: String = chars[i + 3..end].iter().collect();
                spans.extend(parse_inline(&inner, base.patch(theme.bold).patch(theme.italic), theme));
                i = end + 3;
                continue;
            }
        }

        // Bold **x**
        if rest.starts_with("**") {
            if let Some(end) = find_from(&chars, i + 2, "**") {
                flush!();
                let inner: String = chars[i + 2..end].iter().collect();
                spans.extend(parse_inline(&inner, base.patch(theme.bold), theme));
                i = end + 2;
                continue;
            }
        }
        // Strikethrough ~~x~~
        if rest.starts_with("~~") {
            if let Some(end) = find_from(&chars, i + 2, "~~") {
                flush!();
                let inner: String = chars[i + 2..end].iter().collect();
                spans.extend(parse_inline(&inner, base.patch(theme.strikethrough), theme));
                i = end + 2;
                continue;
            }
        }
        // Inline code `x`
        if chars[i] == '`' {
            if let Some(end) = find_from(&chars, i + 1, "`") {
                flush!();
                let inner: String = chars[i + 1..end].iter().collect();
                spans.push(Span::styled(inner, base.patch(theme.inline_code)));
                i = end + 1;
                continue;
            }
        }
        // Link [text](url)
        if chars[i] == '[' {
            if let Some((text_end, url_end)) = find_link(&chars, i) {
                flush!();
                let text: String = chars[i + 1..text_end].iter().collect();
                let url: String = chars[text_end + 2..url_end].iter().collect();
                spans.push(Span::styled(text, base.patch(theme.link)));
                spans.push(Span::styled(format!(" ({})", url), base.patch(theme.dim)));
                i = url_end + 1;
                continue;
            }
        }
        // Italic *x* / _x_ (only effective in pairs)
        if (chars[i] == '*' || chars[i] == '_')
            && (i == 0 || chars[i - 1] != chars[i])
            && !rest.starts_with("**")
        {
            if let Some(end) = find_from(&chars, i + 1, &chars[i].to_string()) {
                if end > i + 1 {
                    flush!();
                    let inner: String = chars[i + 1..end].iter().collect();
                    spans.extend(parse_inline(&inner, base.patch(theme.italic), theme));
                    i = end + 1;
                    continue;
                }
            }
        }

        plain.push(chars[i]);
        i += 1;
    }
    if !plain.is_empty() {
        spans.push(Span::styled(plain, base));
    }
    spans
}

fn find_from(chars: &[char], from: usize, pat: &str) -> Option<usize> {
    let pat: Vec<char> = pat.chars().collect();
    (from..=chars.len().saturating_sub(pat.len())).find(|&i| chars[i..i + pat.len()] == pat[..])
}

/// When `chars[i]` is `[`, looks for the `](…)` structure and returns (text end index, url end index).
fn find_link(chars: &[char], i: usize) -> Option<(usize, usize)> {
    let mut j = i + 1;
    while j < chars.len() && chars[j] != ']' {
        j += 1;
    }
    if j + 1 >= chars.len() || chars[j + 1] != '(' {
        return None;
    }
    let text_end = j;
    let mut k = j + 2;
    while k < chars.len() && chars[k] != ')' {
        k += 1;
    }
    if k >= chars.len() {
        return None;
    }
    Some((text_end, k))
}
