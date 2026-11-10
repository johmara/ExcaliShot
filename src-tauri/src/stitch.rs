//! Helpers for platforms that capture one monitor at a time (macOS, Windows).
//!
//! The rest of the app expects a single desktop image whose size matches the bounding box of
//! Tauri's monitor rectangles. These functions pair each Tauri monitor with the capture
//! library's monitor and paste the shots into one canvas. They are pure so they can be
//! unit-tested on any platform.
#![cfg_attr(target_os = "linux", allow(dead_code))]

use image::{
    RgbaImage,
    imageops::{self, FilterType},
};

use crate::monitors::{Rect, bounding_box};

/// A monitor as reported by the capture library.
#[derive(Debug, Clone)]
pub struct CaptureMonitor {
    pub rect: Rect,
    pub scale: f64,
}

fn scaled(r: Rect, s: f64) -> Rect {
    Rect {
        x: (r.x as f64 * s).round() as i32,
        y: (r.y as f64 * s).round() as i32,
        w: (r.w as f64 * s).round() as u32,
        h: (r.h as f64 * s).round() as u32,
    }
}

/// For each Tauri monitor, the index of the capture monitor showing the same screen.
///
/// Tauri reports physical pixels while capture libraries differ on whether they report logical
/// or physical coordinates, so try an exact match first, then a match after applying the
/// monitor's own scale, and finally pair whatever is left in order.
pub fn match_monitors(tauri: &[Rect], capture: &[CaptureMonitor]) -> Vec<Option<usize>> {
    let mut used = vec![false; capture.len()];
    let mut out = vec![None; tauri.len()];
    for pass in 0..2 {
        for (i, t) in tauri.iter().enumerate() {
            if out[i].is_some() {
                continue;
            }
            let found = capture.iter().enumerate().position(|(j, c)| {
                !used[j] && if pass == 0 { c.rect == *t } else { scaled(c.rect, c.scale) == *t }
            });
            if let Some(j) = found {
                out[i] = Some(j);
                used[j] = true;
            }
        }
    }
    let mut free = (0..capture.len()).filter(|j| !used[*j]);
    for slot in out.iter_mut() {
        if slot.is_none() {
            *slot = free.next();
        }
    }
    out
}

/// Paste `shots[i]` (the picture of `tauri[i]`) into one canvas the size of the monitors'
/// bounding box. Shots whose size differs from their monitor rectangle are resized to it.
pub fn stitch(tauri: &[Rect], shots: &[RgbaImage]) -> RgbaImage {
    let Some(bbox) = bounding_box(tauri) else {
        return RgbaImage::new(1, 1);
    };
    let mut canvas = RgbaImage::new(bbox.w, bbox.h);
    for (r, img) in tauri.iter().zip(shots) {
        let resized;
        let img = if img.width() == r.w && img.height() == r.h {
            img
        } else {
            resized = imageops::resize(img, r.w, r.h, FilterType::Triangle);
            &resized
        };
        imageops::replace(&mut canvas, img, (r.x - bbox.x) as i64, (r.y - bbox.y) as i64);
    }
    canvas
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn r(x: i32, y: i32, w: u32, h: u32) -> Rect {
        Rect { x, y, w, h }
    }
    fn cm(rect: Rect, scale: f64) -> CaptureMonitor {
        CaptureMonitor { rect, scale }
    }

    #[test]
    fn matches_identical_rectangles_in_any_order() {
        let tauri = [r(0, 0, 1920, 1080), r(1920, 0, 1080, 1920)];
        let cap = [cm(r(1920, 0, 1080, 1920), 1.0), cm(r(0, 0, 1920, 1080), 1.0)];
        assert_eq!(match_monitors(&tauri, &cap), vec![Some(1), Some(0)]);
    }

    #[test]
    fn matches_logical_capture_coordinates_by_scale() {
        // Retina: Tauri says 2880x1800 physical, the capture library says 1440x900 points at 2x.
        let tauri = [r(0, 0, 2880, 1800)];
        let cap = [cm(r(0, 0, 1440, 900), 2.0)];
        assert_eq!(match_monitors(&tauri, &cap), vec![Some(0)]);
    }

    #[test]
    fn falls_back_to_order_and_never_reuses_a_monitor() {
        let tauri = [r(0, 0, 100, 100), r(100, 0, 100, 100)];
        let cap = [cm(r(5, 5, 7, 7), 1.0), cm(r(9, 9, 7, 7), 1.0)];
        assert_eq!(match_monitors(&tauri, &cap), vec![Some(0), Some(1)]);
    }

    #[test]
    fn reports_missing_capture_monitors() {
        let tauri = [r(0, 0, 100, 100), r(100, 0, 100, 100)];
        let cap = [cm(r(0, 0, 100, 100), 1.0)];
        assert_eq!(match_monitors(&tauri, &cap), vec![Some(0), None]);
    }

    #[test]
    fn stitch_places_each_shot_at_its_offset() {
        let tauri = [r(-4, 2, 4, 4), r(0, 0, 4, 4)];
        let a = RgbaImage::from_pixel(4, 4, Rgba([255, 0, 0, 255]));
        let b = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 255, 255]));
        let out = stitch(&tauri, &[a, b]);
        assert_eq!((out.width(), out.height()), (8, 6));
        assert_eq!(out.get_pixel(0, 2), &Rgba([255, 0, 0, 255])); // first monitor
        assert_eq!(out.get_pixel(4, 0), &Rgba([0, 0, 255, 255])); // second monitor
        assert_eq!(out.get_pixel(0, 0), &Rgba([0, 0, 0, 0])); // gap above the first
    }

    #[test]
    fn stitch_resizes_a_shot_that_does_not_fit_its_monitor() {
        let tauri = [r(0, 0, 4, 4)];
        let big = RgbaImage::from_pixel(8, 8, Rgba([10, 20, 30, 255]));
        let out = stitch(&tauri, &[big]);
        assert_eq!((out.width(), out.height()), (4, 4));
        assert_eq!(out.get_pixel(2, 2), &Rgba([10, 20, 30, 255]));
    }
}
