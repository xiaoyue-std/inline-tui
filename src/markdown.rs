//! Markdown → rich text rendering.
//!
//! Supports block-level: headings (#~###), fenced code blocks (with language
//! highlighting and rounded borders), unordered/ordered lists, quotes,
//! horizontal rules, paragraphs; inline: **bold**, *italic*, ~~strikethrough~~,
//! `inline code`, [links](url), `\` escapes.
//!
//! Streaming friendly: unclosed code fences are rendered as code blocks, and the
//! accumulated text is fully re-parsed every frame.

use crate::highlight;
use crate::style::Style;
use crate::text::{pad_line, truncate_line, wrap_line, Line, Span, Text};
use crate::theme::Theme;
use crate::width::str_width;

/// Renders markdown source text as rich text. `width` is the available display width.
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

        para.push(raw.trim_start());
        i += 1;
    }
    flush_para(&mut para, &mut lines);

    Text::from_lines(lines)
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
            if "*_`~[]\\".contains(next) {
                plain.push(next);
                i += 2;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Modifier;
    use crate::width::str_width;

    #[test]
    fn headings_and_paragraph() {
        let th = Theme::default();
        let t = render("# Title\n\nhello **world**", &th, 40);
        assert_eq!(t.lines[0].spans[0].content, "Title");
        assert_eq!(t.lines[0].spans[0].style, th.heading1);
        let bold = t.lines.iter().flatten().find(|s| s.content == "world").unwrap();
        assert_eq!(bold.style.add_modifier, Modifier::BOLD);
    }

    #[test]
    fn inline_styles() {
        let th = Theme::default();
        let l = inline("**b** *i* `c` ~~s~~ [t](http://x)", Style::new(), &th);
        let texts: Vec<(&str, Style)> = l.spans.iter().map(|s| (s.content.as_str(), s.style)).collect();
        assert!(texts.iter().any(|(c, s)| *c == "b" && s.add_modifier == Modifier::BOLD));
        assert!(texts.iter().any(|(c, s)| *c == "i" && s.add_modifier == Modifier::ITALIC));
        assert!(texts.iter().any(|(c, s)| *c == "c" && *s == th.inline_code));
        assert!(texts.iter().any(|(c, s)| *c == "s" && s.add_modifier == Modifier::CROSSED_OUT));
        assert!(texts.iter().any(|(c, _)| *c == "t"));
        assert!(texts.iter().any(|(c, _)| c.contains("http://x")));
    }

    #[test]
    fn code_block_with_border() {
        let th = Theme::default();
        let t = render("```rust\nlet x = 1;\n```", &th, 30);
        assert!(t.lines[0].spans[0].content.starts_with('╭'));
        assert!(t.lines[0].spans.iter().any(|s| s.content.contains("rust")));
        assert!(t.lines.iter().flatten().any(|s| s.content == "let"));
        assert!(t.lines.last().unwrap().spans[0].content.starts_with('╰'));
    }

    #[test]
    fn streaming_unterminated_fence() {
        let th = Theme::default();
        let t = render("code:\n```python\nprint('hi')", &th, 30);
        // Unterminated fence renders as a code block with an ellipsis at the bottom
        assert!(t.lines.last().unwrap().spans.iter().any(|s| s.content.contains('…')));
        assert!(t.lines.iter().flatten().any(|s| s.content == "print"));
    }

    #[test]
    fn lists_and_quote() {
        let th = Theme::default();
        let t = render("- one\n- two\n\n> quoted", &th, 40);
        assert!(t.lines[0].spans[0].content.starts_with('•'));
        assert!(t.lines.iter().flatten().any(|s| s.content.contains("two")));
        assert!(t.lines.iter().flatten().any(|s| s.content.starts_with('▌')));
    }

    #[test]
    fn cjk_wraps() {
        let th = Theme::default();
        let t = render("中文段落内容测试", &th, 8);
        assert!(t.lines.iter().all(|l| str_width(&l.spans.iter().map(|s| s.content.clone()).collect::<String>()) <= 8));
        assert!(t.lines.len() >= 2);
    }
}
