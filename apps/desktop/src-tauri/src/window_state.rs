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
}
