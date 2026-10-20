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

/// Whether a window fills its monitor, as a video, a game or a slideshow
/// does; the bubble steps aside for those. `window` and `monitor` are
/// physical (left, top, right, bottom).
pub fn covers(window: (i32, i32, i32, i32), monitor: (i32, i32, i32, i32)) -> bool {
    window.0 <= monitor.0 && window.1 <= monitor.1 && window.2 >= monitor.2 && window.3 >= monitor.3
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: Rect = Rect {
        x: 0.0,
        y: 0.0,
        w: 1536.0,
        h: 816.0,
    };

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
    }
}
