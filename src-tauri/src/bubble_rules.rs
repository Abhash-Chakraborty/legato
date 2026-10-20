//! Where the desktop bubble sits: its window around the dot, collapsed or
//! opened on hover, kept inside the screen's work area, and snapped to an
//! edge when dropped near one. Pure functions in logical pixels, tested
//! without the app.

use serde::{Deserialize, Serialize};
use specta::Type;

/// The window while only the dot shows: the dot and room for its ring.
pub const COLLAPSED: (f64, f64) = (40.0, 40.0);
/// The window while it's open: three buttons in a column and the label of
/// the one under the pointer beside them.
pub const EXPANDED: (f64, f64) = (248.0, 168.0);
/// Half the collapsed window: how far the dot's centre is from its edges.
const HALF: f64 = COLLAPSED.0 / 2.0;
/// Dropped this close to a side, the dot moves flush to it.
pub const SNAP: f64 = 48.0;
/// The gap between a snapped dot's window and the side.
const GAP: f64 = 2.0;

/// A rectangle in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Which side of the buttons the labels go: towards the middle of the
/// screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum LabelSide {
    Left,
    Right,
}

/// Where the bubble's window goes and where the dot is inside it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub window: Rect,
    /// The dot's centre, relative to the window.
    pub dot: (f64, f64),
    pub labels: LabelSide,
}

/// Keeps the dot's centre where its collapsed window fits in the area.
pub fn clamp_center(center: (f64, f64), area: Rect) -> (f64, f64) {
    (
        center.0.clamp(area.x + HALF, area.x + area.w - HALF),
        center.1.clamp(area.y + HALF, area.y + area.h - HALF),
    )
}

/// The window for a dot centred at `center`. Open, it grows towards the
/// middle of the screen, so the dot stays under the pointer, and is pushed
/// back inside the area where it would cross an edge.
pub fn place(center: (f64, f64), area: Rect, expanded: bool) -> Placement {
    let center = clamp_center(center, area);
    let labels = if center.0 > area.x + area.w / 2.0 {
        LabelSide::Left
    } else {
        LabelSide::Right
    };
    let (w, h) = if expanded { EXPANDED } else { COLLAPSED };
    let x = match labels {
        // The dot sits at the right end of the window, labels to its left.
        LabelSide::Left => center.0 + HALF - w,
        LabelSide::Right => center.0 - HALF,
    };
    let y = center.1 - h / 2.0;
    let wx = x.clamp(area.x, (area.x + area.w - w).max(area.x));
    let wy = y.clamp(area.y, (area.y + area.h - h).max(area.y));
    Placement {
        window: Rect { x: wx, y: wy, w, h },
        dot: (center.0 - wx, center.1 - wy),
        labels,
    }
}

/// Where a dropped dot settles: flush against a side when it's near one,
/// otherwise where it was let go, inside the area.
pub fn snap(center: (f64, f64), area: Rect) -> (f64, f64) {
    let (mut x, y) = clamp_center(center, area);
    let left = area.x + HALF + GAP;
    let right = area.x + area.w - HALF - GAP;
    if x - left < SNAP {
        x = left;
    } else if right - x < SNAP {
        x = right;
    }
    (x, y)
}

/// The dot's centre as fractions of the area, so it stays in the same spot
/// when the resolution or scaling changes.
pub fn to_fraction(center: (f64, f64), area: Rect) -> (f64, f64) {
    (
        ((center.0 - area.x) / area.w).clamp(0.0, 1.0),
        ((center.1 - area.y) / area.h).clamp(0.0, 1.0),
    )
}

pub fn from_fraction(fraction: (f64, f64), area: Rect) -> (f64, f64) {
    clamp_center(
        (area.x + fraction.0 * area.w, area.y + fraction.1 * area.h),
        area,
    )
}

/// Where the bubble starts: against the right side, a little below the
/// middle, where Windows puts nothing.
pub const DEFAULT_FRACTION: (f64, f64) = (1.0, 0.62);

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
    fn collapsed_window_is_centred_on_the_dot() {
        let p = place((700.0, 400.0), AREA, false);
        assert_eq!(
            p.window,
            Rect {
                x: 680.0,
                y: 380.0,
                w: 40.0,
                h: 40.0
            }
        );
        assert_eq!(p.dot, (20.0, 20.0));
    }

    #[test]
    fn opening_grows_towards_the_middle_and_keeps_the_dot_still() {
        let right = place((1514.0, 500.0), AREA, true);
        assert_eq!(right.labels, LabelSide::Left);
        assert_eq!(right.window.x + right.dot.0, 1514.0);
        assert_eq!(right.window.y + right.dot.1, 500.0);
        assert!(right.window.x + right.window.w <= AREA.w);

        let left = place((22.0, 500.0), AREA, true);
        assert_eq!(left.labels, LabelSide::Right);
        assert_eq!(left.window.x, 2.0);
    }

    #[test]
    fn an_open_window_near_the_top_stays_on_screen() {
        let p = place((1514.0, 25.0), AREA, true);
        assert_eq!(p.window.y, 0.0);
        assert_eq!(p.window.y + p.dot.1, 25.0);
    }

    #[test]
    fn drops_near_a_side_snap_to_it() {
        assert_eq!(snap((1500.0, 300.0), AREA), (1536.0 - 22.0, 300.0));
        assert_eq!(snap((40.0, 300.0), AREA), (22.0, 300.0));
        assert_eq!(snap((700.0, 300.0), AREA), (700.0, 300.0));
        // Off the screen comes back inside it.
        assert_eq!(snap((-50.0, 2000.0), AREA), (22.0, 816.0 - 20.0));
    }

    #[test]
    fn fractions_survive_a_new_resolution() {
        let f = to_fraction((1514.0, 506.0), AREA);
        let bigger = Rect {
            x: 0.0,
            y: 0.0,
            w: 2560.0,
            h: 1400.0,
        };
        let (x, y) = from_fraction(f, bigger);
        assert!((x - 2560.0 * 1514.0 / 1536.0).abs() < 0.01);
        assert!((y - 1400.0 * 506.0 / 816.0).abs() < 0.01);
        assert_eq!(from_fraction(DEFAULT_FRACTION, AREA).0, 1536.0 - 20.0);
    }

    #[test]
    fn fullscreen_windows_cover_the_monitor() {
        let monitor = (0, 0, 1920, 1080);
        assert!(covers((0, 0, 1920, 1080), monitor));
        assert!(covers((-8, -8, 1928, 1088), monitor));
        assert!(!covers((0, 0, 1920, 1040), monitor));
    }
}
