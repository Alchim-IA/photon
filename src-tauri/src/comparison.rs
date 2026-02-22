use serde::Serialize;
use crate::scanner::ScannerError;

#[derive(Debug, Clone, Serialize)]
pub struct ComparisonResult {
    pub similarity_percent: f64,
    pub diff_image_base64: String,
    pub text_diff: Option<Vec<DiffLine>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffLine {
    pub kind: String, // "same", "added", "removed"
    pub text: String,
}

/// Compare two document images pixel-by-pixel, producing a diff overlay.
pub fn compare_documents(
    png_data_a: &[u8],
    png_data_b: &[u8],
    ocr_text_a: Option<&str>,
    ocr_text_b: Option<&str>,
) -> Result<ComparisonResult, ScannerError> {
    let img_a = image::load_from_memory(png_data_a)
        .map_err(|e| ScannerError::SystemError(format!("Load image A: {}", e)))?;
    let img_b = image::load_from_memory(png_data_b)
        .map_err(|e| ScannerError::SystemError(format!("Load image B: {}", e)))?;

    let rgba_a = img_a.to_rgba8();
    let rgba_b = img_b.to_rgba8();

    // Use the larger dimensions for the diff image
    let width = rgba_a.width().max(rgba_b.width());
    let height = rgba_a.height().max(rgba_b.height());

    let mut diff_img = image::RgbaImage::new(width, height);
    let mut total_pixels = 0u64;
    let mut same_pixels = 0u64;

    for y in 0..height {
        for x in 0..width {
            total_pixels += 1;

            let pixel_a = if x < rgba_a.width() && y < rgba_a.height() {
                *rgba_a.get_pixel(x, y)
            } else {
                image::Rgba([255, 255, 255, 255])
            };

            let pixel_b = if x < rgba_b.width() && y < rgba_b.height() {
                *rgba_b.get_pixel(x, y)
            } else {
                image::Rgba([255, 255, 255, 255])
            };

            // Calculate pixel difference
            let dr = (pixel_a[0] as i32 - pixel_b[0] as i32).unsigned_abs();
            let dg = (pixel_a[1] as i32 - pixel_b[1] as i32).unsigned_abs();
            let db = (pixel_a[2] as i32 - pixel_b[2] as i32).unsigned_abs();
            let diff = (dr + dg + db) / 3;

            if diff < 30 {
                // Same pixel — show dimmed original
                same_pixels += 1;
                diff_img.put_pixel(
                    x,
                    y,
                    image::Rgba([
                        pixel_a[0] / 2 + 64,
                        pixel_a[1] / 2 + 64,
                        pixel_a[2] / 2 + 64,
                        255,
                    ]),
                );
            } else {
                // Different pixel — red overlay
                let intensity = (diff as u8).min(255);
                diff_img.put_pixel(x, y, image::Rgba([255, 0, 0, intensity.max(100)]));
            }
        }
    }

    let similarity = if total_pixels > 0 {
        (same_pixels as f64 / total_pixels as f64) * 100.0
    } else {
        100.0
    };

    // Encode diff image to base64
    let mut buf = Vec::new();
    image::DynamicImage::ImageRgba8(diff_img)
        .write_to(
            &mut std::io::Cursor::new(&mut buf),
            image::ImageFormat::Png,
        )
        .map_err(|e| ScannerError::SystemError(format!("Encode diff: {}", e)))?;

    let diff_base64 = base64::engine::general_purpose::STANDARD.encode(&buf);

    // Text diff if both have OCR
    let text_diff = match (ocr_text_a, ocr_text_b) {
        (Some(a), Some(b)) => Some(compute_text_diff(a, b)),
        _ => None,
    };

    Ok(ComparisonResult {
        similarity_percent: (similarity * 100.0).round() / 100.0,
        diff_image_base64: diff_base64,
        text_diff,
    })
}

fn compute_text_diff(text_a: &str, text_b: &str) -> Vec<DiffLine> {
    let lines_a: Vec<&str> = text_a.lines().collect();
    let lines_b: Vec<&str> = text_b.lines().collect();

    let mut result = Vec::new();
    let max_len = lines_a.len().max(lines_b.len());

    for i in 0..max_len {
        match (lines_a.get(i), lines_b.get(i)) {
            (Some(a), Some(b)) if a == b => {
                result.push(DiffLine {
                    kind: "same".into(),
                    text: a.to_string(),
                });
            }
            (Some(a), Some(b)) => {
                result.push(DiffLine {
                    kind: "removed".into(),
                    text: a.to_string(),
                });
                result.push(DiffLine {
                    kind: "added".into(),
                    text: b.to_string(),
                });
            }
            (Some(a), None) => {
                result.push(DiffLine {
                    kind: "removed".into(),
                    text: a.to_string(),
                });
            }
            (None, Some(b)) => {
                result.push(DiffLine {
                    kind: "added".into(),
                    text: b.to_string(),
                });
            }
            (None, None) => {}
        }
    }

    result
}

use base64::Engine;
