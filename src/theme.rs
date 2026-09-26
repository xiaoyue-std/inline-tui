//! Theme: maps semantic roles to styles; default dark theme (orange accent).

use crate::style::{Color, Modifier, Style};

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

/// Collection of semantic styles. All fields are `Style`; override individual fields as needed.
#[derive(Debug, Clone)]
pub struct Theme {
    // —— Structure ——
    pub border: Style,
    pub border_focused: Style,
    pub text: Style,
    pub dim: Style,
    pub accent: Style,
    pub placeholder: Style,
    pub selection: Style,
    pub error: Style,
    pub success: Style,
    pub warning: Style,

    // —— Status bar / indicators ——
    pub spinner: Style,
    pub status_hint: Style,
    pub status_hint_key: Style,

    // —— Markdown ——
    pub heading1: Style,
    pub heading2: Style,
    pub heading3: Style,
    pub bold: Style,
    pub italic: Style,
    pub strikethrough: Style,
    pub inline_code: Style,
    pub link: Style,
    pub quote: Style,
    pub quote_bar: Style,
    pub list_bullet: Style,
    pub hr: Style,
    pub code_border: Style,

    // —— Syntax highlighting ——
    pub keyword: Style,
    pub string: Style,
    pub comment: Style,
    pub number: Style,
    pub type_name: Style,
    pub fn_name: Style,

    // —— Diff ——
    pub diff_add: Style,
    pub diff_del: Style,
    pub diff_hunk: Style,
    pub diff_meta: Style,
}

impl Default for Theme {
    fn default() -> Theme {
        let accent = rgb(215, 119, 87); // default accent color (orange)
        Theme {
            border: Style::new().fg(rgb(85, 85, 85)),
            border_focused: Style::new().fg(accent),
            text: Style::new(),
            dim: Style::new().fg(rgb(125, 125, 125)),
            accent: Style::new().fg(accent),
            placeholder: Style::new().fg(rgb(125, 125, 125)).add_modifier(Modifier::ITALIC),
            selection: Style::new().add_modifier(Modifier::REVERSED),
            error: Style::new().fg(rgb(235, 100, 100)),
            success: Style::new().fg(rgb(130, 200, 130)),
            warning: Style::new().fg(rgb(230, 190, 100)),

            spinner: Style::new().fg(accent),
            status_hint: Style::new().fg(rgb(125, 125, 125)),
            status_hint_key: Style::new().fg(accent).add_modifier(Modifier::BOLD),

            heading1: Style::new().fg(Color::White).add_modifier(Modifier::BOLD),
            heading2: Style::new().fg(rgb(225, 225, 225)).add_modifier(Modifier::BOLD),
            heading3: Style::new().add_modifier(Modifier::BOLD),
            bold: Style::new().add_modifier(Modifier::BOLD),
            italic: Style::new().add_modifier(Modifier::ITALIC),
            strikethrough: Style::new().add_modifier(Modifier::CROSSED_OUT),
            inline_code: Style::new().fg(rgb(230, 165, 130)),
            link: Style::new().fg(rgb(120, 170, 255)).add_modifier(Modifier::UNDERLINE),
            quote: Style::new().fg(rgb(160, 160, 160)).add_modifier(Modifier::ITALIC),
            quote_bar: Style::new().fg(accent),
            list_bullet: Style::new().fg(accent),
            hr: Style::new().fg(rgb(90, 90, 90)),
            code_border: Style::new().fg(rgb(100, 100, 100)),

            keyword: Style::new().fg(rgb(197, 134, 255)),
            string: Style::new().fg(rgb(150, 205, 120)),
            comment: Style::new().fg(rgb(110, 110, 110)).add_modifier(Modifier::ITALIC),
            number: Style::new().fg(rgb(120, 200, 220)),
            type_name: Style::new().fg(rgb(120, 200, 170)),
            fn_name: Style::new().fg(rgb(220, 220, 170)),

            diff_add: Style::new()
                .fg(rgb(150, 220, 150))
                .bg(rgb(28, 48, 30)),
            diff_del: Style::new()
                .fg(rgb(230, 140, 140))
                .bg(rgb(56, 30, 30)),
            diff_hunk: Style::new().fg(rgb(120, 170, 255)),
            diff_meta: Style::new().fg(rgb(150, 150, 150)).add_modifier(Modifier::BOLD),
        }
    }
}
