//! Desktop capture backends and PNG helpers.

use image::{
    DynamicImage,
    codecs::png::{CompressionType, FilterType, PngEncoder},
};

use crate::monitors::Rect;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// `file:///a/b%20c.png` -> `/a/b c.png` (Unix only).
#[cfg(target_os = "linux")]
fn file_uri_to_path(uri: &str) -> Option<std::path::PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let b = rest.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    Some(std::path::PathBuf::from(String::from_utf8(out).ok()?))
}

/// Capture the whole desktop (all monitors) as one image.
///
/// Linux: the Screenshot portal (works on Wayland and X11). The portal saves the
/// PNG into the user's Pictures folder itself, so we load it and delete it again.
#[cfg(target_os = "linux")]
pub async fn grab_desktop(_monitors: &[Rect]) -> Result<DynamicImage, String> {
    let shot = ashpd::desktop::screenshot::Screenshot::request()
        .interactive(false)
        .send()
        .await
        .map_err(err)?
        .response()
        .map_err(err)?;
    let uri = shot.uri().to_string();
    let path = file_uri_to_path(&uri).ok_or(format!("portal returned a non-file uri: {uri}"))?;
    let loaded = image::open(&path).map_err(err);
    let _ = std::fs::remove_file(&path); // best effort: do not leave stray files in ~/Pictures
    Ok(DynamicImage::ImageRgba8(loaded?.to_rgba8()))
}

/// macOS and Windows: capture every monitor with `xcap` and stitch the shots into one image
/// that lines up with Tauri's monitor rectangles (`monitors`, physical pixels).
///
/// UNTESTED on real hardware so far: only compiled by CI. The first run on a Mac needs the
/// Screen Recording permission; xcap triggers the system prompt.
#[cfg(not(target_os = "linux"))]
pub async fn grab_desktop(monitors: &[Rect]) -> Result<DynamicImage, String> {
    let monitors = monitors.to_vec();
    tauri::async_runtime::spawn_blocking(move || grab_blocking(&monitors))
        .await
        .map_err(err)?
}

#[cfg(not(target_os = "linux"))]
fn grab_blocking(monitors: &[Rect]) -> Result<DynamicImage, String> {
    use crate::stitch::{CaptureMonitor, match_monitors, stitch};

    let cap = xcap::Monitor::all().map_err(err)?;
    let infos = cap
        .iter()
        .map(|m| {
            Ok(CaptureMonitor {
                rect: Rect {
                    x: m.x().map_err(err)?,
                    y: m.y().map_err(err)?,
                    w: m.width().map_err(err)?,
                    h: m.height().map_err(err)?,
                },
                scale: m.scale_factor().map_err(err)? as f64,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let mut shots = Vec::with_capacity(monitors.len());
    for (i, matched) in match_monitors(monitors, &infos).into_iter().enumerate() {
        let j = matched.ok_or_else(|| format!("no capture source for monitor {i}"))?;
        shots.push(cap[j].capture_image().map_err(err)?);
    }
    Ok(DynamicImage::ImageRgba8(stitch(monitors, &shots)))
}

/// Encode as PNG. `fast` trades size for speed (used for the transient overlay slices).
pub fn encode_png(img: &DynamicImage, fast: bool) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    let (c, f) = if fast {
        (CompressionType::Fast, FilterType::Sub)
    } else {
        (CompressionType::Default, FilterType::Adaptive)
    };
    img.write_with_encoder(PngEncoder::new_with_quality(&mut buf, c, f))
        .map_err(err)?;
    Ok(buf)
}

/// Crop `r` (image pixels) out of `img` and encode it as PNG.
pub fn crop_png(img: &DynamicImage, r: Rect, fast: bool) -> Result<Vec<u8>, String> {
    if r.w == 0 || r.h == 0 {
        return Err("empty crop".into());
    }
    let cropped = DynamicImage::ImageRgba8(img.crop_imm(r.x as u32, r.y as u32, r.w, r.h).to_rgba8());
    encode_png(&cropped, fast)
}
