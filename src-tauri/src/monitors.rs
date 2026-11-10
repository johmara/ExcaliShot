//! Geometry helpers: map monitor rectangles and overlay selections onto the
//! pixels of the single desktop image returned by the capture backend.
//!
//! The image-to-desktop scale is *derived* from the image size versus the
//! bounding box of the monitor rectangles, so it stays correct whether the
//! compositor reports logical or physical monitor coordinates.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn right(&self) -> i64 {
        self.x as i64 + self.w as i64
    }
    pub fn bottom(&self) -> i64 {
        self.y as i64 + self.h as i64
    }
}

/// Smallest rectangle containing all monitors. `None` if there are none.
pub fn bounding_box(monitors: &[Rect]) -> Option<Rect> {
    let first = monitors.first()?;
    let (mut l, mut t, mut r, mut b) = (first.x as i64, first.y as i64, first.right(), first.bottom());
    for m in &monitors[1..] {
        l = l.min(m.x as i64);
        t = t.min(m.y as i64);
        r = r.max(m.right());
        b = b.max(m.bottom());
    }
    Some(Rect { x: l as i32, y: t as i32, w: (r - l) as u32, h: (b - t) as u32 })
}

/// Rectangle of `monitor` in the pixels of a desktop image of `image_w` x `image_h`
/// that covers `bbox`. Result is clamped to the image.
pub fn monitor_in_image(monitor: Rect, bbox: Rect, image_w: u32, image_h: u32) -> Rect {
    let sx = image_w as f64 / bbox.w as f64;
    let sy = image_h as f64 / bbox.h as f64;
    let left = ((monitor.x as i64 - bbox.x as i64) as f64 * sx).round();
    let top = ((monitor.y as i64 - bbox.y as i64) as f64 * sy).round();
    let right = ((monitor.right() - bbox.x as i64) as f64 * sx).round();
    let bottom = ((monitor.bottom() - bbox.y as i64) as f64 * sy).round();
    clamp_rect(left, top, right, bottom, image_w, image_h)
}

/// Map a selection made in an overlay's CSS pixels (`sel` = x, y, w, h, origin at the
/// overlay's top-left, overlay is `overlay_w` x `overlay_h` CSS px) to image pixels,
/// given the monitor's rectangle in the image. Negative width/height (dragging
/// up/left) are normalised. Result is clamped to the monitor rectangle.
pub fn selection_in_image(
    monitor_in_img: Rect,
    overlay_w: f64,
    overlay_h: f64,
    sel: (f64, f64, f64, f64),
) -> Option<Rect> {
    if overlay_w <= 0.0 || overlay_h <= 0.0 {
        return None;
    }
    let (x, y, w, h) = sel;
    let (x0, x1) = if w < 0.0 { (x + w, x) } else { (x, x + w) };
    let (y0, y1) = if h < 0.0 { (y + h, y) } else { (y, y + h) };
    let sx = monitor_in_img.w as f64 / overlay_w;
    let sy = monitor_in_img.h as f64 / overlay_h;
    let mx = monitor_in_img.x as f64;
    let my = monitor_in_img.y as f64;
    let r = clamp_rect(
        (mx + x0 * sx).round(),
        (my + y0 * sy).round(),
        (mx + x1 * sx).round(),
        (my + y1 * sy).round(),
        monitor_in_img.right() as u32,
        monitor_in_img.bottom() as u32,
    );
    // Also clamp to the monitor's left/top edge, then reject empty selections.
    let left = r.x.max(monitor_in_img.x);
    let top = r.y.max(monitor_in_img.y);
    let w = (r.right() - left as i64).max(0) as u32;
    let h = (r.bottom() - top as i64).max(0) as u32;
    (w > 0 && h > 0).then_some(Rect { x: left, y: top, w, h })
}

fn clamp_rect(l: f64, t: f64, r: f64, b: f64, max_w: u32, max_h: u32) -> Rect {
    let l = l.clamp(0.0, max_w as f64);
    let t = t.clamp(0.0, max_h as f64);
    let r = r.clamp(l, max_w as f64);
    let b = b.clamp(t, max_h as f64);
    Rect { x: l as i32, y: t as i32, w: (r - l) as u32, h: (b - t) as u32 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: i32, y: i32, w: u32, h: u32) -> Rect {
        Rect { x, y, w, h }
    }

    // The user's real layout from the 2026-11-09 spike.
    fn real_layout() -> [Rect; 3] {
        [r(4320, 0, 1080, 1920), r(2400, 366, 1920, 1080), r(0, 366, 2400, 1600)]
    }

    #[test]
    fn bounding_box_matches_portal_image() {
        let b = bounding_box(&real_layout()).unwrap();
        assert_eq!(b, r(0, 0, 5400, 1966));
    }

    #[test]
    fn bounding_box_handles_negative_origin_and_empty() {
        assert_eq!(bounding_box(&[]), None);
        let b = bounding_box(&[r(-1920, 0, 1920, 1080), r(0, -200, 1000, 500)]).unwrap();
        assert_eq!(b, r(-1920, -200, 2920, 1280));
    }

    #[test]
    fn monitor_in_image_scale_one() {
        let l = real_layout();
        let b = bounding_box(&l).unwrap();
        assert_eq!(monitor_in_image(l[1], b, 5400, 1966), r(2400, 366, 1920, 1080));
    }

    #[test]
    fn monitor_in_image_derives_scale_when_image_is_2x() {
        let l = [r(0, 0, 1000, 500), r(1000, 100, 800, 400)];
        let b = bounding_box(&l).unwrap(); // 1800 x 500
        assert_eq!(monitor_in_image(l[1], b, 3600, 1000), r(2000, 200, 1600, 800));
    }

    #[test]
    fn monitor_in_image_negative_origin() {
        let l = [r(-1920, 0, 1920, 1080), r(0, 0, 1920, 1080)];
        let b = bounding_box(&l).unwrap();
        assert_eq!(monitor_in_image(l[0], b, 3840, 1080), r(0, 0, 1920, 1080));
        assert_eq!(monitor_in_image(l[1], b, 3840, 1080), r(1920, 0, 1920, 1080));
    }

    #[test]
    fn selection_scale_one() {
        let m = r(2400, 366, 1920, 1080);
        let s = selection_in_image(m, 1920.0, 1080.0, (100.0, 50.0, 300.0, 200.0)).unwrap();
        assert_eq!(s, r(2500, 416, 300, 200));
    }

    #[test]
    fn selection_fractional_scale() {
        // 1.5x monitor: 1280x720 CSS px overlay showing a 1920x1080 image region.
        let m = r(0, 0, 1920, 1080);
        let s = selection_in_image(m, 1280.0, 720.0, (100.0, 100.0, 200.0, 100.0)).unwrap();
        assert_eq!(s, r(150, 150, 300, 150));
    }

    #[test]
    fn selection_dragged_up_left_is_normalised() {
        let m = r(0, 0, 1000, 1000);
        let s = selection_in_image(m, 1000.0, 1000.0, (500.0, 500.0, -200.0, -100.0)).unwrap();
        assert_eq!(s, r(300, 400, 200, 100));
    }

    #[test]
    fn selection_is_clamped_to_its_monitor() {
        let m = r(1000, 0, 1000, 1000);
        let s = selection_in_image(m, 1000.0, 1000.0, (900.0, 900.0, 500.0, 500.0)).unwrap();
        assert_eq!(s, r(1900, 900, 100, 100));
        let s = selection_in_image(m, 1000.0, 1000.0, (-50.0, 0.0, 100.0, 100.0)).unwrap();
        assert_eq!(s, r(1000, 0, 50, 100));
    }

    #[test]
    fn empty_selection_is_none() {
        let m = r(0, 0, 100, 100);
        assert_eq!(selection_in_image(m, 100.0, 100.0, (10.0, 10.0, 0.0, 5.0)), None);
        assert_eq!(selection_in_image(m, 0.0, 100.0, (0.0, 0.0, 5.0, 5.0)), None);
    }
}
