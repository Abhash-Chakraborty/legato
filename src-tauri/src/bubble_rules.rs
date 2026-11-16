//! Where the desktop bubble sits: its window around the dot, collapsed or
//! opened on hover, kept inside the screen's work area, and snapped to an
//! edge when dropped near one. Pure functions in logical pixels, tested
//! without the app.

use serde::{Deserialize, Serialize};
use specta::Type;

/// The window while only the dot shows, across the edge it sits on: the
/// dot, and room for the level meter it grows into while you talk.
pub const COLLAPSED: (f64, f64) = (40.0, 88.0);
/// The window while it's open on a side: three buttons in a column and the
/// hovered one's label beside them.
pub const EXPANDED: (f64, f64) = (300.0, 168.0);
/// Open at the top or bottom: three buttons in a row, the label under or
/// over them.
pub const EXPANDED_ROW: (f64, f64) = (300.0, 124.0);
/// Half the thin side of the collapsed window: how far the dot's centre is
/// from the edge of its window.
const HALF: f64 = COLLAPSED.0 / 2.0;
/// The gap between the dot's window and the screen's edge.
const GAP: f64 = 2.0;

/// A rectangle in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// The screen edge the bubble sits in the middle of.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Edge {
    Left,
    #[default]
    Right,
    Top,
    Bottom,
}

impl Edge {
    /// Left and right stand the meter and the buttons upright.
    pub fn upright(self) -> bool {
        matches!(self, Edge::Left | Edge::Right)
    }
}

/// Where the bubble's window goes and where the dot is inside it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub window: Rect,
    /// The dot's centre, relative to the window.
    pub dot: (f64, f64),
    pub edge: Edge,
}

/// Keeps the dot's centre inside the area, clear of its edges.
pub fn clamp_center(center: (f64, f64), area: Rect) -> (f64, f64) {
    (
        center.0.clamp(area.x + HALF, area.x + area.w - HALF),
        center.1.clamp(area.y + HALF, area.y + area.h - HALF),
    )
}

/// The edge nearest a point.
pub fn nearest_edge(center: (f64, f64), area: Rect) -> Edge {
    let (x, y) = clamp_center(center, area);
    [
        (x - area.x, Edge::Left),
        (area.x + area.w - x, Edge::Right),
        (y - area.y, Edge::Top),
        (area.y + area.h - y, Edge::Bottom),
    ]
    .into_iter()
    .min_by(|a, b| a.0.total_cmp(&b.0))
    .map_or(Edge::Right, |(_, e)| e)
}

/// The dot's centre in the middle of an edge.
pub fn edge_center(edge: Edge, area: Rect) -> (f64, f64) {
    let (mx, my) = (area.x + area.w / 2.0, area.y + area.h / 2.0);
    match edge {
        Edge::Left => (area.x + HALF + GAP, my),
        Edge::Right => (area.x + area.w - HALF - GAP, my),
        Edge::Top => (mx, area.y + HALF + GAP),
        Edge::Bottom => (mx, area.y + area.h - HALF - GAP),
    }
}

/// The window for a dot centred at `center` near `edge`. Collapsed, it lies
/// along the edge; open, it grows away from the edge, towards the middle of
/// the screen, so the dot stays under the pointer.
pub fn place(center: (f64, f64), edge: Edge, area: Rect, expanded: bool) -> Placement {
    let center = clamp_center(center, area);
    let (w, h) = match (expanded, edge.upright()) {
        (false, true) => COLLAPSED,
        (false, false) => (COLLAPSED.1, COLLAPSED.0),
        (true, true) => EXPANDED,
        (true, false) => EXPANDED_ROW,
    };
    let (x, y) = if !expanded {
        (center.0 - w / 2.0, center.1 - h / 2.0)
    } else {
        match edge {
            // The dot at the window's outer end, the rest towards the middle.
            Edge::Right => (center.0 + HALF - w, center.1 - h / 2.0),
            Edge::Left => (center.0 - HALF, center.1 - h / 2.0),
            Edge::Bottom => (center.0 - w / 2.0, center.1 + HALF - h),
            Edge::Top => (center.0 - w / 2.0, center.1 - HALF),
        }
    };
    let wx = x.clamp(area.x, (area.x + area.w - w).max(area.x));
    let wy = y.clamp(area.y, (area.y + area.h - h).max(area.y));
    Placement {
        window: Rect { x: wx, y: wy, w, h },
        dot: (center.0 - wx, center.1 - wy),
        edge,
    }
}

/// Where a dropped dot settles: the middle of the nearest edge.
pub fn snap(center: (f64, f64), area: Rect) -> (Edge, (f64, f64)) {
    let edge = nearest_edge(center, area);
    (edge, edge_center(edge, area))
}

/// The live transcript box's distance from the screen's edge.
pub const CAPTION_MARGIN: f64 = 16.0;
/// Its distance from the bubble, when the bubble sits on the same edge.
pub const CAPTION_CLEAR: f64 = 8.0;

/// Where the live transcript box goes: `size` big (its width capped to
/// the area), centred along the top or bottom of `area`. When the bubble's
/// window (`bubble`, collapsed) sits on that edge under the box, the box
/// goes just past it instead of covering it; on a side, it's ignored.
pub fn caption_rect(area: Rect, size: (f64, f64), top: bool, bubble: Option<Placement>) -> Rect {
    let w = size.0.min(area.w - 2.0 * CAPTION_MARGIN).max(1.0);
    let h = size.1;
    let x = area.x + (area.w - w) / 2.0;
    let edge = if top { Edge::Top } else { Edge::Bottom };
    let in_the_way =
        bubble.filter(|b| b.edge == edge && b.window.x < x + w && x < b.window.x + b.window.w);
    let y = match (in_the_way, top) {
        (Some(b), true) => b.window.y + b.window.h + CAPTION_CLEAR,
        (Some(b), false) => b.window.y - h - CAPTION_CLEAR,
        (None, true) => area.y + CAPTION_MARGIN,
        (None, false) => area.y + area.h - h - CAPTION_MARGIN,
    };
    Rect { x, y, w, h }
}

/// A fullscreen window may stop this far short of an edge: some viewers
/// (VNC) leave a pixel so Windows doesn't treat them as exclusive fullscreen.
const COVER_SLACK: i32 = 2;

/// Whether a window fills its monitor, as a video, a game, a slideshow or a
/// remote desktop does; the bubble and the resting island step aside for
/// those. `window` and `monitor` are physical (left, top, right, bottom).
pub fn covers(window: (i32, i32, i32, i32), monitor: (i32, i32, i32, i32)) -> bool {
    window.0 <= monitor.0 + COVER_SLACK
        && window.1 <= monitor.1 + COVER_SLACK
        && window.2 >= monitor.2 - COVER_SLACK
        && window.3 >= monitor.3 - COVER_SLACK
}

/// Whether a window sits exactly on its monitor, over the taskbar, rather
/// than hanging past its edges as a maximized window does. Some viewers
/// (TightVNC) keep their title bar in full screen and are only told apart
/// from an ordinary window this way.
pub fn fits(window: (i32, i32, i32, i32), monitor: (i32, i32, i32, i32)) -> bool {
    (window.0 - monitor.0).abs() <= COVER_SLACK
        && (window.1 - monitor.1).abs() <= COVER_SLACK
        && (window.2 - monitor.2).abs() <= COVER_SLACK
        && (window.3 - monitor.3).abs() <= COVER_SLACK
}

/// The island's window along the top edge of a screen, in physical pixels:
/// its left edge for an offset from -1 (against the left edge) through 0
/// (the middle) to 1.
pub fn island_x(screen_x: i32, screen_w: i32, width: i32, offset: f32) -> i32 {
    let free = ((screen_w - width) / 2).max(0);
    let offset = if offset.is_finite() {
        offset.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    screen_x + free + (free as f32 * offset).round() as i32
}

/// The offset a dragged island is left with at `x`: back in the middle
/// when it's dropped within `snap` pixels of it.
pub fn island_offset(screen_x: i32, screen_w: i32, width: i32, x: i32, snap: i32) -> f32 {
    let free = ((screen_w - width) / 2).max(0);
    let from_middle = x - (screen_x + free);
    if free == 0 || from_middle.abs() <= snap {
        return 0.0;
    }
    (from_middle as f32 / free as f32).clamp(-1.0, 1.0)
}

/// A pointer wiggled from side to side over the island, which asks it to
/// get out of the way: three turns inside 900 ms, each after a stroke of
/// at least `stroke` pixels. Fed the pointer's x as it's seen.
#[derive(Default)]
pub struct Shake {
    last_x: Option<i32>,
    /// The way the pointer is going (-1, 1), and how far it has gone.
    dir: i32,
    run: i32,
    /// When the last turns were, in milliseconds.
    turns: [u64; 3],
    count: usize,
}

impl Shake {
    const WITHIN_MS: u64 = 900;

    /// Forgets the strokes so far: the pointer left the island.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Takes the pointer's x at `now_ms`; true when that completes a shake.
    pub fn feed(&mut self, x: i32, now_ms: u64, stroke: i32) -> bool {
        let Some(last) = self.last_x.replace(x) else {
            return false;
        };
        let dx = x - last;
        if dx == 0 {
            return false;
        }
        let dir = dx.signum();
        if dir == self.dir {
            self.run += dx.abs();
            return false;
        }
        // A turn counts when the stroke before it was long enough.
        let turned = self.dir != 0 && self.run >= stroke;
        self.dir = dir;
        self.run = dx.abs();
        if !turned {
            return false;
        }
        self.turns.rotate_left(1);
        self.turns[2] = now_ms;
        self.count = (self.count + 1).min(3);
        if self.count == 3 && now_ms.saturating_sub(self.turns[0]) <= Self::WITHIN_MS {
            self.reset();
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_island_sits_where_it_was_dragged_and_snaps_to_the_middle() {
        // A 650 px window on a 1920 px screen that starts at x = 100.
        assert_eq!(island_x(100, 1920, 650, 0.0), 735);
        assert_eq!(island_x(100, 1920, 650, -1.0), 100);
        assert_eq!(island_x(100, 1920, 650, 1.0), 1370);
        assert_eq!(island_x(100, 1920, 650, 7.0), 1370);
        assert_eq!(island_x(100, 1920, 650, f32::NAN), 735);
        // A screen narrower than the window keeps it at the screen's edge.
        assert_eq!(island_x(0, 500, 650, 0.5), 0);

        assert_eq!(island_offset(100, 1920, 650, 735 + 20, 24), 0.0);
        assert_eq!(island_offset(100, 1920, 650, 100, 24), -1.0);
        let half = island_offset(100, 1920, 650, 735 + 318, 24);
        assert!((half - 0.5).abs() < 0.01, "{half}");
        // And back again lands where it was dropped.
        assert_eq!(island_x(100, 1920, 650, half), 735 + 318);
        assert_eq!(island_offset(0, 500, 650, 40, 24), 0.0);
    }

    #[test]
    fn a_shake_is_three_quick_turns_and_nothing_less() {
        let mut shake = Shake::default();
        // Right, left, right, left: the third turn completes it.
        let mut hit = false;
        let mut x = 500;
        let mut now = 0;
        for stroke in [60, -60, 60, -60] {
            for _ in 0..3 {
                x += stroke / 3;
                now += 33;
                hit |= shake.feed(x, now, 28);
            }
        }
        assert!(hit);

        // Crossing the island once, however far, is no shake.
        let mut shake = Shake::default();
        assert!(!(0..40).any(|i| shake.feed(300 + i * 12, i as u64 * 33, 28)));

        // A hand that trembles a few pixels each way isn't one either.
        let mut shake = Shake::default();
        assert!(!(0..60).any(|i| shake.feed(500 + (i % 2) * 5, i as u64 * 33, 28)));

        // Slow sweeps back and forth: the turns are too far apart.
        let mut shake = Shake::default();
        let mut x = 500;
        let mut slow = false;
        for (n, stroke) in [80, -80, 80, -80, 80].into_iter().enumerate() {
            x += stroke;
            slow |= shake.feed(x, n as u64 * 500, 28);
        }
        assert!(!slow);
    }

    const AREA: Rect = Rect {
        x: 0.0,
        y: 0.0,
        w: 1536.0,
        h: 816.0,
    };

    #[test]
    fn the_caption_clears_a_bubble_on_its_edge_only() {
        let size = (640.0, 112.0);
        let bottom = place(edge_center(Edge::Bottom, AREA), Edge::Bottom, AREA, false);
        let top = place(edge_center(Edge::Top, AREA), Edge::Top, AREA, false);
        let side = place(edge_center(Edge::Right, AREA), Edge::Right, AREA, false);

        // On its own, a margin from the edge, centred.
        let alone = caption_rect(AREA, size, false, Some(side));
        assert_eq!(alone, caption_rect(AREA, size, false, None));
        assert_eq!(alone.x, (AREA.w - 640.0) / 2.0);
        assert_eq!(alone.y + alone.h, AREA.h - CAPTION_MARGIN);
        assert_eq!(caption_rect(AREA, size, true, Some(side)).y, CAPTION_MARGIN);

        // Over the bubble at the bottom, under it at the top.
        let above = caption_rect(AREA, size, false, Some(bottom));
        assert_eq!(above.y + above.h, bottom.window.y - CAPTION_CLEAR);
        let below = caption_rect(AREA, size, true, Some(top));
        assert_eq!(below.y, top.window.y + top.window.h + CAPTION_CLEAR);

        // A bubble on the other edge doesn't move it.
        assert_eq!(
            caption_rect(AREA, size, true, Some(bottom)).y,
            CAPTION_MARGIN
        );

        // A narrow screen narrows the box.
        let narrow = Rect { w: 500.0, ..AREA };
        assert_eq!(
            caption_rect(narrow, size, false, None).w,
            500.0 - 2.0 * CAPTION_MARGIN
        );
    }

    #[test]
    fn collapsed_lies_along_its_edge() {
        let side = place(edge_center(Edge::Right, AREA), Edge::Right, AREA, false);
        assert_eq!((side.window.w, side.window.h), (40.0, 88.0));
        assert_eq!(side.dot, (20.0, 44.0));
        assert_eq!(side.window.x + side.window.w, AREA.w - 2.0);
        let top = place(edge_center(Edge::Top, AREA), Edge::Top, AREA, false);
        assert_eq!((top.window.w, top.window.h), (88.0, 40.0));
        assert_eq!(top.window.y, 2.0);
        assert_eq!(top.window.x + top.dot.0, AREA.w / 2.0);
    }

    #[test]
    fn opening_grows_towards_the_middle_and_keeps_the_dot_still() {
        for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
            let c = edge_center(edge, AREA);
            let p = place(c, edge, AREA, true);
            assert_eq!((p.window.x + p.dot.0, p.window.y + p.dot.1), c, "{edge:?}");
            assert!(p.window.x >= 0.0 && p.window.x + p.window.w <= AREA.w);
            assert!(p.window.y >= 0.0 && p.window.y + p.window.h <= AREA.h);
        }
        let bottom = place(edge_center(Edge::Bottom, AREA), Edge::Bottom, AREA, true);
        assert_eq!(bottom.dot.1, EXPANDED_ROW.1 - 20.0);
    }

    #[test]
    fn drops_settle_in_the_middle_of_the_nearest_edge() {
        assert_eq!(snap((1500.0, 100.0), AREA), (Edge::Right, (1514.0, 408.0)));
        assert_eq!(snap((300.0, 400.0), AREA), (Edge::Left, (22.0, 408.0)));
        assert_eq!(snap((700.0, 30.0), AREA), (Edge::Top, (768.0, 22.0)));
        assert_eq!(snap((700.0, 790.0), AREA), (Edge::Bottom, (768.0, 794.0)));
        // Off the screen comes back to the nearest edge.
        assert_eq!(snap((-50.0, 300.0), AREA).0, Edge::Left);
    }

    #[test]
    fn fullscreen_windows_cover_the_monitor() {
        let monitor = (0, 0, 1920, 1080);
        assert!(covers((0, 0, 1920, 1080), monitor));
        assert!(covers((-8, -8, 1928, 1088), monitor));
        assert!(!covers((0, 0, 1920, 1040), monitor));
        // A viewer a pixel short of the edges is still fullscreen.
        assert!(covers((1, 1, 1919, 1079), monitor));
        assert!(!covers((0, 0, 1900, 1080), monitor));
    }

    #[test]
    fn only_a_window_on_the_monitor_exactly_fits_it() {
        let monitor = (0, 0, 1920, 1080);
        // TightVNC in full screen, title bar and all.
        assert!(fits((0, 0, 1920, 1080), monitor));
        assert!(fits((1, 1, 1919, 1079), monitor));
        // Maximized windows hang past the edges by their frame.
        assert!(!fits((-9, -9, 1929, 1089), monitor));
        // Above the taskbar, or snapped to a side.
        assert!(!fits((0, 0, 1920, 1020), monitor));
        assert!(!fits((0, 0, 1862, 1080), monitor));
        assert!(fits((1920, 0, 3840, 1080), (1920, 0, 3840, 1080)));
    }
}
