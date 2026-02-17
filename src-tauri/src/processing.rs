use crate::ocr::OcrResult;
use crate::scanner::ScannerError;
use ::image::imageops;
use ::image::{DynamicImage, GenericImageView, GrayImage, ImageFormat, Luma, Rgb, RgbImage};
use printpdf::*;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufWriter, Cursor};
use std::path::Path;

// ─── Enums & DTOs for image operations ────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RotationAngle {
    R90,
    R180,
    R270,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FlipAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageAdjustments {
    pub brightness: f32,  // -100.0 to +100.0
    pub contrast: f32,    // -100.0 to +100.0
    pub saturation: f32,  // -100.0 to +100.0
    pub sharpness: f32,   //    0.0 to +100.0
}

/// Page data for multi-page PDF generation.
pub struct PageData<'a> {
    pub png_data: &'a [u8],
    pub dpi: u32,
    pub ocr: Option<&'a OcrResult>,
}

/// Auto-crop: detects document edges and crops the image.
/// Uses a simple threshold-based edge detection.
pub fn auto_crop(png_data: &[u8]) -> Result<Vec<u8>, ScannerError> {
    let img = ::image::load_from_memory(png_data)
        .map_err(|e| ScannerError::SystemError(format!("Décodage image: {}", e)))?;

    let gray = img.to_luma8();
    let (w, h) = gray.dimensions();

    // Find the bounding box of the document
    // We look for the transition from background (light) to content (darker)
    let threshold: u8 = 230; // Pixels brighter than this are considered background

    let mut min_x = w;
    let mut min_y = h;
    let mut max_x: u32 = 0;
    let mut max_y: u32 = 0;

    for y in 0..h {
        for x in 0..w {
            let pixel = gray.get_pixel(x, y).0[0];
            if pixel < threshold {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }

    // Add a small margin (2% of dimension)
    let margin_x = (w as f32 * 0.02) as u32;
    let margin_y = (h as f32 * 0.02) as u32;

    let crop_x = min_x.saturating_sub(margin_x);
    let crop_y = min_y.saturating_sub(margin_y);
    let crop_w = (max_x + margin_x + 1).min(w) - crop_x;
    let crop_h = (max_y + margin_y + 1).min(h) - crop_y;

    // Only crop if we found meaningful content and it's smaller than the original
    if crop_w < 10 || crop_h < 10 || (crop_w >= w - 10 && crop_h >= h - 10) {
        // Nothing to crop or image is already tight
        return Ok(png_data.to_vec());
    }

    let cropped = img.crop_imm(crop_x, crop_y, crop_w, crop_h);

    let mut output = Vec::new();
    let mut cursor = Cursor::new(&mut output);
    cropped
        .write_to(&mut cursor, ImageFormat::Png)
        .map_err(|e| ScannerError::SystemError(format!("Encodage PNG après recadrage: {}", e)))?;

    Ok(output)
}

// ─── Rotation & Flip ──────────────────────────────────────────────

/// Rotates the image by 90, 180, or 270 degrees.
pub fn rotate_image(png_data: &[u8], angle: &RotationAngle) -> Result<Vec<u8>, ScannerError> {
    let img = ::image::load_from_memory(png_data)
        .map_err(|e| ScannerError::SystemError(format!("Décodage image: {}", e)))?;

    let rotated = match angle {
        RotationAngle::R90 => img.rotate90(),
        RotationAngle::R180 => img.rotate180(),
        RotationAngle::R270 => img.rotate270(),
    };

    encode_to_png(&rotated)
}

/// Flips the image horizontally or vertically.
pub fn flip_image(png_data: &[u8], axis: &FlipAxis) -> Result<Vec<u8>, ScannerError> {
    let img = ::image::load_from_memory(png_data)
        .map_err(|e| ScannerError::SystemError(format!("Décodage image: {}", e)))?;

    let flipped = match axis {
        FlipAxis::Horizontal => img.fliph(),
        FlipAxis::Vertical => img.flipv(),
    };

    encode_to_png(&flipped)
}

// ─── Image Adjustments ───────────────────────────────────────────

/// Applies brightness, contrast, saturation, and sharpness adjustments.
pub fn apply_adjustments(
    png_data: &[u8],
    adj: &ImageAdjustments,
) -> Result<Vec<u8>, ScannerError> {
    let img = ::image::load_from_memory(png_data)
        .map_err(|e| ScannerError::SystemError(format!("Décodage image: {}", e)))?;

    let mut result = img;

    // Brightness: map -100..+100 to -255..+255
    if adj.brightness.abs() > 0.5 {
        let value = (adj.brightness * 2.55) as i32;
        result = DynamicImage::ImageRgba8(imageops::brighten(&result, value));
    }

    // Contrast: map -100..+100 to scale factor
    if adj.contrast.abs() > 0.5 {
        result = DynamicImage::ImageRgba8(imageops::contrast(&result, adj.contrast));
    }

    // Saturation: manual HSL adjustment
    if adj.saturation.abs() > 0.5 {
        result = adjust_saturation(result, adj.saturation);
    }

    // Sharpness: unsharp mask
    if adj.sharpness > 0.5 {
        result = unsharp_mask(result, adj.sharpness);
    }

    encode_to_png(&result)
}

/// Adjusts saturation by converting to HSL, scaling S, and converting back.
fn adjust_saturation(img: DynamicImage, amount: f32) -> DynamicImage {
    let factor = 1.0 + amount / 100.0; // -100→0.0, 0→1.0, +100→2.0
    let mut rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();

    for y in 0..h {
        for x in 0..w {
            let pixel = rgb.get_pixel(x, y);
            let (r, g, b) = (
                pixel[0] as f32 / 255.0,
                pixel[1] as f32 / 255.0,
                pixel[2] as f32 / 255.0,
            );

            let max = r.max(g).max(b);
            let min = r.min(g).min(b);
            let delta = max - min;

            if delta < 0.001 {
                continue; // already gray
            }

            let lum = (max + min) / 2.0;

            // Compute hue
            let hue = if (max - r).abs() < 0.001 {
                60.0 * (((g - b) / delta) % 6.0)
            } else if (max - g).abs() < 0.001 {
                60.0 * ((b - r) / delta + 2.0)
            } else {
                60.0 * ((r - g) / delta + 4.0)
            };
            let hue = if hue < 0.0 { hue + 360.0 } else { hue };

            let sat = if lum < 0.5 {
                delta / (max + min)
            } else {
                delta / (2.0 - max - min)
            };

            let new_sat = (sat * factor).clamp(0.0, 1.0);

            // HSL to RGB
            let (nr, ng, nb) = hsl_to_rgb(hue, new_sat, lum);
            rgb.put_pixel(
                x,
                y,
                Rgb([
                    (nr * 255.0).clamp(0.0, 255.0) as u8,
                    (ng * 255.0).clamp(0.0, 255.0) as u8,
                    (nb * 255.0).clamp(0.0, 255.0) as u8,
                ]),
            );
        }
    }

    DynamicImage::ImageRgb8(rgb)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s < 0.001 {
        return (l, l, l);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let h_norm = h / 360.0;
    (
        hue_to_rgb(p, q, h_norm + 1.0 / 3.0),
        hue_to_rgb(p, q, h_norm),
        hue_to_rgb(p, q, h_norm - 1.0 / 3.0),
    )
}

fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 { t += 1.0; }
    if t > 1.0 { t -= 1.0; }
    if t < 1.0 / 6.0 {
        return p + (q - p) * 6.0 * t;
    }
    if t < 1.0 / 2.0 {
        return q;
    }
    if t < 2.0 / 3.0 {
        return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
    }
    p
}

/// Unsharp mask for sharpening.
fn unsharp_mask(img: DynamicImage, amount: f32) -> DynamicImage {
    let sigma = 1.0; // fixed blur radius
    let strength = amount / 100.0; // 0..1
    let blurred = img.blur(sigma);

    let orig = img.to_rgb8();
    let blur_rgb = blurred.to_rgb8();
    let (w, h) = orig.dimensions();
    let mut out = RgbImage::new(w, h);

    for y in 0..h {
        for x in 0..w {
            let op = orig.get_pixel(x, y);
            let bp = blur_rgb.get_pixel(x, y);
            let r = (op[0] as f32 + strength * (op[0] as f32 - bp[0] as f32)).clamp(0.0, 255.0) as u8;
            let g = (op[1] as f32 + strength * (op[1] as f32 - bp[1] as f32)).clamp(0.0, 255.0) as u8;
            let b = (op[2] as f32 + strength * (op[2] as f32 - bp[2] as f32)).clamp(0.0, 255.0) as u8;
            out.put_pixel(x, y, Rgb([r, g, b]));
        }
    }

    DynamicImage::ImageRgb8(out)
}

// ─── Noise Reduction ─────────────────────────────────────────────

/// Reduces noise using a median filter.
/// strength: 1 = 3x3 window, 2 = 5x5 window
pub fn reduce_noise(png_data: &[u8], strength: u8) -> Result<Vec<u8>, ScannerError> {
    let img = ::image::load_from_memory(png_data)
        .map_err(|e| ScannerError::SystemError(format!("Décodage image: {}", e)))?;

    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let radius = if strength >= 2 { 2i32 } else { 1i32 };
    let window_size = ((2 * radius + 1) * (2 * radius + 1)) as usize;
    let mut out = RgbImage::new(w, h);

    let mut r_vals = Vec::with_capacity(window_size);
    let mut g_vals = Vec::with_capacity(window_size);
    let mut b_vals = Vec::with_capacity(window_size);

    for y in 0..h {
        for x in 0..w {
            r_vals.clear();
            g_vals.clear();
            b_vals.clear();

            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    let nx = (x as i32 + dx).clamp(0, w as i32 - 1) as u32;
                    let ny = (y as i32 + dy).clamp(0, h as i32 - 1) as u32;
                    let p = rgb.get_pixel(nx, ny);
                    r_vals.push(p[0]);
                    g_vals.push(p[1]);
                    b_vals.push(p[2]);
                }
            }

            r_vals.sort_unstable();
            g_vals.sort_unstable();
            b_vals.sort_unstable();

            let mid = r_vals.len() / 2;
            out.put_pixel(x, y, Rgb([r_vals[mid], g_vals[mid], b_vals[mid]]));
        }
    }

    encode_to_png(&DynamicImage::ImageRgb8(out))
}

// ─── Deskew ──────────────────────────────────────────────────────

/// Detects and corrects document skew. Returns (corrected_png, angle_degrees).
pub fn deskew(png_data: &[u8]) -> Result<(Vec<u8>, f32), ScannerError> {
    let img = ::image::load_from_memory(png_data)
        .map_err(|e| ScannerError::SystemError(format!("Décodage image: {}", e)))?;

    let gray = img.to_luma8();
    let (w, h) = gray.dimensions();

    // Downsample for faster angle search
    let max_dim = w.max(h) as f32;
    let scale = if max_dim > 800.0 { 800.0 / max_dim } else { 1.0 };
    let small = if scale < 1.0 {
        imageops::resize(
            &gray,
            (w as f32 * scale) as u32,
            (h as f32 * scale) as u32,
            imageops::FilterType::Nearest,
        )
    } else {
        gray.clone()
    };

    // Otsu threshold for binarization
    let threshold = otsu_threshold(&small);

    // Binarize
    let (sw, sh) = small.dimensions();
    let mut binary = GrayImage::new(sw, sh);
    for y in 0..sh {
        for x in 0..sw {
            let v = small.get_pixel(x, y).0[0];
            binary.put_pixel(x, y, Luma([if v < threshold { 255 } else { 0 }]));
        }
    }

    // Search for best angle using projection profile variance
    let mut best_angle: f32 = 0.0;
    let mut best_variance: f64 = 0.0;

    // Search -10 to +10 degrees in 0.5 degree steps (coarse)
    let mut angle = -10.0f32;
    while angle <= 10.0 {
        let var = projection_variance(&binary, angle);
        if var > best_variance {
            best_variance = var;
            best_angle = angle;
        }
        angle += 0.5;
    }

    // Refine: search ±1 degree around best in 0.1 degree steps
    let refine_start = best_angle - 1.0;
    let refine_end = best_angle + 1.0;
    let mut fine_angle = refine_start;
    while fine_angle <= refine_end {
        let var = projection_variance(&binary, fine_angle);
        if var > best_variance {
            best_variance = var;
            best_angle = fine_angle;
        }
        fine_angle += 0.1;
    }

    // Skip if angle is negligible
    if best_angle.abs() < 0.2 {
        return Ok((png_data.to_vec(), 0.0));
    }

    // Apply rotation to full-res color image
    let corrected = rotate_arbitrary(&img, -best_angle);
    let result = encode_to_png(&corrected)?;

    Ok((result, best_angle))
}

/// Computes the Otsu threshold for a grayscale image.
fn otsu_threshold(gray: &GrayImage) -> u8 {
    let mut histogram = [0u32; 256];
    for pixel in gray.pixels() {
        histogram[pixel.0[0] as usize] += 1;
    }

    let total = gray.width() * gray.height();
    let mut sum: f64 = 0.0;
    for (i, &count) in histogram.iter().enumerate() {
        sum += i as f64 * count as f64;
    }

    let mut sum_bg: f64 = 0.0;
    let mut weight_bg: f64 = 0.0;
    let mut max_variance: f64 = 0.0;
    let mut best_threshold: u8 = 0;

    for (t, &count) in histogram.iter().enumerate() {
        weight_bg += count as f64;
        if weight_bg == 0.0 {
            continue;
        }
        let weight_fg = total as f64 - weight_bg;
        if weight_fg == 0.0 {
            break;
        }

        sum_bg += t as f64 * count as f64;
        let mean_bg = sum_bg / weight_bg;
        let mean_fg = (sum - sum_bg) / weight_fg;

        let between_var = weight_bg * weight_fg * (mean_bg - mean_fg).powi(2);
        if between_var > max_variance {
            max_variance = between_var;
            best_threshold = t as u8;
        }
    }

    best_threshold
}

/// Computes the variance of horizontal projection profile for a binary image
/// rotated by the given angle.
fn projection_variance(binary: &GrayImage, angle_deg: f32) -> f64 {
    let (w, h) = binary.dimensions();
    let angle_rad = angle_deg * std::f32::consts::PI / 180.0;
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();
    let cx = w as f32 / 2.0;
    let cy = h as f32 / 2.0;

    // Compute row sums of the rotated image
    let mut profile = vec![0u32; h as usize];

    for y in 0..h {
        for x in 0..w {
            // Reverse-map: find source pixel for this destination position
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let src_x = (dx * cos_a + dy * sin_a + cx) as i32;
            let src_y = (-dx * sin_a + dy * cos_a + cy) as i32;

            if src_x >= 0 && src_x < w as i32 && src_y >= 0 && src_y < h as i32 {
                if binary.get_pixel(src_x as u32, src_y as u32).0[0] > 0 {
                    profile[y as usize] += 1;
                }
            }
        }
    }

    // Compute variance of the profile
    let n = profile.len() as f64;
    let mean = profile.iter().map(|&v| v as f64).sum::<f64>() / n;
    profile
        .iter()
        .map(|&v| (v as f64 - mean).powi(2))
        .sum::<f64>()
        / n
}

/// Rotates a color image by an arbitrary angle (in degrees) with white fill.
/// Uses bilinear interpolation for smooth results at small angles.
fn rotate_arbitrary(img: &DynamicImage, angle_deg: f32) -> DynamicImage {
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let angle_rad = angle_deg * std::f32::consts::PI / 180.0;
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();
    let cx = w as f32 / 2.0;
    let cy = h as f32 / 2.0;

    let mut out = RgbImage::from_pixel(w, h, Rgb([255, 255, 255]));

    for y in 0..h {
        for x in 0..w {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let src_x = dx * cos_a + dy * sin_a + cx;
            let src_y = -dx * sin_a + dy * cos_a + cy;

            // Bilinear interpolation
            let x0 = src_x.floor();
            let y0 = src_y.floor();
            let x1 = x0 + 1.0;
            let y1 = y0 + 1.0;

            if x0 >= 0.0 && x1 < w as f32 && y0 >= 0.0 && y1 < h as f32 {
                let fx = src_x - x0;
                let fy = src_y - y0;
                let x0u = x0 as u32;
                let y0u = y0 as u32;
                let x1u = x1 as u32;
                let y1u = y1 as u32;

                let p00 = rgb.get_pixel(x0u, y0u);
                let p10 = rgb.get_pixel(x1u, y0u);
                let p01 = rgb.get_pixel(x0u, y1u);
                let p11 = rgb.get_pixel(x1u, y1u);

                let mut pixel = [0u8; 3];
                for c in 0..3 {
                    let v = p00[c] as f32 * (1.0 - fx) * (1.0 - fy)
                        + p10[c] as f32 * fx * (1.0 - fy)
                        + p01[c] as f32 * (1.0 - fx) * fy
                        + p11[c] as f32 * fx * fy;
                    pixel[c] = v.clamp(0.0, 255.0) as u8;
                }
                out.put_pixel(x, y, Rgb(pixel));
            }
        }
    }

    DynamicImage::ImageRgb8(out)
}

// ─── Background Whitening ────────────────────────────────────────

/// Whitens the background of a scanned document.
/// Pixels with luminance above the threshold are set to white.
pub fn whiten_background(png_data: &[u8], threshold: u8) -> Result<Vec<u8>, ScannerError> {
    let img = ::image::load_from_memory(png_data)
        .map_err(|e| ScannerError::SystemError(format!("Décodage image: {}", e)))?;

    let mut rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();

    for y in 0..h {
        for x in 0..w {
            let pixel = rgb.get_pixel(x, y);
            let lum = (0.299 * pixel[0] as f32 + 0.587 * pixel[1] as f32 + 0.114 * pixel[2] as f32) as u8;
            if lum > threshold {
                rgb.put_pixel(x, y, Rgb([255, 255, 255]));
            }
        }
    }

    encode_to_png(&DynamicImage::ImageRgb8(rgb))
}

// ─── Multi-page PDF ──────────────────────────────────────────────

/// Saves multiple pages as a single PDF file.
pub fn save_as_pdf_multipage(
    pages: &[PageData],
    output_path: &str,
    title: &str,
) -> Result<(), ScannerError> {
    if pages.is_empty() {
        return Err(ScannerError::SystemError("Aucune page à sauvegarder".into()));
    }

    // Create document with first page
    let first = &pages[0];
    let first_img = ::image::load_from_memory(first.png_data)
        .map_err(|e| ScannerError::SystemError(format!("Décodage page 1: {}", e)))?;
    let (first_w, first_h) = first_img.dimensions();
    let first_dpi = if first.dpi > 0 { first.dpi as f32 } else { 300.0 };
    let first_w_mm = first_w as f32 / first_dpi * 25.4;
    let first_h_mm = first_h as f32 / first_dpi * 25.4;

    let (doc, first_page_idx, first_layer_idx) =
        PdfDocument::new(title, Mm(first_w_mm), Mm(first_h_mm), "Scan");

    // Embed first page
    embed_page_in_pdf(
        &doc,
        first_page_idx,
        first_layer_idx,
        &first_img,
        first_w_mm,
        first_h_mm,
        first_dpi,
        first.ocr,
    )?;

    // Add remaining pages
    for (i, page) in pages.iter().enumerate().skip(1) {
        let img = ::image::load_from_memory(page.png_data)
            .map_err(|e| ScannerError::SystemError(format!("Décodage page {}: {}", i + 1, e)))?;
        let (pw, ph) = img.dimensions();
        let dpi = if page.dpi > 0 { page.dpi as f32 } else { 300.0 };
        let w_mm = pw as f32 / dpi * 25.4;
        let h_mm = ph as f32 / dpi * 25.4;

        let (page_idx, layer_idx) =
            doc.add_page(Mm(w_mm), Mm(h_mm), &format!("Page {}", i + 1));

        embed_page_in_pdf(&doc, page_idx, layer_idx, &img, w_mm, h_mm, dpi, page.ocr)?;
    }

    let file = File::create(output_path)
        .map_err(|e| ScannerError::SystemError(format!("Création fichier PDF: {}", e)))?;
    doc.save(&mut BufWriter::new(file))
        .map_err(|e| ScannerError::SystemError(format!("Sauvegarde PDF: {}", e)))?;

    Ok(())
}

/// Embeds an image page into a PDF document with optional OCR text layer.
fn embed_page_in_pdf(
    doc: &PdfDocumentReference,
    page_idx: PdfPageIndex,
    layer_idx: PdfLayerIndex,
    img: &DynamicImage,
    _width_mm: f32,
    height_mm: f32,
    dpi: f32,
    ocr_result: Option<&OcrResult>,
) -> Result<(), ScannerError> {
    let page = doc.get_page(page_idx);
    let layer = page.get_layer(layer_idx);

    let (width_px, height_px) = img.dimensions();
    let rgb_img = img.to_rgb8();
    let raw_pixels: Vec<u8> = rgb_img.into_raw();

    let pdf_image = printpdf::Image::from(ImageXObject {
        width: Px(width_px as usize),
        height: Px(height_px as usize),
        color_space: ColorSpace::Rgb,
        bits_per_component: ColorBits::Bit8,
        interpolate: true,
        image_data: raw_pixels,
        image_filter: None,
        smask: None,
        clipping_bbox: None,
    });

    pdf_image.add_to_layer(
        layer,
        ImageTransform {
            translate_x: Some(Mm(0.0)),
            translate_y: Some(Mm(0.0)),
            scale_x: Some(1.0),
            scale_y: Some(1.0),
            ..Default::default()
        },
    );

    // Add invisible OCR text layer if available
    if let Some(ocr) = ocr_result {
        if !ocr.words.is_empty() {
            let text_layer = page.add_layer("OCR Text");
            let font = doc
                .add_builtin_font(BuiltinFont::Helvetica)
                .map_err(|e| ScannerError::SystemError(format!("Police PDF: {}", e)))?;

            text_layer.set_text_rendering_mode(TextRenderingMode::Invisible);

            for word in &ocr.words {
                if word.text.is_empty() || word.w <= 0 || word.h <= 0 {
                    continue;
                }

                let word_x_mm = word.x as f32 / dpi * 25.4;
                let word_y_mm = height_mm - ((word.y + word.h) as f32 / dpi * 25.4);
                let word_h_mm = word.h as f32 / dpi * 25.4;
                let font_size_pt = word_h_mm / 25.4 * 72.0;

                if font_size_pt < 1.0 || font_size_pt > 200.0 {
                    continue;
                }

                text_layer.use_text(&word.text, font_size_pt, Mm(word_x_mm), Mm(word_y_mm), &font);
            }
        }
    }

    Ok(())
}

// ─── Helper ──────────────────────────────────────────────────────

/// Encodes a DynamicImage as PNG bytes.
fn encode_to_png(img: &DynamicImage) -> Result<Vec<u8>, ScannerError> {
    let mut output = Vec::new();
    let mut cursor = Cursor::new(&mut output);
    img.write_to(&mut cursor, ImageFormat::Png)
        .map_err(|e| ScannerError::SystemError(format!("Encodage PNG: {}", e)))?;
    Ok(output)
}

/// Saves image data as a single-page PDF file, optionally with an invisible OCR text layer.
/// Delegates to `save_as_pdf_multipage` with a single page.
pub fn save_as_pdf(
    png_data: &[u8],
    output_path: &str,
    dpi: u32,
    ocr_result: Option<&OcrResult>,
) -> Result<(), ScannerError> {
    save_as_pdf_multipage(
        &[PageData {
            png_data,
            dpi,
            ocr: ocr_result,
        }],
        output_path,
        "Document numérisé",
    )
}

/// Saves image data as an image file (PNG, JPEG, TIFF).
pub fn save_as_image(
    png_data: &[u8],
    output_path: &str,
    format: &str,
    quality: u8,
) -> Result<(), ScannerError> {
    let img = ::image::load_from_memory(png_data)
        .map_err(|e| ScannerError::SystemError(format!("Décodage image: {}", e)))?;

    let path = Path::new(output_path);

    match format.to_uppercase().as_str() {
        "PNG" => {
            img.save_with_format(path, ImageFormat::Png)
                .map_err(|e| ScannerError::SystemError(format!("Sauvegarde PNG: {}", e)))?;
        }
        "JPEG" | "JPG" => {
            let file = File::create(path)
                .map_err(|e| ScannerError::SystemError(format!("Création fichier: {}", e)))?;
            let mut writer = BufWriter::new(file);
            let encoder = ::image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, quality);
            img.write_with_encoder(encoder)
                .map_err(|e| ScannerError::SystemError(format!("Sauvegarde JPEG: {}", e)))?;
        }
        "TIFF" | "TIF" => {
            img.save_with_format(path, ImageFormat::Tiff)
                .map_err(|e| ScannerError::SystemError(format!("Sauvegarde TIFF: {}", e)))?;
        }
        "BMP" => {
            img.save_with_format(path, ImageFormat::Bmp)
                .map_err(|e| ScannerError::SystemError(format!("Sauvegarde BMP: {}", e)))?;
        }
        _ => {
            return Err(ScannerError::UnsupportedFormat(format.to_string()));
        }
    }

    Ok(())
}

/// Converts image data to a different format, returning the new bytes.
pub fn convert_image(
    png_data: &[u8],
    target_format: &str,
    quality: u8,
) -> Result<Vec<u8>, ScannerError> {
    let img = ::image::load_from_memory(png_data)
        .map_err(|e| ScannerError::SystemError(format!("Décodage: {}", e)))?;

    let mut output = Vec::new();
    let mut cursor = Cursor::new(&mut output);

    match target_format.to_uppercase().as_str() {
        "PNG" => {
            img.write_to(&mut cursor, ImageFormat::Png)
                .map_err(|e| ScannerError::SystemError(format!("Conversion PNG: {}", e)))?;
        }
        "JPEG" | "JPG" => {
            let encoder = ::image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, quality);
            img.write_with_encoder(encoder)
                .map_err(|e| ScannerError::SystemError(format!("Conversion JPEG: {}", e)))?;
        }
        _ => {
            return Err(ScannerError::UnsupportedFormat(target_format.to_string()));
        }
    }

    Ok(output)
}

/// Adjusts image quality/resolution by resizing.
pub fn resize_image(
    png_data: &[u8],
    target_dpi: u32,
    original_dpi: u32,
) -> Result<Vec<u8>, ScannerError> {
    if target_dpi >= original_dpi {
        return Ok(png_data.to_vec());
    }

    let img = ::image::load_from_memory(png_data)
        .map_err(|e| ScannerError::SystemError(format!("Décodage: {}", e)))?;

    let scale = target_dpi as f32 / original_dpi as f32;
    let new_width = (img.width() as f32 * scale) as u32;
    let new_height = (img.height() as f32 * scale) as u32;

    let resized = img.resize(new_width, new_height, ::image::imageops::FilterType::Lanczos3);

    let mut output = Vec::new();
    let mut cursor = Cursor::new(&mut output);
    resized
        .write_to(&mut cursor, ImageFormat::Png)
        .map_err(|e| ScannerError::SystemError(format!("Encodage: {}", e)))?;

    Ok(output)
}
