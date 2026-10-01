//! Widget layer: the `Widget` trait and built-in widgets.

pub mod anim;
pub mod block;
pub mod checkbox;
pub mod collapsible;
pub mod diff;
pub mod gauge;
pub mod input;
pub mod list;
pub mod menu;
pub mod paragraph;
pub mod scrollbar;
pub mod sparkline;
pub mod spinner;
pub mod statusbar;
pub mod table;
pub mod tabs;
pub mod viewport;

pub use anim::{
    lerp_rgb, pulse_color, wave_spans, LoadingDots, ProgressBar, Shimmer, Ticker, Typewriter, Wave,
};
pub use block::{Block, BorderType, Borders};
pub use checkbox::{Checkbox, RadioGroup};
pub use collapsible::Collapsible;
pub use diff::{diff_lines, DiffView};
pub use gauge::Gauge;
pub use input::{Editor, InputAction};
pub use list::{List, ListState};
pub use menu::Menu;
pub use paragraph::Paragraph;
pub use scrollbar::{ScrollOrientation, Scrollbar};
pub use sparkline::Sparkline;
pub use spinner::Spinner;
pub use statusbar::StatusBar;
pub use table::{Table, TableState};
pub use tabs::Tabs;
pub use viewport::{ScrollState, Viewport};

/// Widget: renders itself into the given area of a buffer.
pub trait Widget {
    fn render(self, area: crate::layout::Rect, buf: &mut crate::buffer::Buffer);
}
