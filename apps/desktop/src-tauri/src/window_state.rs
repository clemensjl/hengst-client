//! Window size and position: the pure size clamps of DESIGN.md 7.1 and the saved state in
//! `<data root>/window.json`.

use hengst_core::paths::{unique_temp, Paths};
use hengst_core::Result;
use serde::{Deserialize, Serialize};

const FORMAT: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

pub const MIN_SIZE: Size = Size {
    width: 960,
    height: 600,
};
pub const DEFAULT_SIZE: Size = Size {
    width: 1120,
    height: 720,
};

/// Saved window state. `width` and `height` are the logical inner size, `x` and `y` the outer
/// position in physical pixels as Tauri reports it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowState {
    pub format: u32,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub maximized: bool,
}

fn per_axis(a: Size, b: Size, f: impl Fn(u32, u32) -> u32) -> Size {
    Size {
        width: f(a.width, b.width),
        height: f(a.height, b.height),
    }
}

/// What is left of the work area for the client area.
fn available(work: Size, frame: Size) -> Size {
    per_axis(work, frame, u32::saturating_sub)
}

/// `min(960 x 600, work area minus frame)` per axis.
pub fn min_size(work: Size, frame: Size) -> Size {
    per_axis(MIN_SIZE, available(work, frame), u32::min)
}

/// Per axis the maximum of the minimum and `min(1120 x 720, 90 % of (work area minus frame))`.
pub fn default_size(work: Size, frame: Size) -> Size {
    let avail = available(work, frame);
    let ninety = Size {
        width: avail.width * 9 / 10,
        height: avail.height * 9 / 10,
    };
    per_axis(
        min_size(work, frame),
        per_axis(DEFAULT_SIZE, ninety, u32::min),
        u32::max,
    )
}

/// Per axis between the minimum size and the work area minus the frame.
pub fn clamp_size(requested: Size, work: Size, frame: Size) -> Size {
    let min = min_size(work, frame);
    let max = available(work, frame);
    per_axis(per_axis(requested, min, u32::max), max, u32::min)
}

/// `--window-size` wins, then the saved size (only when `restore`), then the default; the first
/// two are clamped.
pub fn startup_size(
    arg: Option<Size>,
    saved: Option<Size>,
    restore: bool,
    work: Size,
    frame: Size,
) -> Size {
    match (arg, saved) {
        (Some(arg), _) => clamp_size(arg, work, frame),
        (None, Some(saved)) if restore => clamp_size(saved, work, frame),
        _ => default_size(work, frame),
    }
}

/// Nothing is restored (or saved) when the size was given on the command line or in e2e runs, so
/// such runs never depend on an earlier run.
pub fn should_restore(size_arg_given: bool, e2e: bool) -> bool {
    !size_arg_given && !e2e
}

/// The window overlaps one of the monitors (`x, y, width, height`) by at least 100 x 50 px. The
/// window's `x`, `y`, `width` and `height` and the monitor rectangles must use the same unit.
pub fn fits_on_a_monitor(s: &WindowState, monitors: &[(i32, i32, u32, u32)]) -> bool {
    monitors.iter().any(|&(mx, my, mw, mh)| {
        let overlap = |a: i64, alen: i64, b: i64, blen: i64| (a + alen).min(b + blen) - a.max(b);
        overlap(s.x.into(), s.width.into(), mx.into(), mw.into()) >= 100
            && overlap(s.y.into(), s.height.into(), my.into(), mh.into()) >= 50
    })
}

/// A monitor as the placement code sees it: the work area in physical pixels and the scale
/// factor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MonitorArea {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
}

impl MonitorArea {
    /// The work area in logical pixels.
    pub fn work(&self) -> Size {
        Size {
            width: (f64::from(self.width) / self.scale).round() as u32,
            height: (f64::from(self.height) / self.scale).round() as u32,
        }
    }

    fn outer_physical(&self, inner: Size, frame: Size) -> (u32, u32) {
        (
            (f64::from(inner.width + frame.width) * self.scale).round() as u32,
            (f64::from(inner.height + frame.height) * self.scale).round() as u32,
        )
    }
}

/// Where a window goes: the monitor, the logical inner size and the physical outer position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub monitor: usize,
    pub inner: Size,
    pub x: i32,
    pub y: i32,
}

/// Moves an outer rectangle into the work area of `monitor`, so the title bar can always be
/// reached. A window larger than the work area sits at its top-left corner.
pub fn clamp_position(x: i32, y: i32, outer: (u32, u32), monitor: &MonitorArea) -> (i32, i32) {
    let fit = |pos: i32, start: i32, work: u32, len: u32| -> i32 {
        let last = (i64::from(start) + i64::from(work) - i64::from(len)).max(i64::from(start));
        i64::from(pos).clamp(i64::from(start), last) as i32
    };
    (
        fit(x, monitor.x, monitor.width, outer.0),
        fit(y, monitor.y, monitor.height, outer.1),
    )
}

/// First start without saved state: the start size (or the clamped `--window-size`) centered in the
/// work area of `monitors[index]`. `frame` is the window frame measured on that monitor.
pub fn first_start(
    monitors: &[MonitorArea],
    index: usize,
    arg: Option<Size>,
    frame: Size,
) -> Placement {
    let monitor = &monitors[index];
    let inner = startup_size(arg, None, false, monitor.work(), frame);
    let (w, h) = monitor.outer_physical(inner, frame);
    let centered_x = monitor.x + ((i64::from(monitor.width) - i64::from(w)) / 2) as i32;
    let centered_y = monitor.y + ((i64::from(monitor.height) - i64::from(h)) / 2) as i32;
    let (x, y) = clamp_position(centered_x, centered_y, (w, h), monitor);
    Placement {
        monitor: index,
        inner,
        x,
        y,
    }
}

/// The monitor a saved window lands on: the one it overlaps most (by the 100 x 50 px rule of
/// `fits_on_a_monitor`, with the size scaled for each monitor). `None` when it is on no connected
/// monitor any more.
pub fn restore_monitor(s: &WindowState, frame: Size, monitors: &[MonitorArea]) -> Option<usize> {
    let overlap = |m: &MonitorArea| -> Option<i64> {
        let inner = Size {
            width: s.width,
            height: s.height,
        };
        let (w, h) = m.outer_physical(inner, frame);
        let rect = WindowState {
            width: w,
            height: h,
            ..s.clone()
        };
        let area = (m.x, m.y, m.width, m.height);
        fits_on_a_monitor(&rect, &[area]).then(|| {
            let side = |a: i64, alen: i64, b: i64, blen: i64| (a + alen).min(b + blen) - a.max(b);
            side(s.x.into(), w.into(), m.x.into(), m.width.into())
                * side(s.y.into(), h.into(), m.y.into(), m.height.into())
        })
    };
    monitors
        .iter()
        .enumerate()
        .filter_map(|(i, m)| overlap(m).map(|a| (i, a)))
        .max_by_key(|&(_, a)| a)
        .map(|(i, _)| i)
}

/// A saved window brought onto `monitors[index]`: the saved size clamped like at any start, the
/// position pulled into the work area (so a negative y or a half off-screen window is corrected).
pub fn restored(s: &WindowState, monitors: &[MonitorArea], index: usize, frame: Size) -> Placement {
    let monitor = &monitors[index];
    let saved = Size {
        width: s.width,
        height: s.height,
    };
    let inner = startup_size(None, Some(saved), true, monitor.work(), frame);
    let outer = monitor.outer_physical(inner, frame);
    let (x, y) = clamp_position(s.x, s.y, outer, monitor);
    Placement {
        monitor: index,
        inner,
        x,
        y,
    }
}

/// The last normal (not maximized, not minimized) rectangle of this session.
///
/// Windows reports a move before it reports that the window is maximized or minimized: on
/// maximizing, the `Moved` event already carries the big rectangle while `is_maximized` is still
/// false (measured with Tauri 2.12 on Windows 11). So a rectangle taken at a `Moved` event is
/// provisional, and the `Resized` event that reports the maximized or minimized state takes it
/// back.
#[derive(Clone, Debug)]
pub struct Tracker {
    current: WindowState,
    before: WindowState,
    last_was_move: bool,
}

impl Tracker {
    pub fn new(start: WindowState) -> Tracker {
        Tracker {
            before: start.clone(),
            current: start,
            last_was_move: false,
        }
    }

    fn push(&mut self, now: WindowState, was_move: bool) {
        self.before = std::mem::replace(&mut self.current, now);
        self.last_was_move = was_move;
    }

    /// `now` is the rectangle if the window is neither maximized nor minimized, else `None`.
    pub fn on_moved(&mut self, now: Option<WindowState>) {
        if let Some(now) = now {
            self.push(now, true);
        }
    }

    /// `now` is as for `on_moved`; `special` tells that the window is maximized or minimized.
    pub fn on_resized(&mut self, now: Option<WindowState>, special: bool) {
        match now {
            Some(now) => self.push(now, false),
            None if special && self.last_was_move => {
                self.current = self.before.clone();
                self.last_was_move = false;
            }
            None => {}
        }
    }

    /// What to save when the window closes.
    pub fn saved(&self, maximized: bool) -> WindowState {
        WindowState {
            maximized,
            ..self.current.clone()
        }
    }
}

fn file(paths: &Paths) -> std::path::PathBuf {
    paths.root().join("window.json")
}

/// Missing, unreadable, corrupt or of another `format`: `None`. Never fails.
pub fn load(paths: &Paths) -> Option<WindowState> {
    let bytes = std::fs::read(file(paths)).ok()?;
    let state: WindowState = serde_json::from_slice(&bytes).ok()?;
    (state.format == FORMAT).then_some(state)
}

/// Temp file next to the target, then rename over it.
pub fn save(paths: &Paths, s: &WindowState) -> Result<()> {
    let target = file(paths);
    std::fs::create_dir_all(paths.root())?;
    let tmp = unique_temp(&target);
    std::fs::write(&tmp, serde_json::to_vec_pretty(s)?)?;
    if let Err(e) = std::fs::rename(&tmp, &target) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn sz(width: u32, height: u32) -> Size {
        Size { width, height }
    }

    fn state(width: u32, height: u32, x: i32, y: i32) -> WindowState {
        WindowState {
            format: 1,
            width,
            height,
            x,
            y,
            maximized: false,
        }
    }

    #[test]
    fn window_state_round_trips_and_a_broken_file_counts_as_absent() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let s = state(1120, 720, 40, 30);
        save(&paths, &s).unwrap();
        assert_eq!(load(&paths), Some(s));
        std::fs::write(paths.root().join("window.json"), "{").unwrap();
        assert_eq!(load(&paths), None);
    }

    #[test]
    fn other_formats_and_missing_fields_count_as_absent() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let write = |text: &str| std::fs::write(paths.root().join("window.json"), text).unwrap();
        write(r#"{"format":2,"width":1,"height":1,"x":0,"y":0,"maximized":false}"#);
        assert_eq!(load(&paths), None);
        write(r#"{"format":1}"#);
        assert_eq!(load(&paths), None);
        write("");
        assert_eq!(load(&paths), None);
    }

    #[test]
    fn save_leaves_no_temp_file_behind() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let s = state(960, 600, 0, 0);
        save(&paths, &s).unwrap();
        save(&paths, &s).unwrap();
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn window_file_lives_in_the_data_directory() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        save(&paths, &state(960, 600, 0, 0)).unwrap();
        assert!(dir.path().join("window.json").is_file());
    }

    #[test]
    fn restore_is_skipped_with_a_size_argument_and_in_e2e() {
        assert!(should_restore(false, false));
        assert!(!should_restore(true, false));
        assert!(!should_restore(false, true));
    }

    #[test]
    fn a_window_that_is_off_every_monitor_is_not_restored() {
        let monitors = [(0, 0, 1920, 1080)];
        let on = state(1120, 720, 100, 100);
        assert!(fits_on_a_monitor(&on, &monitors));
        assert!(!fits_on_a_monitor(&state(1120, 720, 5000, 100), &monitors));
        assert!(!fits_on_a_monitor(
            &state(1120, 720, 1920 - 99, 100),
            &monitors
        ));
        assert!(!fits_on_a_monitor(
            &state(1120, 720, 100, 1080 - 49),
            &monitors
        ));
        assert!(fits_on_a_monitor(
            &state(1120, 720, -1000, 100),
            &[(-1920, 0, 1920, 1080)]
        ));
    }

    // Work areas in logical pixels; the test frame is the title bar at 125 %, borders do not count.
    const LARGE: Size = sz(1920, 1040);
    const LAPTOP: Size = sz(1093, 580);
    const TINY: Size = sz(800, 500);
    const FRAME: Size = sz(0, 31);

    #[test]
    fn a_large_screen_keeps_the_nominal_sizes() {
        assert_eq!(min_size(LARGE, FRAME), MIN_SIZE);
        assert_eq!(default_size(LARGE, FRAME), DEFAULT_SIZE);
        assert_eq!(clamp_size(sz(1920, 1080), LARGE, FRAME), sz(1920, 1009));
        assert_eq!(clamp_size(sz(500, 300), LARGE, FRAME), MIN_SIZE);
    }

    #[test]
    fn a_1366_by_768_screen_at_125_percent_still_fits_the_window() {
        assert_eq!(min_size(LAPTOP, FRAME), sz(960, 549));
        // DESIGN.md 7.1: 90 % of (work area minus frame) is 983 x 494; the height is raised to the minimum.
        assert_eq!(default_size(LAPTOP, FRAME), sz(983, 549));
        assert_eq!(clamp_size(DEFAULT_SIZE, LAPTOP, FRAME), sz(1093, 549));
        assert_eq!(clamp_size(MIN_SIZE, LAPTOP, FRAME), sz(960, 549));
    }

    #[test]
    fn a_tiny_work_area_wins_over_the_nominal_minimum() {
        assert_eq!(min_size(TINY, FRAME), sz(800, 469));
        assert_eq!(default_size(TINY, FRAME), sz(800, 469));
        assert_eq!(clamp_size(sz(1920, 1080), TINY, FRAME), sz(800, 469));
    }

    #[test]
    fn a_frame_larger_than_the_work_area_does_not_underflow() {
        let huge = sz(5000, 5000);
        assert_eq!(min_size(TINY, huge), sz(0, 0));
        assert_eq!(default_size(TINY, huge), sz(0, 0));
        assert_eq!(clamp_size(DEFAULT_SIZE, TINY, huge), sz(0, 0));
    }

    #[test]
    fn startup_size_prefers_the_argument_then_the_saved_size_then_the_default() {
        let arg = Some(sz(1920, 1080));
        let saved = Some(sz(1000, 650));
        // The argument wins and is clamped; the saved size is ignored (restore is false then).
        assert_eq!(
            startup_size(arg, saved, false, LAPTOP, FRAME),
            sz(1093, 549)
        );
        assert_eq!(startup_size(None, saved, true, LARGE, FRAME), sz(1000, 650));
        // e2e: nothing is restored
        assert_eq!(startup_size(None, saved, false, LARGE, FRAME), DEFAULT_SIZE);
        // a size saved on a large screen is clamped on a small one
        assert_eq!(
            startup_size(None, Some(sz(1800, 1000)), true, LAPTOP, FRAME),
            sz(1093, 549)
        );
        assert_eq!(startup_size(None, None, true, LAPTOP, FRAME), sz(983, 549));
    }

    fn monitors() -> [MonitorArea; 3] {
        [
            MonitorArea {
                x: 0,
                y: 0,
                width: 2560,
                height: 1392,
                scale: 1.0,
            },
            MonitorArea {
                x: 2560,
                y: -476,
                width: 1200,
                height: 1920,
                scale: 1.5,
            },
            MonitorArea {
                x: -1366,
                y: 100,
                width: 1366,
                height: 728,
                scale: 1.25,
            },
        ]
    }

    fn inside(m: &MonitorArea, p: &Placement, frame: Size) -> bool {
        let (w, h) = m.outer_physical(p.inner, frame);
        let (right, bottom) = (i64::from(p.x) + i64::from(w), i64::from(p.y) + i64::from(h));
        p.x >= m.x
            && p.y >= m.y
            && right <= i64::from(m.x) + i64::from(m.width)
            && bottom <= i64::from(m.y) + i64::from(m.height)
    }

    #[test]
    fn first_start_lies_inside_the_target_work_area_on_every_monitor() {
        let monitors = monitors();
        let frame = sz(13, 33);
        for index in 0..monitors.len() {
            for arg in [
                None,
                Some(sz(960, 600)),
                Some(sz(3000, 2000)),
                Some(sz(10, 10)),
            ] {
                let p = first_start(&monitors, index, arg, frame);
                assert_eq!(p.monitor, index);
                assert!(inside(&monitors[index], &p, frame), "{index} {arg:?} {p:?}");
            }
        }
        // centered where it fits
        let p = first_start(&monitors, 0, None, sz(0, 31));
        assert_eq!((p.x, p.y), ((2560 - 1120) / 2, (1392 - 751) / 2));
    }

    #[test]
    fn clamp_position_pulls_the_title_bar_into_the_work_area() {
        let m = monitors()[0];
        assert_eq!(clamp_position(100, -30, (1000, 700), &m), (100, 0));
        assert_eq!(clamp_position(-400, 50, (1000, 700), &m), (0, 50));
        // half off the right and bottom edges
        assert_eq!(clamp_position(2260, 1100, (1000, 700), &m), (1560, 692));
        // larger than the work area: top-left corner
        assert_eq!(clamp_position(50, 50, (3000, 2000), &m), (0, 0));
        let second = monitors()[1];
        assert_eq!(
            clamp_position(2600, -900, (900, 900), &second),
            (2600, -476)
        );
    }

    #[test]
    fn a_saved_window_is_corrected_onto_its_monitor_or_dropped_when_the_monitor_is_gone() {
        let monitors = monitors();
        let frame = sz(13, 33);
        // negative y on the main monitor: same monitor, y raised to the work area
        let s = state(1120, 720, 100, -200);
        let i = restore_monitor(&s, frame, &monitors).unwrap();
        assert_eq!(i, 0);
        let p = restored(&s, &monitors, i, frame);
        assert_eq!((p.x, p.y), (100, 0));
        // half off-screen to the right: pulled back in
        let s = state(1120, 720, 2200, 300);
        let i = restore_monitor(&s, frame, &monitors).unwrap();
        assert!(inside(
            &monitors[i],
            &restored(&s, &monitors, i, frame),
            frame
        ));
        // a window on the second monitor (scale 1.5) stays there and fits
        let s = state(1000, 650, 2600, -400);
        let i = restore_monitor(&s, frame, &monitors).unwrap();
        assert_eq!(i, 1);
        assert!(inside(
            &monitors[i],
            &restored(&s, &monitors, i, frame),
            frame
        ));
        // the monitor is gone
        let only_main = [monitors[0]];
        assert_eq!(
            restore_monitor(&state(1120, 720, 2700, 100), frame, &only_main),
            None
        );
        assert_eq!(
            restore_monitor(&state(1120, 720, -1300, 300), frame, &only_main),
            None
        );
        // a size saved on a large monitor is clamped on a small one
        let s = state(1800, 1000, -1300, 150);
        let i = restore_monitor(&s, frame, &monitors).unwrap();
        assert_eq!(i, 2);
        assert!(inside(
            &monitors[i],
            &restored(&s, &monitors, i, frame),
            frame
        ));
    }

    #[test]
    fn a_maximized_window_saves_the_normal_rectangle_of_this_session() {
        let normal = state(1000, 650, 300, 200);
        let big = state(2560, 1369, -8, -8);
        let mut t = Tracker::new(state(1120, 720, 712, 316));
        // the user drags and resizes the window
        t.on_moved(Some(state(1120, 720, 400, 100)));
        t.on_resized(Some(normal.clone()), false);
        t.on_moved(Some(state(1000, 650, 300, 200)));
        // maximize: Moved still sees a "normal" window with the big rectangle, Resized knows better
        t.on_moved(Some(big.clone()));
        t.on_resized(None, true);
        let saved = t.saved(true);
        assert!(saved.maximized);
        assert_eq!(
            (saved.width, saved.height, saved.x, saved.y),
            (1000, 650, 300, 200)
        );
        // restoring reports the normal rectangle again
        t.on_moved(None);
        t.on_resized(Some(normal.clone()), false);
        assert_eq!(t.saved(false).width, 1000);
        assert!(!t.saved(false).maximized);
    }

    #[test]
    fn a_minimized_window_is_never_saved_as_the_rectangle() {
        let mut t = Tracker::new(state(1000, 650, 300, 200));
        t.on_moved(Some(state(160, 28, -32000, -32000)));
        t.on_resized(None, true);
        let saved = t.saved(false);
        assert_eq!((saved.x, saved.y), (300, 200));
        // events while minimized or maximized change nothing
        t.on_moved(None);
        t.on_resized(None, true);
        assert_eq!(t.saved(false).x, 300);
    }

    #[test]
    fn a_plain_move_is_kept() {
        let mut t = Tracker::new(state(1000, 650, 300, 200));
        t.on_moved(Some(state(1000, 650, 350, 220)));
        assert_eq!((t.saved(false).x, t.saved(false).y), (350, 220));
    }
}
