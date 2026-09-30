//! Theme: maps semantic roles to styles; default dark theme (orange accent).

use crate::style::{Color, Modifier, Style};
use crate::widgets::anim::lerp_rgb;

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

impl Theme {
    /// Derives a theme variant from a custom accent color.
    ///
    /// Every accent-bearing role — focused border, accent text, spinner, hint
    /// keys, quote bar, list bullets, inline-code highlight — is re-derived
    /// from `accent`. Semantic roles (error/success/warning, syntax colors,
    /// diff colors) keep this theme's values, so the result stays coherent.
    ///
    /// ```
    /// use inline_tui::style::Color;
    /// use inline_tui::theme::Theme;
    ///
    /// let theme = Theme::default().with_accent(Color::Rgb(80, 160, 255));
    /// assert_eq!(theme.accent.fg, Some(Color::Rgb(80, 160, 255)));
    /// assert_eq!(theme.spinner.fg, Some(Color::Rgb(80, 160, 255)));
    /// ```
    pub fn with_accent(&self, accent: Color) -> Theme {
        let mut t = self.clone();
        let accent_st = Style::new().fg(accent);
        t.border_focused = accent_st;
        t.accent = accent_st;
        t.spinner = accent_st;
        t.status_hint_key = accent_st.add_modifier(Modifier::BOLD);
        t.quote_bar = accent_st;
        t.list_bullet = accent_st;
        // Inline code reads as a lighter shade of the accent.
        t.inline_code = Style::new().fg(lerp_rgb(accent, Color::White, 0.45));
        t
    }
}
