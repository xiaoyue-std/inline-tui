//! Lightweight syntax highlighting: hand-written token scanner, no regex dependency.
//!
//! Supports comments (line/block), strings (with escapes), numbers, keywords,
//! type names starting with an uppercase letter, and function names followed by
//! `(`. Covers rust / c / cpp / python / js / ts / json / bash / go / java.

use crate::text::{Line, Span};
use crate::theme::Theme;

struct LangCfg {
    keywords: &'static [&'static str],
    line_comments: &'static [&'static str],
    block_comment: Option<(&'static str, &'static str)>,
    quotes: &'static [char],
    /// JSON mode: strings followed by `:` are rendered with the key style.
    json: bool,
}

const RUST_KW: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while",
];
const C_KW: &[&str] = &[
    "auto", "break", "case", "char", "const", "continue", "default", "do", "double", "else", "enum",
    "extern", "float", "for", "goto", "if", "int", "long", "register", "return", "short", "signed",
    "sizeof", "static", "struct", "switch", "typedef", "union", "unsigned", "void", "volatile",
    "while",
];
const CPP_KW: &[&str] = &[
    "alignas", "auto", "bool", "break", "case", "catch", "char", "class", "const", "constexpr",
    "continue", "default", "delete", "do", "double", "else", "enum", "explicit", "export", "extern",
    "false", "float", "for", "friend", "goto", "if", "inline", "int", "long", "mutable",
    "namespace", "new", "noexcept", "nullptr", "operator", "private", "protected", "public",
    "return", "short", "signed", "sizeof", "static", "struct", "switch", "template", "this",
    "throw", "true", "try", "typedef", "typename", "union", "unsigned", "using", "virtual", "void",
    "volatile", "while",
];
const PY_KW: &[&str] = &[
    "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif",
    "else", "except", "False", "finally", "for", "from", "global", "if", "import", "in", "is",
    "lambda", "None", "nonlocal", "not", "or", "pass", "raise", "return", "True", "try", "while",
    "with", "yield",
];
const JS_KW: &[&str] = &[
    "async", "await", "break", "case", "catch", "class", "const", "continue", "debugger",
    "default", "delete", "do", "else", "export", "extends", "false", "finally", "for", "function",
    "if", "import", "in", "instanceof", "let", "new", "null", "of", "return", "static", "super",
    "switch", "this", "throw", "true", "try", "typeof", "undefined", "var", "void", "while",
    "yield",
];
const JSON_KW: &[&str] = &["false", "true", "null"];
const SH_KW: &[&str] = &[
    "case", "do", "done", "elif", "else", "esac", "fi", "for", "function", "if", "in", "return",
    "then", "until", "while", "local", "export", "echo", "cd", "set", "unset", "source",
];
const GO_KW: &[&str] = &[
    "break", "case", "chan", "const", "continue", "default", "defer", "else", "fallthrough", "for",
    "func", "go", "goto", "if", "import", "interface", "map", "package", "range", "return",
    "select", "struct", "switch", "type", "var", "nil", "true", "false",
];
const JAVA_KW: &[&str] = &[
    "abstract", "boolean", "break", "byte", "case", "catch", "char", "class", "const", "continue",
    "default", "do", "double", "else", "enum", "extends", "final", "finally", "float", "for", "goto",
    "if", "implements", "import", "instanceof", "int", "interface", "long", "native", "new",
    "package", "private", "protected", "public", "return", "short", "static", "super", "switch",
    "synchronized", "this", "throw", "throws", "transient", "try", "void", "volatile", "while",
    "true", "false", "null",
];

fn config(lang: &str) -> LangCfg {
    match lang.to_ascii_lowercase().as_str() {
        "rust" | "rs" => LangCfg {
            keywords: RUST_KW,
            line_comments: &["//"],
            block_comment: Some(("/*", "*/")),
            quotes: &['"', '\''],
            json: false,
        },
        "c" | "h" => LangCfg {
            keywords: C_KW,
            line_comments: &["//"],
            block_comment: Some(("/*", "*/")),
            quotes: &['"', '\''],
            json: false,
        },
        "cpp" | "c++" | "cc" | "cxx" | "hpp" => LangCfg {
            keywords: CPP_KW,
            line_comments: &["//"],
            block_comment: Some(("/*", "*/")),
            quotes: &['"', '\''],
            json: false,
        },
        "python" | "py" => LangCfg {
            keywords: PY_KW,
            line_comments: &["#"],
            block_comment: None,
            quotes: &['"', '\''],
            json: false,
        },
        "javascript" | "js" | "typescript" | "ts" | "jsx" | "tsx" => LangCfg {
            keywords: JS_KW,
            line_comments: &["//"],
            block_comment: Some(("/*", "*/")),
            quotes: &['"', '\'', '`'],
            json: false,
        },
        "json" | "jsonc" => LangCfg {
            keywords: JSON_KW,
            line_comments: &["//"],
            block_comment: None,
            quotes: &['"'],
            json: true,
        },
        "bash" | "sh" | "shell" | "zsh" => LangCfg {
            keywords: SH_KW,
            line_comments: &["#"],
            block_comment: None,
            quotes: &['"', '\''],
            json: false,
        },
        "go" => LangCfg {
            keywords: GO_KW,
            line_comments: &["//"],
            block_comment: Some(("/*", "*/")),
            quotes: &['"', '\''],
            json: false,
        },
        "java" => LangCfg {
            keywords: JAVA_KW,
            line_comments: &["//"],
            block_comment: Some(("/*", "*/")),
            quotes: &['"', '\''],
            json: false,
        },
        _ => LangCfg {
            keywords: &[],
            line_comments: &[],
            block_comment: None,
            quotes: &['"', '\''],
            json: false,
        },
    }
}

/// Highlights a piece of code into a collection of styled lines.
///
/// Block comment state persists across lines, naturally supporting incomplete
/// code during streaming renders.
pub fn highlight(code: &str, lang: &str, theme: &Theme) -> Vec<Line> {
    let cfg = config(lang);
    let mut in_block = false;
    let mut out = Vec::new();
    for line in code.split('\n') {
        out.push(highlight_line(line, &cfg, theme, &mut in_block));
    }
    out
}

fn highlight_line(line: &str, cfg: &LangCfg, theme: &Theme, in_block: &mut bool) -> Line {
    let chars: Vec<char> = line.chars().collect();
    let at = |i: usize, s: &str| -> bool {
        s.chars().enumerate().all(|(k, c)| chars.get(i + k) == Some(&c))
    };
    let mut spans: Vec<Span> = Vec::new();
    let mut plain = String::new();
    let mut i = 0;

    macro_rules! flush_plain {
        () => {
            if !plain.is_empty() {
                spans.push(Span::styled(std::mem::take(&mut plain), theme.text));
            }
        };
    }

    while i < chars.len() {
        if *in_block {
            if let Some((_, end)) = cfg.block_comment {
                // Find the block comment end
                let mut j = i;
                while j < chars.len() {
                    if at(j, end) {
                        j += end.chars().count();
                        spans.push(Span::styled(chars[i..j].iter().collect::<String>(), theme.comment));
                        *in_block = false;
                        break;
                    }
                    j += 1;
                }
                if *in_block {
                    spans.push(Span::styled(chars[i..].iter().collect::<String>(), theme.comment));
                    i = chars.len();
                } else {
                    i = j;
                }
                continue;
            }
        }
        // Block comment start
        if let Some((start, _)) = cfg.block_comment {
            if at(i, start) {
                flush_plain!();
                *in_block = true;
                continue;
            }
        }
        // Line comment
        if cfg.line_comments.iter().any(|p| at(i, p)) {
            flush_plain!();
            spans.push(Span::styled(chars[i..].iter().collect::<String>(), theme.comment));
            break;
        }
        let c = chars[i];
        // String
        if cfg.quotes.contains(&c) {
            flush_plain!();
            let quote = c;
            let mut j = i + 1;
            while j < chars.len() {
                if chars[j] == '\\' {
                    j += 2;
                    continue;
                }
                if chars[j] == quote {
                    j += 1;
                    break;
                }
                j += 1;
            }
            let content: String = chars[i..j.min(chars.len())].iter().collect();
            // json: one followed by ':' is a key
            let is_json_key = cfg.json && {
                let mut k = j;
                while k < chars.len() && chars[k] == ' ' {
                    k += 1;
                }
                chars.get(k) == Some(&':')
            };
            spans.push(Span::styled(content, if is_json_key { theme.keyword } else { theme.string }));
            i = j;
            continue;
        }
        // Number
        if c.is_ascii_digit() {
            flush_plain!();
            let mut j = i;
            while j < chars.len()
                && (chars[j].is_ascii_alphanumeric() || chars[j] == '_' || chars[j] == '.')
            {
                j += 1;
            }
            spans.push(Span::styled(chars[i..j].iter().collect::<String>(), theme.number));
            i = j;
            continue;
        }
        // Identifier
        if c.is_alphabetic() || c == '_' {
            let mut j = i;
            while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
                j += 1;
            }
            let word: String = chars[i..j].iter().collect::<String>();
            flush_plain!();
            let st = if cfg.keywords.contains(&word.as_str()) {
                theme.keyword
            } else if word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                theme.type_name
            } else {
                let mut k = j;
                while k < chars.len() && chars[k] == ' ' {
                    k += 1;
                }
                if chars.get(k) == Some(&'(') {
                    theme.fn_name
                } else {
                    theme.text
                }
            };
            spans.push(Span::styled(word, st));
            i = j;
            continue;
        }
        plain.push(c);
        i += 1;
    }
    if !plain.is_empty() {
        spans.push(Span::styled(plain, theme.text));
    }
    Line::from_spans(spans)
}
