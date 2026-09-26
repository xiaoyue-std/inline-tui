//! Rectangular areas and simple constraint-based layout.

/// A rectangular area on screen (coordinates relative to the render buffer, 0-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Margin {
    pub horizontal: u16,
    pub vertical: u16,
}

impl Margin {
    pub fn new(h: u16, v: u16) -> Margin {
        Margin { horizontal: h, vertical: v }
    }
}

impl Rect {
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Rect {
        Rect { x, y, width, height }
    }

    pub const fn left(&self) -> u16 {
        self.x
    }

    pub const fn right(&self) -> u16 {
        self.x + self.width
    }

    pub const fn top(&self) -> u16 {
        self.y
    }

    pub const fn bottom(&self) -> u16 {
        self.y + self.height
    }

    pub fn area(&self) -> u32 {
        self.width as u32 * self.height as u32
    }

    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub fn contains(&self, x: u16, y: u16) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    /// Inner area after shrinking by the margin (at least 0 size).
    pub fn inner(&self, margin: Margin) -> Rect {
        let w = self.width.saturating_sub(margin.horizontal.saturating_mul(2));
        let h = self.height.saturating_sub(margin.vertical.saturating_mul(2));
        Rect {
            x: self.x + margin.horizontal.min(self.width),
            y: self.y + margin.vertical.min(self.height),
            width: w,
            height: h,
        }
    }

    pub fn intersection(&self, other: Rect) -> Rect {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = self.right().min(other.right());
        let y2 = self.bottom().min(other.bottom());
        Rect {
            x: x1,
            y: y1,
            width: x2.saturating_sub(x1),
            height: y2.saturating_sub(y1),
        }
    }
}

/// Layout constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Constraint {
    /// Fixed number of rows/columns.
    Length(u16),
    /// At least this much (participates in leftover space allocation, weighted by its value).
    Min(u16),
    /// At most this much.
    Max(u16),
    /// Percentage of the total.
    Percentage(u16),
    /// Shares leftover space by weight.
    Fill(u16),
}

/// Vertical split: allocates the height of `area` top-to-bottom following the constraints.
///
/// Allocation rules: Length/Percentage/Max take their fixed values first; the
/// remaining space is distributed between Min and Fill weighted by their values
/// (Min is guaranteed to receive its minimum).
pub fn vsplit(area: Rect, constraints: &[Constraint]) -> Vec<Rect> {
    let heights = solve(area.height, constraints);
    let mut y = area.y;
    heights
        .into_iter()
        .map(|h| {
            let r = Rect { x: area.x, y, width: area.width, height: h };
            y += h;
            r
        })
        .collect()
}

/// Horizontal split: allocates the width of `area` left-to-right following the constraints.
pub fn hsplit(area: Rect, constraints: &[Constraint]) -> Vec<Rect> {
    let widths = solve(area.width, constraints);
    let mut x = area.x;
    widths
        .into_iter()
        .map(|w| {
            let r = Rect { x, y: area.y, width: w, height: area.height };
            x += w;
            r
        })
        .collect()
}

/// Constraint solver: distributes `total` into segment sizes according to the constraints
/// (the engine behind `vsplit`/`hsplit`).
pub(crate) fn solve(total: u16, constraints: &[Constraint]) -> Vec<u16> {
    let mut sizes = vec![0u16; constraints.len()];
    let mut used: u32 = 0;

    // Round 1: fixed-style constraints
    for (i, c) in constraints.iter().enumerate() {
        let v = match c {
            Constraint::Length(n) => *n,
            Constraint::Percentage(p) => (total as u32 * *p as u32 / 100) as u16,
            Constraint::Max(n) => *n,
            _ => 0,
        };
        sizes[i] = v.min(total);
        used += sizes[i] as u32;
    }

    // When fixed constraints exceed the total, shrink from back to front
    if used > total as u32 {
        let mut excess = used - total as u32;
        for i in (0..sizes.len()).rev() {
            if excess == 0 {
                break;
            }
            if matches!(constraints[i], Constraint::Length(_) | Constraint::Max(_)) {
                let cut = (sizes[i] as u32).min(excess);
                sizes[i] -= cut as u16;
                excess -= cut;
            }
        }
    }

    let remainder = (total as i32 - used as i32).max(0) as u32;

    // Round 2: distribute the remainder between Min and Fill by weight
    let weights: Vec<u32> = constraints
        .iter()
        .map(|c| match c {
            Constraint::Min(n) => (*n).max(1) as u32,
            Constraint::Fill(n) => (*n).max(1) as u32,
            _ => 0,
        })
        .collect();
    let weight_sum: u32 = weights.iter().sum();

    let mut distributed = 0u32;
    for (i, w) in weights.iter().enumerate() {
        if *w == 0 {
            continue;
        }
        let share = if i == weights.len() - 1 || weights[i + 1..].iter().all(|w| *w == 0) {
            remainder - distributed
        } else {
            remainder
                .checked_mul(*w)
                .and_then(|v| v.checked_div(weight_sum))
                .unwrap_or(0)
        };
        sizes[i] = share.min(u16::MAX as u32) as u16;
        distributed += share;
    }
    // Guarantee Min's lower bound (squeeze fixed-style constraints back-to-front if needed)
    for (i, c) in constraints.iter().enumerate().rev() {
        if let Constraint::Min(n) = c {
            if sizes[i] < *n {
                let need = *n - sizes[i];
                // Borrow from fixed constraints before this one
                for j in (0..i).rev() {
                    if sizes[j] > need {
                        sizes[j] -= need;
                        sizes[i] += need;
                        break;
                    }
                }
            }
        }
    }

    sizes
}
