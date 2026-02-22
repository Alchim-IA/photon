pub mod ocr;
pub mod scanner;
pub mod processing;
pub mod storage;
pub mod intelligence;
pub mod pdf_postprocess;
pub mod vault;
pub mod templates;
pub mod comparison;
pub mod pdf_forms;
pub mod groq;
pub mod search;
pub mod tables;

use ocr::OcrResult;
use processing::{FlipAxis, ImageAdjustments, PageData, RotationAngle};
use scanner::{ScannerDevice, ScannerError, ScanOptions};
use intelligence::{AutomationRule, RuleContext};
use storage::{AppSettings, DocumentMeta, ScanProfile, TagDefinition};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Mutex;
use uuid::Uuid;

// ─── Application State ────────────────────────────────────────────

struct AppState {
    /// In-memory store for scanned images (id -> PNG bytes).
    documents: Mutex<HashMap<String, DocumentData>>,
    /// Multi-page document assembly (multipage_id -> ordered page doc_ids).
    multi_page_docs: Mutex<HashMap<String, MultiPageDoc>>,
    /// Current app settings.
    settings: Mutex<AppSettings>,
    /// Cached OCR results (doc_id -> OcrResult).
    ocr_cache: Mutex<HashMap<String, OcrResult>>,
    /// Encrypted vault manager.
    vault_manager: Mutex<vault::VaultManager>,
    /// Version history stacks (doc_id -> Vec<PNG bytes>), max 20 per doc.
    version_history: Mutex<HashMap<String, Vec<Vec<u8>>>>,
}

struct DocumentData {
    png_data: Vec<u8>,
    /// Pre-adjustment original, set when adjustment preview starts.
    original_png_data: Option<Vec<u8>>,
    width: u32,
    height: u32,
    dpi: u32,
}

struct MultiPageDoc {
    id: String,
    name: String,
    page_ids: Vec<String>,
    created_at: String,
}

// ─── DTOs ─────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct ScanResultDto {
    id: String,
    name: String,
    date: String,
    width: u32,
    height: u32,
    image_base64: String,
}

#[derive(Serialize, Deserialize)]
struct HistoryEntryDto {
    id: String,
    name: String,
    date: String,
    format: String,
    file_path: Option<String>,
    has_preview: bool,
    has_ocr: bool,
    ocr_text: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct AdjustmentPreviewResult {
    image_base64: String,
    width: u32,
    height: u32,
}

#[derive(Serialize, Deserialize)]
struct MultiPageDocDto {
    id: String,
    name: String,
    page_ids: Vec<String>,
    page_count: usize,
    created_at: String,
}

// ─── v1.1.0: PDF Save Result ──────────────────────────────────────

#[derive(Serialize)]
struct PdfSaveResult {
    path: String,
    sha256: Option<String>,
}

// ─── Tauri Commands ───────────────────────────────────────────────

#[tauri::command]
async fn list_scanners() -> Result<Vec<ScannerDevice>, ScannerError> {
    tokio::task::spawn_blocking(|| {
        let backend = scanner::get_backend();
        backend.list_devices()
    })
    .await
    .map_err(|e| ScannerError::SystemError(format!("Thread join: {}", e)))?
}

#[tauri::command]
async fn scan_document(
    options: ScanOptions,
    state: tauri::State<'_, AppState>,
) -> Result<ScanResultDto, ScannerError> {
    let opts = options.clone();
    let result = tokio::task::spawn_blocking(move || {
        let backend = scanner::get_backend();
        backend.scan(opts)
    })
    .await
    .map_err(|e| ScannerError::SystemError(format!("Thread join: {}", e)))??;

    // Apply auto-crop if enabled
    let settings = state.settings.lock().unwrap().clone();
    let final_data = if settings.auto_crop {
        processing::auto_crop(&result.image_data).unwrap_or(result.image_data.clone())
    } else {
        result.image_data.clone()
    };

    // Reload dimensions after potential crop
    let (width, height) = if settings.auto_crop {
        if let Ok(img) = ::image::load_from_memory(&final_data) {
            (img.width(), img.height())
        } else {
            (result.width, result.height)
        }
    } else {
        (result.width, result.height)
    };

    let id = Uuid::new_v4().to_string();
    let now = Local::now();

    // Increment counter and generate name from template
    let counter = {
        let mut s = state.settings.lock().unwrap();
        s.scan_counter += 1;
        s.scan_counter
    };
    let base_name = storage::expand_naming_template(
        &settings.naming_template,
        options.dpi,
        &options.color_mode,
        &settings.default_format,
        counter,
    );
    let name = format!("{}.png", base_name);
    let date = now.format("%d/%m/%Y %H:%M").to_string();

    let image_base64 = BASE64.encode(&final_data);
    {
        let mut docs = state.documents.lock().unwrap();
        docs.insert(
            id.clone(),
            DocumentData {
                png_data: final_data.clone(),
                original_png_data: None,
                width,
                height,
                dpi: options.dpi,
            },
        );
    }

    // Run auto-OCR if enabled
    let mut ocr_text: Option<String> = None;
    let mut ocr_lang_used: Option<String> = None;

    if settings.auto_ocr {
        if let Ok(ocr_result) = ocr::extract_text_with_boxes(&final_data, &settings.default_ocr_lang) {
            if !ocr_result.text.is_empty() {
                ocr_text = Some(ocr_result.text.clone());
                ocr_lang_used = Some(ocr_result.lang.clone());
                let mut cache = state.ocr_cache.lock().unwrap();
                cache.insert(id.clone(), ocr_result);
            }
        }
    }

    // Auto-export to watch folder if configured
    if let Some(ref watch_dir) = settings.watch_folder {
        let ext = settings.default_format.to_lowercase();
        let export_path = std::path::Path::new(watch_dir).join(format!("{}.{}", base_name, ext));
        let _ = auto_export_document(&final_data, export_path.to_string_lossy().as_ref(), &settings, &id, &state);
    }

    // Persist updated counter
    {
        let s = state.settings.lock().unwrap().clone();
        let _ = storage::save_settings(&s);
    }

    let _ = storage::add_to_history(DocumentMeta {
        id: id.clone(),
        name: name.clone(),
        date: date.clone(),
        file_path: None,
        format: "PNG".to_string(),
        size_bytes: 0,
        width,
        height,
        dpi: options.dpi,
        ocr_text,
        ocr_lang: ocr_lang_used,
    });

    // Track statistics for scan
    let _ = (|| -> Result<(), ScannerError> {
        let mut stats = load_stats();
        stats.total_scans += 1;
        stats.total_pages_scanned += 1;
        let month = chrono::Local::now().format("%Y-%m").to_string();
        *stats.scans_by_month.entry(month).or_insert(0) += 1;
        save_stats(&stats)
    })();

    Ok(ScanResultDto {
        id,
        name,
        date,
        width,
        height,
        image_base64,
    })
}

/// Import an external file (image, multi-page TIFF, or PDF) and return one ScanResultDto per page.
#[tauri::command]
async fn import_file(
    file_path: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ScanResultDto>, ScannerError> {
    let path = std::path::Path::new(&file_path);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Import")
        .to_string();

    let raw_pages: Vec<Vec<u8>> = match ext.as_str() {
        "pdf" => extract_pdf_pages(&file_path)?,
        "tif" | "tiff" => extract_tiff_pages(&file_path)?,
        "png" | "jpg" | "jpeg" | "bmp" | "webp" => {
            let data = std::fs::read(&file_path)
                .map_err(|e| ScannerError::SystemError(format!("Lecture fichier: {}", e)))?;
            // Validate and convert to PNG
            let img = ::image::load_from_memory(&data)
                .map_err(|e| ScannerError::SystemError(format!("Décodage image: {}", e)))?;
            let mut png_buf = Vec::new();
            img.write_to(&mut Cursor::new(&mut png_buf), ::image::ImageFormat::Png)
                .map_err(|e| ScannerError::SystemError(format!("Conversion PNG: {}", e)))?;
            vec![png_buf]
        }
        _ => return Err(ScannerError::UnsupportedFormat(ext)),
    };

    let now = Local::now();
    let date = now.format("%d/%m/%Y %H:%M").to_string();
    let total = raw_pages.len();
    let mut results = Vec::with_capacity(total);

    for (i, png_data) in raw_pages.into_iter().enumerate() {
        let img = ::image::load_from_memory(&png_data)
            .map_err(|e| ScannerError::SystemError(format!("Décodage page {}: {}", i + 1, e)))?;
        let width = img.width();
        let height = img.height();

        let id = Uuid::new_v4().to_string();
        let name = if total > 1 {
            format!("{} - page {}.png", file_stem, i + 1)
        } else {
            format!("{}.png", file_stem)
        };

        let image_base64 = BASE64.encode(&png_data);
        {
            let mut docs = state.documents.lock().unwrap();
            docs.insert(
                id.clone(),
                DocumentData {
                    png_data: png_data.clone(),
                    original_png_data: None,
                    width,
                    height,
                    dpi: 300,
                },
            );
        }

        let _ = storage::add_to_history(DocumentMeta {
            id: id.clone(),
            name: name.clone(),
            date: date.clone(),
            file_path: Some(file_path.clone()),
            format: ext.to_uppercase(),
            size_bytes: 0,
            width,
            height,
            dpi: 300,
            ocr_text: None,
            ocr_lang: None,
        });

        results.push(ScanResultDto {
            id,
            name,
            date: date.clone(),
            width,
            height,
            image_base64,
        });
    }

    // Track statistics for import
    let _ = (|| -> Result<(), ScannerError> {
        let mut stats = load_stats();
        stats.total_scans += 1;
        stats.total_pages_scanned += total as u64;
        let month = chrono::Local::now().format("%Y-%m").to_string();
        *stats.scans_by_month.entry(month).or_insert(0) += 1;
        *stats.formats_used.entry(ext.to_uppercase()).or_insert(0) += 1;
        save_stats(&stats)
    })();

    Ok(results)
}

/// Extract each page of a PDF as a PNG image.
/// Uses pdfium for full rendering, falls back to lopdf image extraction.
fn extract_pdf_pages(file_path: &str) -> Result<Vec<Vec<u8>>, ScannerError> {
    // Try pdfium rendering first (handles all PDFs: text, images, mixed)
    match extract_pdf_pages_pdfium(file_path) {
        Ok(pages) if !pages.is_empty() => {
            log::info!("PDF: {} pages rendues via pdfium", pages.len());
            return Ok(pages);
        }
        Err(e) => log::warn!("pdfium indisponible, fallback lopdf: {}", e),
        _ => log::warn!("pdfium: aucune page rendue, fallback lopdf"),
    }

    // Fallback: extract embedded images with lopdf
    extract_pdf_pages_lopdf(file_path)
}

/// Render each PDF page to a PNG using pdfium (full rendering).
fn extract_pdf_pages_pdfium(file_path: &str) -> Result<Vec<Vec<u8>>, ScannerError> {
    use pdfium_render::prelude::*;

    // Try to find libpdfium in several locations
    let pdfium = Pdfium::new(
        Pdfium::bind_to_library(
            Pdfium::pdfium_platform_library_name_at_path("./")
        )
        .or_else(|_| Pdfium::bind_to_library(
            Pdfium::pdfium_platform_library_name_at_path("../Frameworks/")
        ))
        .or_else(|_| Pdfium::bind_to_system_library())
        .map_err(|e| ScannerError::SystemError(format!(
            "Bibliothèque pdfium introuvable. Placez libpdfium.dylib à côté de l'exécutable. ({})", e
        )))?
    );
    let document = pdfium
        .load_pdf_from_file(file_path, None)
        .map_err(|e| ScannerError::SystemError(format!("pdfium: {}", e)))?;

    let render_config = PdfRenderConfig::new()
        .set_target_width(2480)  // A4 @ 300 DPI
        .set_maximum_height(3508);

    let mut pages_png = Vec::new();

    for (i, page) in document.pages().iter().enumerate() {
        let bitmap = page
            .render_with_config(&render_config)
            .map_err(|e| ScannerError::SystemError(format!("pdfium render page {}: {}", i + 1, e)))?;

        let img = bitmap
            .as_image();

        let mut png_buf = Vec::new();
        img.write_to(&mut Cursor::new(&mut png_buf), ::image::ImageFormat::Png)
            .map_err(|e| ScannerError::SystemError(format!("PNG encode page {}: {}", i + 1, e)))?;

        pages_png.push(png_buf);
    }

    Ok(pages_png)
}

/// Fallback: extract embedded images from PDF pages using lopdf.
fn extract_pdf_pages_lopdf(file_path: &str) -> Result<Vec<Vec<u8>>, ScannerError> {
    use lopdf::Document;

    let doc = Document::load(file_path)
        .map_err(|e| ScannerError::SystemError(format!("Lecture PDF: {}", e)))?;

    let mut pages_png: Vec<Vec<u8>> = Vec::new();

    let page_numbers = doc.get_pages();
    let mut sorted_pages: Vec<(u32, lopdf::ObjectId)> = page_numbers.into_iter().collect();
    sorted_pages.sort_by_key(|(num, _)| *num);

    for (page_num, page_id) in &sorted_pages {
        let page_images = extract_images_from_page(&doc, *page_id);

        if let Some(png_data) = page_images {
            pages_png.push(png_data);
        } else {
            log::warn!("PDF page {} : aucune image extractible (lopdf fallback)", page_num);
        }
    }

    if pages_png.is_empty() {
        return Err(ScannerError::SystemError(
            "Impossible de lire ce PDF. Installez la bibliothèque pdfium pour le support complet des PDF.".into(),
        ));
    }

    Ok(pages_png)
}

/// Dereference a lopdf Object if it's an indirect reference.
fn deref_object<'a>(doc: &'a lopdf::Document, obj: &'a lopdf::Object) -> Option<&'a lopdf::Object> {
    match obj {
        lopdf::Object::Reference(r) => doc.get_object(*r).ok(),
        other => Some(other),
    }
}

/// Resolve Resources for a page, walking up the page tree for inherited Resources.
fn resolve_page_resources<'a>(doc: &'a lopdf::Document, page_id: lopdf::ObjectId) -> Option<&'a lopdf::Object> {
    let mut current_id = page_id;
    for _ in 0..10 {
        let obj = doc.get_object(current_id).ok()?;
        let dict = obj.as_dict().ok()?;

        // Check if this node has Resources
        if let Ok(resources) = dict.get(b"Resources") {
            return Some(resources);
        }

        // Walk up to Parent
        let parent = dict.get(b"Parent").ok()?;
        match parent {
            lopdf::Object::Reference(r) => current_id = *r,
            _ => return None,
        }
    }
    None
}

/// Collect all image streams from an XObject dictionary, recursing into Form XObjects.
fn collect_image_streams(
    doc: &lopdf::Document,
    xobjects: &lopdf::Dictionary,
    depth: u8,
) -> Vec<lopdf::Stream> {
    if depth > 3 {
        return Vec::new();
    }

    let mut images = Vec::new();

    for (_name, obj_ref) in xobjects.iter() {
        let obj_id = match obj_ref {
            lopdf::Object::Reference(r) => *r,
            _ => continue,
        };

        let stream = match doc.get_object(obj_id) {
            Ok(lopdf::Object::Stream(ref s)) => s.clone(),
            _ => continue,
        };

        let subtype: &[u8] = stream
            .dict
            .get(b"Subtype")
            .ok()
            .and_then(|s| s.as_name().ok())
            .unwrap_or(b"");

        if subtype == b"Image" {
            images.push(stream);
        } else if subtype == b"Form" {
            // Recurse into Form XObject's own Resources
            if let Ok(form_resources) = stream.dict.get(b"Resources") {
                if let Some(form_res) = deref_object(doc, form_resources) {
                    if let Ok(form_dict) = form_res.as_dict() {
                        if let Ok(inner_xobjects) = form_dict.get(b"XObject") {
                            if let Some(inner_xobj) = deref_object(doc, inner_xobjects) {
                                if let Ok(inner_dict) = inner_xobj.as_dict() {
                                    images.extend(collect_image_streams(doc, inner_dict, depth + 1));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    images
}

/// Get the filter name from a stream, handling both single name and array of names.
fn get_stream_filter(stream: &lopdf::Stream) -> Vec<u8> {
    match stream.dict.get(b"Filter").ok() {
        Some(lopdf::Object::Name(name)) => name.clone(),
        Some(lopdf::Object::Array(arr)) => {
            // Return the first filter (outermost encoding)
            arr.first()
                .and_then(|f| {
                    if let lopdf::Object::Name(n) = f { Some(n.clone()) } else { None }
                })
                .unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

/// Try to decode a PDF image stream to a PNG.
fn decode_pdf_image(doc: &lopdf::Document, stream: &lopdf::Stream) -> Option<Vec<u8>> {
    let width = stream.dict.get(b"Width")
        .ok()
        .and_then(|w| w.as_i64().ok())
        .unwrap_or(0) as u32;
    let height = stream.dict.get(b"Height")
        .ok()
        .and_then(|h| h.as_i64().ok())
        .unwrap_or(0) as u32;

    if width == 0 || height == 0 {
        return None;
    }

    let filter = get_stream_filter(stream);

    // For DCTDecode (JPEG) or JPXDecode (JPEG2000), the raw content IS the image
    if filter == b"DCTDecode" || filter == b"JPXDecode" {
        let data = &stream.content;
        if let Ok(img) = ::image::load_from_memory(data) {
            let mut png_buf = Vec::new();
            if img.write_to(&mut Cursor::new(&mut png_buf), ::image::ImageFormat::Png).is_ok() {
                return Some(png_buf);
            }
        }
        return None;
    }

    // For FlateDecode or no filter, decompress and reconstruct the image
    let decoded = stream.decompressed_content().ok();
    let raw_data = decoded.as_deref().unwrap_or(&stream.content);

    // Try to load directly with image crate (handles PNG-like streams)
    if let Ok(img) = ::image::load_from_memory(raw_data) {
        let mut png_buf = Vec::new();
        if img.write_to(&mut Cursor::new(&mut png_buf), ::image::ImageFormat::Png).is_ok() {
            return Some(png_buf);
        }
    }

    // Manual reconstruction from raw pixel data
    let color_space = resolve_color_space(doc, stream);
    let bits = stream.dict.get(b"BitsPerComponent")
        .ok()
        .and_then(|b| b.as_i64().ok())
        .unwrap_or(8) as u8;

    if bits != 8 {
        log::debug!("PDF image: unsupported BitsPerComponent={}", bits);
        return None;
    }

    let expected_rgb = (width * height * 3) as usize;
    let expected_gray = (width * height) as usize;
    let expected_rgba = (width * height * 4) as usize;

    let dyn_image = if (color_space == "DeviceRGB" || color_space == "ICCBased-3") && raw_data.len() >= expected_rgb {
        ::image::RgbImage::from_raw(width, height, raw_data[..expected_rgb].to_vec())
            .map(::image::DynamicImage::ImageRgb8)
    } else if (color_space == "DeviceGray" || color_space == "ICCBased-1" || color_space == "CalGray") && raw_data.len() >= expected_gray {
        ::image::GrayImage::from_raw(width, height, raw_data[..expected_gray].to_vec())
            .map(::image::DynamicImage::ImageLuma8)
    } else if color_space == "DeviceCMYK" && raw_data.len() >= expected_rgba {
        // Convert CMYK to RGB
        let mut rgb_data = Vec::with_capacity(expected_rgb);
        for chunk in raw_data[..expected_rgba].chunks_exact(4) {
            let (c, m, y, k) = (chunk[0] as f32 / 255.0, chunk[1] as f32 / 255.0, chunk[2] as f32 / 255.0, chunk[3] as f32 / 255.0);
            rgb_data.push(((1.0 - c) * (1.0 - k) * 255.0) as u8);
            rgb_data.push(((1.0 - m) * (1.0 - k) * 255.0) as u8);
            rgb_data.push(((1.0 - y) * (1.0 - k) * 255.0) as u8);
        }
        ::image::RgbImage::from_raw(width, height, rgb_data)
            .map(::image::DynamicImage::ImageRgb8)
    } else {
        log::debug!("PDF image: colorspace={} data_len={} expected_rgb={} expected_gray={}", color_space, raw_data.len(), expected_rgb, expected_gray);
        None
    };

    if let Some(img) = dyn_image {
        let mut png_buf = Vec::new();
        if img.write_to(&mut Cursor::new(&mut png_buf), ::image::ImageFormat::Png).is_ok() {
            return Some(png_buf);
        }
    }

    None
}

/// Resolve the color space of a PDF image stream to a simple string.
fn resolve_color_space(doc: &lopdf::Document, stream: &lopdf::Stream) -> String {
    match stream.dict.get(b"ColorSpace").ok() {
        Some(lopdf::Object::Name(name)) => String::from_utf8_lossy(name).to_string(),
        Some(lopdf::Object::Array(arr)) => {
            // e.g. [/ICCBased 10 0 R] — resolve the profile to get channel count
            let base_name = arr.first()
                .and_then(|o| if let lopdf::Object::Name(n) = o { Some(n.clone()) } else { None })
                .unwrap_or_default();
            let base_str = String::from_utf8_lossy(&base_name).to_string();

            if base_str == "ICCBased" {
                // Get the ICC profile stream to determine channels
                if let Some(lopdf::Object::Reference(r)) = arr.get(1) {
                    if let Ok(lopdf::Object::Stream(ref profile)) = doc.get_object(*r) {
                        let n = profile.dict.get(b"N")
                            .ok()
                            .and_then(|v| v.as_i64().ok())
                            .unwrap_or(3);
                        return format!("ICCBased-{}", n);
                    }
                }
            }
            base_str
        }
        Some(lopdf::Object::Reference(r)) => {
            if let Ok(obj) = doc.get_object(*r) {
                if let Ok(name) = obj.as_name() {
                    return String::from_utf8_lossy(name).to_string();
                }
            }
            "DeviceRGB".to_string()
        }
        _ => "DeviceRGB".to_string(),
    }
}

/// Try to extract the main image from a single PDF page.
fn extract_images_from_page(doc: &lopdf::Document, page_id: lopdf::ObjectId) -> Option<Vec<u8>> {
    // Resolve Resources with inheritance
    let resources_obj = resolve_page_resources(doc, page_id)?;
    let resources = deref_object(doc, resources_obj)?;
    let resources_dict = resources.as_dict().ok()?;

    let xobjects_obj = resources_dict.get(b"XObject").ok()?;
    let xobjects = deref_object(doc, xobjects_obj)?;
    let xobjects_dict = xobjects.as_dict().ok()?;

    log::debug!("PDF page {:?}: found {} XObjects", page_id, xobjects_dict.len());

    // Collect all image streams (including from nested Form XObjects)
    let image_streams = collect_image_streams(doc, xobjects_dict, 0);

    log::debug!("PDF page {:?}: found {} image streams", page_id, image_streams.len());

    // Find the largest decodable image
    let mut best_image: Option<Vec<u8>> = None;
    let mut best_size: usize = 0;

    for stream in &image_streams {
        let width = stream.dict.get(b"Width")
            .ok()
            .and_then(|w| w.as_i64().ok())
            .unwrap_or(0) as usize;
        let height = stream.dict.get(b"Height")
            .ok()
            .and_then(|h| h.as_i64().ok())
            .unwrap_or(0) as usize;
        let pixel_count = width * height;

        if pixel_count <= best_size {
            continue;
        }

        if let Some(png) = decode_pdf_image(doc, stream) {
            best_size = pixel_count;
            best_image = Some(png);
        }
    }

    best_image
}

/// Extract each frame of a multi-page TIFF as a PNG image.
fn extract_tiff_pages(file_path: &str) -> Result<Vec<Vec<u8>>, ScannerError> {
    let file = std::fs::File::open(file_path)
        .map_err(|e| ScannerError::SystemError(format!("Lecture TIFF: {}", e)))?;
    let mut decoder = tiff::decoder::Decoder::new(std::io::BufReader::new(file))
        .map_err(|e| ScannerError::SystemError(format!("Décodage TIFF: {}", e)))?;

    let mut pages: Vec<Vec<u8>> = Vec::new();

    loop {
        let (width, height) = decoder.dimensions()
            .map_err(|e| ScannerError::SystemError(format!("Dimensions TIFF: {}", e)))?;

        let decode_result = decoder.read_image()
            .map_err(|e| ScannerError::SystemError(format!("Lecture frame TIFF: {}", e)))?;

        let dyn_image = match decode_result {
            tiff::decoder::DecodingResult::U8(data) => {
                let channels = data.len() / (width as usize * height as usize);
                match channels {
                    1 => ::image::GrayImage::from_raw(width, height, data)
                        .map(::image::DynamicImage::ImageLuma8),
                    3 => ::image::RgbImage::from_raw(width, height, data)
                        .map(::image::DynamicImage::ImageRgb8),
                    4 => ::image::RgbaImage::from_raw(width, height, data)
                        .map(::image::DynamicImage::ImageRgba8),
                    _ => None,
                }
            }
            tiff::decoder::DecodingResult::U16(data) => {
                // Convert 16-bit to 8-bit
                let data8: Vec<u8> = data.iter().map(|&v| (v >> 8) as u8).collect();
                let channels = data8.len() / (width as usize * height as usize);
                match channels {
                    1 => ::image::GrayImage::from_raw(width, height, data8)
                        .map(::image::DynamicImage::ImageLuma8),
                    3 => ::image::RgbImage::from_raw(width, height, data8)
                        .map(::image::DynamicImage::ImageRgb8),
                    _ => None,
                }
            }
            _ => None,
        };

        if let Some(img) = dyn_image {
            let mut png_buf = Vec::new();
            img.write_to(&mut Cursor::new(&mut png_buf), ::image::ImageFormat::Png)
                .map_err(|e| ScannerError::SystemError(format!("Conversion PNG: {}", e)))?;
            pages.push(png_buf);
        }

        // Try to move to the next frame
        match decoder.next_image() {
            Ok(()) => continue,
            Err(tiff::TiffError::FormatError(_)) => break,
            Err(tiff::TiffError::LimitsExceeded) => break,
            Err(_) => break,
        }
    }

    if pages.is_empty() {
        return Err(ScannerError::SystemError("Aucune page trouvée dans le TIFF".into()));
    }

    Ok(pages)
}

#[tauri::command]
async fn save_document_as_pdf(
    doc_id: String,
    output_path: String,
    export_options: Option<pdf_postprocess::PdfExportOptions>,
    annotations: Option<Vec<pdf_postprocess::PageAnnotations>>,
    state: tauri::State<'_, AppState>,
) -> Result<PdfSaveResult, ScannerError> {
    let (png_data, dpi) = {
        let docs = state.documents.lock().unwrap();
        let doc = docs
            .get(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("Document non trouvé en mémoire".into()))?;
        (doc.png_data.clone(), doc.dpi)
    };

    let ocr_result = {
        let cache = state.ocr_cache.lock().unwrap();
        cache.get(&doc_id).cloned()
    };

    processing::save_as_pdf(&png_data, &output_path, dpi, ocr_result.as_ref())?;

    // Post-processing pipeline
    let sha256 = pdf_postprocess::postprocess_pdf(
        &output_path,
        export_options.as_ref(),
        annotations.as_deref(),
    ).map_err(|e| ScannerError::SystemError(e))?;

    let mut history = storage::load_history();
    if let Some(entry) = history.iter_mut().find(|h| h.id == doc_id) {
        entry.file_path = Some(output_path.clone());
        entry.format = "PDF".to_string();
        let _ = storage::save_history(&history);
    }

    Ok(PdfSaveResult { path: output_path, sha256 })
}

#[tauri::command]
async fn save_document_as_image(
    doc_id: String,
    output_path: String,
    format: String,
    quality: u8,
    state: tauri::State<'_, AppState>,
) -> Result<String, ScannerError> {
    let png_data = {
        let docs = state.documents.lock().unwrap();
        let doc = docs
            .get(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("Document non trouvé en mémoire".into()))?;
        doc.png_data.clone()
    };

    processing::save_as_image(&png_data, &output_path, &format, quality)?;

    let mut history = storage::load_history();
    if let Some(entry) = history.iter_mut().find(|h| h.id == doc_id) {
        entry.file_path = Some(output_path.clone());
        entry.format = format;
        let _ = storage::save_history(&history);
    }

    Ok(output_path)
}

#[tauri::command]
async fn auto_crop_document(
    doc_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<ScanResultDto, ScannerError> {
    let (png_data, dpi) = {
        let docs = state.documents.lock().unwrap();
        let doc = docs
            .get(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("Document non trouvé".into()))?;
        (doc.png_data.clone(), doc.dpi)
    };

    push_version(&doc_id, &png_data, &state);

    let cropped = processing::auto_crop(&png_data)?;

    let img = ::image::load_from_memory(&cropped)
        .map_err(|e| ScannerError::SystemError(format!("Décodage: {}", e)))?;
    let width = img.width();
    let height = img.height();
    let image_base64 = BASE64.encode(&cropped);

    {
        let mut docs = state.documents.lock().unwrap();
        docs.insert(
            doc_id.clone(),
            DocumentData {
                png_data: cropped,
                original_png_data: None,
                width,
                height,
                dpi,
            },
        );
    }

    // Invalidate OCR cache
    state.ocr_cache.lock().unwrap().remove(&doc_id);

    let now = Local::now();
    Ok(ScanResultDto {
        id: doc_id,
        name: format!("Scan_recadré_{}.png", now.format("%H%M%S")),
        date: now.format("%d/%m/%Y %H:%M").to_string(),
        width,
        height,
        image_base64,
    })
}

#[tauri::command]
async fn print_document(
    doc_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), ScannerError> {
    let png_data = {
        let docs = state.documents.lock().unwrap();
        let doc = docs
            .get(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("Document non trouvé".into()))?;
        doc.png_data.clone()
    };

    let tmp_path = std::env::temp_dir().join(format!("print_{}.png", Uuid::new_v4()));
    std::fs::write(&tmp_path, &png_data)
        .map_err(|e| ScannerError::SystemError(format!("Fichier temporaire: {}", e)))?;

    let path_str = tmp_path.to_string_lossy().to_string();

    #[cfg(windows)]
    {
        use std::process::Command;
        Command::new("rundll32")
            .args(["shimgvw.dll,ImageView_PrintTo", &path_str])
            .spawn()
            .map_err(|e| ScannerError::SystemError(format!("Impression: {}", e)))?;
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        Command::new("lpr")
            .arg(&path_str)
            .spawn()
            .map_err(|e| ScannerError::SystemError(format!("Impression: {}", e)))?;
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        Command::new("lp")
            .arg(&path_str)
            .spawn()
            .map_err(|e| ScannerError::SystemError(format!("Impression: {}", e)))?;
    }

    Ok(())
}

#[tauri::command]
async fn print_multipage_document(
    multipage_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), ScannerError> {
    // Collect all pages' PNG data
    let page_data: Vec<(Vec<u8>, u32)> = {
        let mp_docs = state.multi_page_docs.lock().unwrap();
        let mp = mp_docs
            .get(&multipage_id)
            .ok_or_else(|| ScannerError::SystemError("Document multi-pages non trouvé".into()))?;
        let docs = state.documents.lock().unwrap();
        mp.page_ids.iter().map(|pid| {
            let doc = docs.get(pid).ok_or_else(|| ScannerError::SystemError("Page non trouvée".into()))?;
            Ok((doc.png_data.clone(), doc.dpi))
        }).collect::<Result<Vec<_>, ScannerError>>()?
    };

    // Generate a temporary PDF with all pages
    let pages: Vec<processing::PageData> = page_data.iter().map(|(data, dpi)| {
        processing::PageData { png_data: data, dpi: *dpi, ocr: None }
    }).collect();

    let tmp_path = std::env::temp_dir().join(format!("print_{}.pdf", Uuid::new_v4()));
    let tmp_str = tmp_path.to_string_lossy().to_string();
    processing::save_as_pdf_multipage(&pages, &tmp_str, "Impression")?;

    // Print the PDF
    #[cfg(windows)]
    {
        use std::process::Command;
        Command::new("cmd")
            .args(["/c", "start", "/min", "", &tmp_str])
            .spawn()
            .map_err(|e| ScannerError::SystemError(format!("Impression: {}", e)))?;
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        Command::new("lpr")
            .arg(&tmp_str)
            .spawn()
            .map_err(|e| ScannerError::SystemError(format!("Impression: {}", e)))?;
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        Command::new("lp")
            .arg(&tmp_str)
            .spawn()
            .map_err(|e| ScannerError::SystemError(format!("Impression: {}", e)))?;
    }

    Ok(())
}

#[tauri::command]
async fn load_settings(
    state: tauri::State<'_, AppState>,
) -> Result<AppSettings, ScannerError> {
    let settings = state.settings.lock().unwrap().clone();
    Ok(settings)
}

#[tauri::command]
async fn save_app_settings(
    settings: AppSettings,
    state: tauri::State<'_, AppState>,
) -> Result<(), ScannerError> {
    storage::save_settings(&settings)?;
    storage::ensure_output_dir(&settings.output_dir)?;
    let mut current = state.settings.lock().unwrap();
    *current = settings;
    Ok(())
}

#[tauri::command]
async fn get_documents_dir() -> Result<String, ScannerError> {
    let settings = storage::load_settings();
    storage::ensure_output_dir(&settings.output_dir)?;
    Ok(settings.output_dir)
}

#[tauri::command]
async fn get_history() -> Result<Vec<HistoryEntryDto>, ScannerError> {
    let history = storage::load_history();
    Ok(history
        .into_iter()
        .map(|h| HistoryEntryDto {
            id: h.id,
            name: h.name,
            date: h.date,
            format: h.format,
            file_path: h.file_path,
            has_preview: false,
            has_ocr: h.ocr_text.is_some(),
            ocr_text: h.ocr_text,
        })
        .collect())
}

#[tauri::command]
async fn delete_history_entry(doc_id: String) -> Result<(), ScannerError> {
    let mut history = storage::load_history();
    history.retain(|h| h.id != doc_id);
    storage::save_history(&history)
}

#[tauri::command]
async fn get_document_preview(
    doc_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, ScannerError> {
    let docs = state.documents.lock().unwrap();
    if let Some(doc) = docs.get(&doc_id) {
        Ok(BASE64.encode(&doc.png_data))
    } else {
        Err(ScannerError::SystemError("Document non trouvé en mémoire".into()))
    }
}

#[tauri::command]
async fn run_ocr(
    doc_id: String,
    lang: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, ScannerError> {
    let png_data = {
        let docs = state.documents.lock().unwrap();
        let doc = docs
            .get(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("Document non trouvé en mémoire".into()))?;
        doc.png_data.clone()
    };

    let result = ocr::extract_text_with_boxes(&png_data, &lang)?;
    let text = result.text.clone();

    {
        let mut cache = state.ocr_cache.lock().unwrap();
        cache.insert(doc_id.clone(), result);
    }

    let mut history = storage::load_history();
    if let Some(entry) = history.iter_mut().find(|h| h.id == doc_id) {
        entry.ocr_text = Some(text.clone());
        entry.ocr_lang = Some(lang);
        let _ = storage::save_history(&history);
    }

    Ok(text)
}

#[tauri::command]
async fn search_documents(query: String) -> Result<Vec<HistoryEntryDto>, ScannerError> {
    let history = storage::load_history();
    let query_lower = query.to_lowercase();

    let results: Vec<HistoryEntryDto> = history
        .into_iter()
        .filter(|h| {
            h.name.to_lowercase().contains(&query_lower)
                || h.date.to_lowercase().contains(&query_lower)
                || h.ocr_text
                    .as_ref()
                    .map(|t| t.to_lowercase().contains(&query_lower))
                    .unwrap_or(false)
        })
        .map(|h| HistoryEntryDto {
            id: h.id,
            name: h.name,
            date: h.date,
            format: h.format,
            file_path: h.file_path,
            has_preview: false,
            has_ocr: h.ocr_text.is_some(),
            ocr_text: h.ocr_text,
        })
        .collect();

    Ok(results)
}

// ─── v0.3.0: Rotation & Flip ─────────────────────────────────────

#[tauri::command]
async fn rotate_document(
    doc_id: String,
    direction: String,
    state: tauri::State<'_, AppState>,
) -> Result<ScanResultDto, ScannerError> {
    let (png_data, dpi) = {
        let docs = state.documents.lock().unwrap();
        let doc = docs
            .get(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("Document non trouvé".into()))?;
        (doc.png_data.clone(), doc.dpi)
    };

    push_version(&doc_id, &png_data, &state);

    let angle = match direction.as_str() {
        "90" => RotationAngle::R90,
        "180" => RotationAngle::R180,
        "270" => RotationAngle::R270,
        _ => return Err(ScannerError::SystemError(format!("Angle invalide: {}", direction))),
    };

    let rotated = processing::rotate_image(&png_data, &angle)?;
    let img = ::image::load_from_memory(&rotated)
        .map_err(|e| ScannerError::SystemError(format!("Décodage: {}", e)))?;
    let (width, height) = (img.width(), img.height());
    let image_base64 = BASE64.encode(&rotated);

    {
        let mut docs = state.documents.lock().unwrap();
        docs.insert(doc_id.clone(), DocumentData {
            png_data: rotated,
            original_png_data: None,
            width,
            height,
            dpi,
        });
    }

    state.ocr_cache.lock().unwrap().remove(&doc_id);

    let now = Local::now();
    Ok(ScanResultDto {
        id: doc_id,
        name: format!("Scan_rotation{}_{}.png", direction, now.format("%H%M%S")),
        date: now.format("%d/%m/%Y %H:%M").to_string(),
        width,
        height,
        image_base64,
    })
}

#[tauri::command]
async fn flip_document(
    doc_id: String,
    axis: String,
    state: tauri::State<'_, AppState>,
) -> Result<ScanResultDto, ScannerError> {
    let (png_data, dpi) = {
        let docs = state.documents.lock().unwrap();
        let doc = docs
            .get(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("Document non trouvé".into()))?;
        (doc.png_data.clone(), doc.dpi)
    };

    push_version(&doc_id, &png_data, &state);

    let flip_axis = match axis.as_str() {
        "horizontal" => FlipAxis::Horizontal,
        "vertical" => FlipAxis::Vertical,
        _ => return Err(ScannerError::SystemError(format!("Axe invalide: {}", axis))),
    };

    let flipped = processing::flip_image(&png_data, &flip_axis)?;
    let img = ::image::load_from_memory(&flipped)
        .map_err(|e| ScannerError::SystemError(format!("Décodage: {}", e)))?;
    let (width, height) = (img.width(), img.height());
    let image_base64 = BASE64.encode(&flipped);

    {
        let mut docs = state.documents.lock().unwrap();
        docs.insert(doc_id.clone(), DocumentData {
            png_data: flipped,
            original_png_data: None,
            width,
            height,
            dpi,
        });
    }

    state.ocr_cache.lock().unwrap().remove(&doc_id);

    let now = Local::now();
    Ok(ScanResultDto {
        id: doc_id,
        name: format!("Scan_miroir_{}.png", now.format("%H%M%S")),
        date: now.format("%d/%m/%Y %H:%M").to_string(),
        width,
        height,
        image_base64,
    })
}

// ─── v0.3.0: Image Adjustments ───────────────────────────────────

#[tauri::command]
async fn preview_adjustments(
    doc_id: String,
    adjustments: ImageAdjustments,
    state: tauri::State<'_, AppState>,
) -> Result<AdjustmentPreviewResult, ScannerError> {
    let source_data = {
        let mut docs = state.documents.lock().unwrap();
        let doc = docs
            .get_mut(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("Document non trouvé".into()))?;

        // Save original on first preview call
        if doc.original_png_data.is_none() {
            doc.original_png_data = Some(doc.png_data.clone());
        }

        doc.original_png_data.as_ref().unwrap().clone()
    };

    let adjusted = processing::apply_adjustments(&source_data, &adjustments)?;
    let img = ::image::load_from_memory(&adjusted)
        .map_err(|e| ScannerError::SystemError(format!("Décodage: {}", e)))?;

    Ok(AdjustmentPreviewResult {
        image_base64: BASE64.encode(&adjusted),
        width: img.width(),
        height: img.height(),
    })
}

#[tauri::command]
async fn commit_adjustments(
    doc_id: String,
    adjustments: ImageAdjustments,
    state: tauri::State<'_, AppState>,
) -> Result<ScanResultDto, ScannerError> {
    let source_data = {
        let docs = state.documents.lock().unwrap();
        let doc = docs
            .get(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("Document non trouvé".into()))?;
        doc.original_png_data.as_ref().unwrap_or(&doc.png_data).clone()
    };

    let adjusted = processing::apply_adjustments(&source_data, &adjustments)?;
    let img = ::image::load_from_memory(&adjusted)
        .map_err(|e| ScannerError::SystemError(format!("Décodage: {}", e)))?;
    let (width, height) = (img.width(), img.height());
    let image_base64 = BASE64.encode(&adjusted);

    {
        let mut docs = state.documents.lock().unwrap();
        let dpi = docs
            .get(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("Document non trouvé".into()))?
            .dpi;
        docs.insert(doc_id.clone(), DocumentData {
            png_data: adjusted,
            original_png_data: None,
            width,
            height,
            dpi,
        });
    }

    let now = Local::now();
    Ok(ScanResultDto {
        id: doc_id,
        name: format!("Scan_ajusté_{}.png", now.format("%H%M%S")),
        date: now.format("%d/%m/%Y %H:%M").to_string(),
        width,
        height,
        image_base64,
    })
}

#[tauri::command]
async fn revert_adjustments(
    doc_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<ScanResultDto, ScannerError> {
    // Extract original data with a short-lived lock
    let (original_data, dpi) = {
        let mut docs = state.documents.lock().unwrap();
        let doc = docs
            .get_mut(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("Document non trouvé".into()))?;

        match doc.original_png_data.take() {
            Some(orig) => (orig, doc.dpi),
            None => {
                // No adjustments to revert — return current state
                let data = doc.png_data.clone();
                return Ok(ScanResultDto {
                    id: doc_id,
                    name: format!("Scan_{}.png", Local::now().format("%H%M%S")),
                    date: Local::now().format("%d/%m/%Y %H:%M").to_string(),
                    width: doc.width,
                    height: doc.height,
                    image_base64: BASE64.encode(&data),
                });
            }
        }
    }; // Lock dropped here

    // Heavy work without holding the lock
    let img = ::image::load_from_memory(&original_data)
        .map_err(|e| ScannerError::SystemError(format!("Décodage: {}", e)))?;
    let (width, height) = (img.width(), img.height());
    let image_base64 = BASE64.encode(&original_data);

    // Re-acquire lock to update state
    {
        let mut docs = state.documents.lock().unwrap();
        docs.insert(doc_id.clone(), DocumentData {
            png_data: original_data,
            original_png_data: None,
            width,
            height,
            dpi,
        });
    }

    let now = Local::now();
    Ok(ScanResultDto {
        id: doc_id,
        name: format!("Scan_original_{}.png", now.format("%H%M%S")),
        date: now.format("%d/%m/%Y %H:%M").to_string(),
        width,
        height,
        image_base64,
    })
}

// ─── v0.3.0: Noise Reduction ─────────────────────────────────────

#[tauri::command]
async fn denoise_document(
    doc_id: String,
    strength: u8,
    state: tauri::State<'_, AppState>,
) -> Result<ScanResultDto, ScannerError> {
    let (png_data, dpi) = {
        let docs = state.documents.lock().unwrap();
        let doc = docs
            .get(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("Document non trouvé".into()))?;
        (doc.png_data.clone(), doc.dpi)
    };

    push_version(&doc_id, &png_data, &state);

    let denoised = processing::reduce_noise(&png_data, strength)?;
    let img = ::image::load_from_memory(&denoised)
        .map_err(|e| ScannerError::SystemError(format!("Décodage: {}", e)))?;
    let (width, height) = (img.width(), img.height());
    let image_base64 = BASE64.encode(&denoised);

    {
        let mut docs = state.documents.lock().unwrap();
        docs.insert(doc_id.clone(), DocumentData {
            png_data: denoised,
            original_png_data: None,
            width,
            height,
            dpi,
        });
    }

    let now = Local::now();
    Ok(ScanResultDto {
        id: doc_id,
        name: format!("Scan_débruité_{}.png", now.format("%H%M%S")),
        date: now.format("%d/%m/%Y %H:%M").to_string(),
        width,
        height,
        image_base64,
    })
}

// ─── v0.3.0: Deskew ──────────────────────────────────────────────

#[tauri::command]
async fn deskew_document(
    doc_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<ScanResultDto, ScannerError> {
    let (png_data, dpi) = {
        let docs = state.documents.lock().unwrap();
        let doc = docs
            .get(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("Document non trouvé".into()))?;
        (doc.png_data.clone(), doc.dpi)
    };

    push_version(&doc_id, &png_data, &state);

    let (corrected, angle) = processing::deskew(&png_data)?;
    let img = ::image::load_from_memory(&corrected)
        .map_err(|e| ScannerError::SystemError(format!("Décodage: {}", e)))?;
    let (width, height) = (img.width(), img.height());
    let image_base64 = BASE64.encode(&corrected);

    {
        let mut docs = state.documents.lock().unwrap();
        docs.insert(doc_id.clone(), DocumentData {
            png_data: corrected,
            original_png_data: None,
            width,
            height,
            dpi,
        });
    }

    state.ocr_cache.lock().unwrap().remove(&doc_id);

    let now = Local::now();
    Ok(ScanResultDto {
        id: doc_id,
        name: format!("Scan_redressé_{:.1}deg_{}.png", angle, now.format("%H%M%S")),
        date: now.format("%d/%m/%Y %H:%M").to_string(),
        width,
        height,
        image_base64,
    })
}

// ─── v0.3.0: Background Whitening ────────────────────────────────

#[tauri::command]
async fn whiten_document_background(
    doc_id: String,
    threshold: u8,
    state: tauri::State<'_, AppState>,
) -> Result<ScanResultDto, ScannerError> {
    let (png_data, dpi) = {
        let docs = state.documents.lock().unwrap();
        let doc = docs
            .get(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("Document non trouvé".into()))?;
        (doc.png_data.clone(), doc.dpi)
    };

    push_version(&doc_id, &png_data, &state);

    let whitened = processing::whiten_background(&png_data, threshold)?;
    let img = ::image::load_from_memory(&whitened)
        .map_err(|e| ScannerError::SystemError(format!("Décodage: {}", e)))?;
    let (width, height) = (img.width(), img.height());
    let image_base64 = BASE64.encode(&whitened);

    {
        let mut docs = state.documents.lock().unwrap();
        docs.insert(doc_id.clone(), DocumentData {
            png_data: whitened,
            original_png_data: None,
            width,
            height,
            dpi,
        });
    }

    let now = Local::now();
    Ok(ScanResultDto {
        id: doc_id,
        name: format!("Scan_blanchi_{}.png", now.format("%H%M%S")),
        date: now.format("%d/%m/%Y %H:%M").to_string(),
        width,
        height,
        image_base64,
    })
}

// ─── v0.3.0: Multi-Page Document Management ──────────────────────

#[tauri::command]
async fn create_multipage_document(
    name: String,
    state: tauri::State<'_, AppState>,
) -> Result<MultiPageDocDto, ScannerError> {
    let id = Uuid::new_v4().to_string();
    let now = Local::now();
    let created_at = now.format("%d/%m/%Y %H:%M").to_string();

    let doc = MultiPageDoc {
        id: id.clone(),
        name: name.clone(),
        page_ids: Vec::new(),
        created_at: created_at.clone(),
    };

    state.multi_page_docs.lock().unwrap().insert(id.clone(), doc);

    Ok(MultiPageDocDto {
        id,
        name,
        page_ids: Vec::new(),
        page_count: 0,
        created_at,
    })
}

#[tauri::command]
async fn add_page_to_document(
    multipage_id: String,
    doc_id: String,
    position: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<MultiPageDocDto, ScannerError> {
    // Verify the doc_id exists
    {
        let docs = state.documents.lock().unwrap();
        if !docs.contains_key(&doc_id) {
            return Err(ScannerError::SystemError("Document source non trouvé".into()));
        }
    }

    let mut mp_docs = state.multi_page_docs.lock().unwrap();
    let mp = mp_docs
        .get_mut(&multipage_id)
        .ok_or_else(|| ScannerError::SystemError("Document multi-pages non trouvé".into()))?;

    match position {
        Some(pos) if pos <= mp.page_ids.len() => mp.page_ids.insert(pos, doc_id),
        _ => mp.page_ids.push(doc_id),
    }

    Ok(MultiPageDocDto {
        id: mp.id.clone(),
        name: mp.name.clone(),
        page_ids: mp.page_ids.clone(),
        page_count: mp.page_ids.len(),
        created_at: mp.created_at.clone(),
    })
}

#[tauri::command]
async fn remove_page_from_document(
    multipage_id: String,
    page_index: usize,
    state: tauri::State<'_, AppState>,
) -> Result<MultiPageDocDto, ScannerError> {
    let mut mp_docs = state.multi_page_docs.lock().unwrap();
    let mp = mp_docs
        .get_mut(&multipage_id)
        .ok_or_else(|| ScannerError::SystemError("Document multi-pages non trouvé".into()))?;

    if page_index >= mp.page_ids.len() {
        return Err(ScannerError::SystemError("Index de page invalide".into()));
    }

    mp.page_ids.remove(page_index);

    Ok(MultiPageDocDto {
        id: mp.id.clone(),
        name: mp.name.clone(),
        page_ids: mp.page_ids.clone(),
        page_count: mp.page_ids.len(),
        created_at: mp.created_at.clone(),
    })
}

#[tauri::command]
async fn reorder_document_pages(
    multipage_id: String,
    new_order: Vec<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<MultiPageDocDto, ScannerError> {
    let mut mp_docs = state.multi_page_docs.lock().unwrap();
    let mp = mp_docs
        .get_mut(&multipage_id)
        .ok_or_else(|| ScannerError::SystemError("Document multi-pages non trouvé".into()))?;

    let page_count = mp.page_ids.len();
    if new_order.len() != page_count {
        return Err(ScannerError::SystemError("Nombre d'indices invalide".into()));
    }

    // Validate: new_order must be a permutation of 0..page_count
    let mut seen = vec![false; page_count];
    for &i in &new_order {
        if i >= page_count || seen[i] {
            return Err(ScannerError::SystemError("Ordre de pages invalide".into()));
        }
        seen[i] = true;
    }

    let old_ids = mp.page_ids.clone();
    mp.page_ids = new_order.iter().map(|&i| old_ids[i].clone()).collect();

    Ok(MultiPageDocDto {
        id: mp.id.clone(),
        name: mp.name.clone(),
        page_ids: mp.page_ids.clone(),
        page_count: mp.page_ids.len(),
        created_at: mp.created_at.clone(),
    })
}

#[tauri::command]
async fn list_multipage_documents(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<MultiPageDocDto>, ScannerError> {
    let mp_docs = state.multi_page_docs.lock().unwrap();
    Ok(mp_docs
        .values()
        .map(|mp| MultiPageDocDto {
            id: mp.id.clone(),
            name: mp.name.clone(),
            page_ids: mp.page_ids.clone(),
            page_count: mp.page_ids.len(),
            created_at: mp.created_at.clone(),
        })
        .collect())
}

#[tauri::command]
async fn save_multipage_as_pdf(
    multipage_id: String,
    output_path: String,
    export_options: Option<pdf_postprocess::PdfExportOptions>,
    annotations: Option<Vec<pdf_postprocess::PageAnnotations>>,
    state: tauri::State<'_, AppState>,
) -> Result<PdfSaveResult, ScannerError> {
    let page_ids = {
        let mp_docs = state.multi_page_docs.lock().unwrap();
        let mp = mp_docs
            .get(&multipage_id)
            .ok_or_else(|| ScannerError::SystemError("Document multi-pages non trouvé".into()))?;
        mp.page_ids.clone()
    };

    if page_ids.is_empty() {
        return Err(ScannerError::SystemError("Le document multi-pages est vide".into()));
    }

    // Collect all page data
    let mut pages_data: Vec<(Vec<u8>, u32)> = Vec::new();
    {
        let docs = state.documents.lock().unwrap();
        for pid in &page_ids {
            let doc = docs
                .get(pid)
                .ok_or_else(|| ScannerError::SystemError(format!("Page {} non trouvée", pid)))?;
            pages_data.push((doc.png_data.clone(), doc.dpi));
        }
    }

    let mut ocr_results: Vec<Option<OcrResult>> = Vec::new();
    {
        let cache = state.ocr_cache.lock().unwrap();
        for pid in &page_ids {
            ocr_results.push(cache.get(pid).cloned());
        }
    }

    let pages: Vec<PageData> = pages_data
        .iter()
        .zip(ocr_results.iter())
        .map(|((data, dpi), ocr)| PageData {
            png_data: data,
            dpi: *dpi,
            ocr: ocr.as_ref(),
        })
        .collect();

    processing::save_as_pdf_multipage(&pages, &output_path, "Document multi-pages")?;

    // Post-processing pipeline
    let sha256 = pdf_postprocess::postprocess_pdf(
        &output_path,
        export_options.as_ref(),
        annotations.as_deref(),
    ).map_err(|e| ScannerError::SystemError(e))?;

    Ok(PdfSaveResult { path: output_path, sha256 })
}

#[tauri::command]
async fn combine_documents_as_pdf(
    doc_ids: Vec<String>,
    output_path: String,
    export_options: Option<pdf_postprocess::PdfExportOptions>,
    annotations: Option<Vec<pdf_postprocess::PageAnnotations>>,
    state: tauri::State<'_, AppState>,
) -> Result<PdfSaveResult, ScannerError> {
    if doc_ids.is_empty() {
        return Err(ScannerError::SystemError("Aucun document à combiner".into()));
    }

    let mut pages_data: Vec<(Vec<u8>, u32)> = Vec::new();
    {
        let docs = state.documents.lock().unwrap();
        for did in &doc_ids {
            let doc = docs
                .get(did)
                .ok_or_else(|| ScannerError::SystemError(format!("Document {} non trouvé", did)))?;
            pages_data.push((doc.png_data.clone(), doc.dpi));
        }
    }

    let mut ocr_results: Vec<Option<OcrResult>> = Vec::new();
    {
        let cache = state.ocr_cache.lock().unwrap();
        for did in &doc_ids {
            ocr_results.push(cache.get(did).cloned());
        }
    }

    let pages: Vec<PageData> = pages_data
        .iter()
        .zip(ocr_results.iter())
        .map(|((data, dpi), ocr)| PageData {
            png_data: data,
            dpi: *dpi,
            ocr: ocr.as_ref(),
        })
        .collect();

    processing::save_as_pdf_multipage(&pages, &output_path, "Document combiné")?;

    // Post-processing pipeline
    let sha256 = pdf_postprocess::postprocess_pdf(
        &output_path,
        export_options.as_ref(),
        annotations.as_deref(),
    ).map_err(|e| ScannerError::SystemError(e))?;

    Ok(PdfSaveResult { path: output_path, sha256 })
}

// ─── v0.4.0: Scan Profiles ────────────────────────────────────────

#[tauri::command]
async fn list_scan_profiles() -> Result<Vec<ScanProfile>, ScannerError> {
    Ok(storage::load_profiles())
}

#[tauri::command]
async fn save_scan_profile(profile: ScanProfile) -> Result<Vec<ScanProfile>, ScannerError> {
    let mut profiles = storage::load_profiles();
    if let Some(existing) = profiles.iter_mut().find(|p| p.id == profile.id) {
        *existing = profile;
    } else {
        profiles.push(profile);
    }
    storage::save_profiles(&profiles)?;
    Ok(profiles)
}

#[tauri::command]
async fn delete_scan_profile(profile_id: String) -> Result<Vec<ScanProfile>, ScannerError> {
    let mut profiles = storage::load_profiles();
    profiles.retain(|p| p.id != profile_id);
    storage::save_profiles(&profiles)?;
    Ok(profiles)
}

// ─── v0.4.0: Batch Scanning ──────────────────────────────────────

#[tauri::command]
async fn batch_scan(
    options: ScanOptions,
    page_count: usize,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ScanResultDto>, ScannerError> {
    let mut results = Vec::new();

    let settings = state.settings.lock().unwrap().clone();

    for i in 0..page_count {
        let opts = options.clone();
        let scan_result = tokio::task::spawn_blocking(move || {
            let backend = scanner::get_backend();
            backend.scan(opts)
        })
        .await
        .map_err(|e| ScannerError::SystemError(format!("Thread join: {}", e)))??;

        let final_data = if settings.auto_crop {
            processing::auto_crop(&scan_result.image_data).unwrap_or(scan_result.image_data.clone())
        } else {
            scan_result.image_data.clone()
        };

        let (width, height) = if settings.auto_crop {
            if let Ok(img) = ::image::load_from_memory(&final_data) {
                (img.width(), img.height())
            } else {
                (scan_result.width, scan_result.height)
            }
        } else {
            (scan_result.width, scan_result.height)
        };

        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        // Increment counter
        let counter = {
            let mut s = state.settings.lock().unwrap();
            s.scan_counter += 1;
            s.scan_counter
        };

        let name = format!(
            "{}_{}.png",
            storage::expand_naming_template(
                &settings.naming_template,
                options.dpi,
                &options.color_mode,
                &settings.default_format,
                counter,
            ),
            i + 1
        );
        let date = now.format("%d/%m/%Y %H:%M").to_string();
        let image_base64 = BASE64.encode(&final_data);

        {
            let mut docs = state.documents.lock().unwrap();
            docs.insert(
                id.clone(),
                DocumentData {
                    png_data: final_data.clone(),
                    original_png_data: None,
                    width,
                    height,
                    dpi: options.dpi,
                },
            );
        }

        // Auto-OCR
        let mut ocr_text: Option<String> = None;
        let mut ocr_lang_used: Option<String> = None;
        if settings.auto_ocr {
            if let Ok(ocr_result) = ocr::extract_text_with_boxes(&final_data, &settings.default_ocr_lang) {
                if !ocr_result.text.is_empty() {
                    ocr_text = Some(ocr_result.text.clone());
                    ocr_lang_used = Some(ocr_result.lang.clone());
                    state.ocr_cache.lock().unwrap().insert(id.clone(), ocr_result);
                }
            }
        }

        // Auto-export to watch folder
        if let Some(ref watch_dir) = settings.watch_folder {
            let ext = settings.default_format.to_lowercase();
            let export_name = format!(
                "{}.{}",
                storage::expand_naming_template(
                    &settings.naming_template,
                    options.dpi,
                    &options.color_mode,
                    &settings.default_format,
                    counter,
                ),
                ext
            );
            let export_path = std::path::Path::new(watch_dir).join(&export_name);
            let _ = auto_export_document(&final_data, export_path.to_string_lossy().as_ref(), &settings, &id, &state);
        }

        let _ = storage::add_to_history(DocumentMeta {
            id: id.clone(),
            name: name.clone(),
            date: date.clone(),
            file_path: None,
            format: "PNG".to_string(),
            size_bytes: 0,
            width,
            height,
            dpi: options.dpi,
            ocr_text,
            ocr_lang: ocr_lang_used,
        });

        results.push(ScanResultDto {
            id,
            name,
            date,
            width,
            height,
            image_base64,
        });
    }

    // Persist updated counter
    {
        let s = state.settings.lock().unwrap().clone();
        let _ = storage::save_settings(&s);
    }

    Ok(results)
}

/// Auto-exports a document to the given path based on format settings.
fn auto_export_document(
    png_data: &[u8],
    output_path: &str,
    settings: &AppSettings,
    doc_id: &str,
    state: &tauri::State<'_, AppState>,
) -> Result<(), ScannerError> {
    let _ = std::fs::create_dir_all(std::path::Path::new(output_path).parent().unwrap_or(std::path::Path::new(".")));

    match settings.default_format.to_uppercase().as_str() {
        "PDF" => {
            let ocr_result = state.ocr_cache.lock().unwrap().get(doc_id).cloned();
            processing::save_as_pdf(png_data, output_path, settings.default_dpi, ocr_result.as_ref())?;
        }
        fmt => {
            processing::save_as_image(png_data, output_path, fmt, settings.quality)?;
        }
    }
    Ok(())
}

// ─── v0.4.0: Naming Template ─────────────────────────────────────

#[tauri::command]
async fn preview_naming_template(
    template: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, ScannerError> {
    let settings = state.settings.lock().unwrap();
    let counter = settings.scan_counter + 1;
    Ok(storage::expand_naming_template(
        &template,
        settings.default_dpi,
        &settings.default_color_mode,
        &settings.default_format,
        counter,
    ))
}

// ─── v0.4.0: Document Actions (rename, duplicate) ───────────────

#[tauri::command]
async fn rename_document(
    doc_id: String,
    new_name: String,
) -> Result<(), ScannerError> {
    let mut history = storage::load_history();
    if let Some(entry) = history.iter_mut().find(|h| h.id == doc_id) {
        entry.name = new_name;
        storage::save_history(&history)?;
    }
    Ok(())
}

#[tauri::command]
async fn duplicate_document(
    doc_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<ScanResultDto, ScannerError> {
    let (png_data, width, height, dpi) = {
        let docs = state.documents.lock().unwrap();
        let doc = docs
            .get(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("Document non trouvé".into()))?;
        (doc.png_data.clone(), doc.width, doc.height, doc.dpi)
    };

    let new_id = Uuid::new_v4().to_string();
    let now = Local::now();
    let name = format!("Copie_{}.png", now.format("%H%M%S"));
    let date = now.format("%d/%m/%Y %H:%M").to_string();
    let image_base64 = BASE64.encode(&png_data);

    {
        let mut docs = state.documents.lock().unwrap();
        docs.insert(
            new_id.clone(),
            DocumentData {
                png_data: png_data.clone(),
                original_png_data: None,
                width,
                height,
                dpi,
            },
        );
    }

    // Copy OCR cache if present
    {
        let mut cache = state.ocr_cache.lock().unwrap();
        if let Some(ocr) = cache.get(&doc_id).cloned() {
            cache.insert(new_id.clone(), ocr);
        }
    }

    let _ = storage::add_to_history(DocumentMeta {
        id: new_id.clone(),
        name: name.clone(),
        date: date.clone(),
        file_path: None,
        format: "PNG".to_string(),
        size_bytes: 0,
        width,
        height,
        dpi,
        ocr_text: None,
        ocr_lang: None,
    });

    Ok(ScanResultDto {
        id: new_id,
        name,
        date,
        width,
        height,
        image_base64,
    })
}

// ─── v0.6.0: Classification & Extraction ─────────────────────────

#[derive(Serialize, Deserialize)]
struct AnalysisResultDto {
    classification: intelligence::ClassificationResult,
    extracted_data: intelligence::ExtractedData,
    suggestion: intelligence::SmartSuggestion,
    auto_tags: Vec<String>,
    rule_results: Vec<intelligence::RuleExecutionResult>,
}

#[tauri::command]
async fn analyze_document(
    doc_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<AnalysisResultDto, ScannerError> {
    // Get OCR text
    let ocr_text = {
        let cache = state.ocr_cache.lock().unwrap();
        cache.get(&doc_id).map(|r| r.text.clone())
    };

    let text = match ocr_text {
        Some(t) if !t.is_empty() => t,
        _ => {
            // Run OCR first if not cached
            let lang = {
                let settings = state.settings.lock().unwrap();
                settings.default_ocr_lang.clone()
            };
            let png_data = {
                let docs = state.documents.lock().unwrap();
                let doc = docs
                    .get(&doc_id)
                    .ok_or_else(|| ScannerError::SystemError("Document non trouvé".into()))?;
                doc.png_data.clone()
            };

            let result = ocr::extract_text_with_boxes(&png_data, &lang)?;
            let text = result.text.clone();
            state.ocr_cache.lock().unwrap().insert(doc_id.clone(), result);
            text
        }
    };

    // Classify
    let classification = intelligence::classify_document(&text);

    // Extract data
    let extracted_data = intelligence::extract_data(&text);

    // Generate suggestions
    let suggestion = intelligence::generate_suggestions(&classification, &extracted_data);

    // Auto-generate tags
    let auto_tags = suggestion.suggested_tags.clone();

    // Evaluate automation rules
    let rules = storage::load_rules();
    let current_tags = {
        let tags_map = storage::load_tags();
        tags_map.get(&doc_id).cloned().unwrap_or_default()
    };

    let all_tags: Vec<String> = current_tags.iter().chain(auto_tags.iter()).cloned().collect();
    let ctx = RuleContext {
        classification: &classification,
        extracted_data: &extracted_data,
        tags: &all_tags,
        ocr_text: &text,
    };
    let rule_results = intelligence::evaluate_rules(&rules, &ctx);

    Ok(AnalysisResultDto {
        classification,
        extracted_data,
        suggestion,
        auto_tags,
        rule_results,
    })
}

// ─── v0.6.0: Tags ────────────────────────────────────────────────

#[tauri::command]
async fn get_document_tags(doc_id: String) -> Result<Vec<String>, ScannerError> {
    let tags_map = storage::load_tags();
    Ok(tags_map.get(&doc_id).cloned().unwrap_or_default())
}

#[tauri::command]
async fn set_document_tags(doc_id: String, tags: Vec<String>) -> Result<(), ScannerError> {
    let mut tags_map = storage::load_tags();
    if tags.is_empty() {
        tags_map.remove(&doc_id);
    } else {
        tags_map.insert(doc_id, tags);
    }
    storage::save_tags(&tags_map)
}

#[tauri::command]
async fn add_document_tag(doc_id: String, tag: String) -> Result<Vec<String>, ScannerError> {
    let mut tags_map = storage::load_tags();
    let entry = tags_map.entry(doc_id).or_default();
    if !entry.contains(&tag) {
        entry.push(tag);
    }
    let result = entry.clone();
    storage::save_tags(&tags_map)?;
    Ok(result)
}

#[tauri::command]
async fn remove_document_tag(doc_id: String, tag: String) -> Result<Vec<String>, ScannerError> {
    let mut tags_map = storage::load_tags();
    if let Some(entry) = tags_map.get_mut(&doc_id) {
        entry.retain(|t| t != &tag);
        let result = entry.clone();
        storage::save_tags(&tags_map)?;
        Ok(result)
    } else {
        Ok(Vec::new())
    }
}

#[tauri::command]
async fn get_tag_definitions() -> Result<Vec<TagDefinition>, ScannerError> {
    Ok(storage::load_tag_definitions())
}

#[tauri::command]
async fn save_tag_definitions_cmd(definitions: Vec<TagDefinition>) -> Result<(), ScannerError> {
    storage::save_tag_definitions(&definitions)
}

#[tauri::command]
async fn get_all_tags_map() -> Result<HashMap<String, Vec<String>>, ScannerError> {
    Ok(storage::load_tags())
}

// ─── v0.6.0: Automation Rules ────────────────────────────────────

#[tauri::command]
async fn list_automation_rules() -> Result<Vec<AutomationRule>, ScannerError> {
    Ok(storage::load_rules())
}

#[tauri::command]
async fn save_automation_rule(rule: AutomationRule) -> Result<Vec<AutomationRule>, ScannerError> {
    // Validate regex patterns at save time
    for condition in &rule.conditions {
        if matches!(condition.operator, intelligence::ConditionOperator::Regex) {
            regex::Regex::new(&condition.value).map_err(|e| {
                ScannerError::SystemError(format!("Regex invalide: {}", e))
            })?;
        }
    }
    let mut rules = storage::load_rules();
    if let Some(existing) = rules.iter_mut().find(|r| r.id == rule.id) {
        *existing = rule;
    } else {
        rules.push(rule);
    }
    storage::save_rules(&rules)?;
    Ok(rules)
}

#[tauri::command]
async fn delete_automation_rule(rule_id: String) -> Result<Vec<AutomationRule>, ScannerError> {
    let mut rules = storage::load_rules();
    rules.retain(|r| r.id != rule_id);
    storage::save_rules(&rules)?;
    Ok(rules)
}

#[tauri::command]
async fn apply_rule_actions(
    doc_id: String,
    actions: Vec<intelligence::RuleAction>,
    state: tauri::State<'_, AppState>,
) -> Result<(), ScannerError> {
    for action in &actions {
        match &action.action_type {
            intelligence::ActionType::Rename => {
                let mut history = storage::load_history();
                if let Some(entry) = history.iter_mut().find(|h| h.id == doc_id) {
                    entry.name = action.value.clone();
                    let _ = storage::save_history(&history);
                }
            }
            intelligence::ActionType::MoveToFolder => {
                let settings = state.settings.lock().unwrap().clone();
                let output_dir = std::path::Path::new(&settings.output_dir);
                let target_dir = output_dir.join(&action.value);
                // Validate path doesn't escape output_dir
                let canonical_output = std::fs::canonicalize(output_dir).unwrap_or_else(|_| output_dir.to_path_buf());
                let _ = std::fs::create_dir_all(&target_dir);
                let canonical_target = std::fs::canonicalize(&target_dir).unwrap_or_else(|_| target_dir.clone());
                if !canonical_target.starts_with(&canonical_output) {
                    continue; // Skip path traversal attempts
                }
                let mut history = storage::load_history();
                if let Some(entry) = history.iter_mut().find(|h| h.id == doc_id) {
                    if let Some(ref src_path) = entry.file_path.clone() {
                        let filename = std::path::Path::new(src_path)
                            .file_name()
                            .unwrap_or_default();
                        let dest = target_dir.join(filename);
                        if std::fs::rename(src_path, &dest).is_ok() {
                            entry.file_path = Some(dest.to_string_lossy().to_string());
                            let _ = storage::save_history(&history);
                        }
                    }
                }
            }
            intelligence::ActionType::AddTag => {
                let mut tags_map = storage::load_tags();
                let entry = tags_map.entry(doc_id.clone()).or_default();
                if !entry.contains(&action.value) {
                    entry.push(action.value.clone());
                }
                let _ = storage::save_tags(&tags_map);
            }
            intelligence::ActionType::ApplyProfile => {
                // Apply profile settings — this is a frontend concern mainly,
                // but we can store a hint for next scan
                let _ = &action.value; // Profile ID stored for reference
            }
        }
    }
    Ok(())
}

// ─── v1.1.0: Vault Commands ───────────────────────────────────────

#[tauri::command]
async fn vault_is_setup(
    state: tauri::State<'_, AppState>,
) -> Result<bool, ScannerError> {
    Ok(state.vault_manager.lock().unwrap().is_setup())
}

#[tauri::command]
async fn vault_is_unlocked(
    state: tauri::State<'_, AppState>,
) -> Result<bool, ScannerError> {
    Ok(state.vault_manager.lock().unwrap().is_unlocked())
}

#[tauri::command]
async fn vault_set_password(
    password: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), ScannerError> {
    state.vault_manager.lock().unwrap().set_password(&password)
}

#[tauri::command]
async fn vault_unlock(
    password: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), ScannerError> {
    state.vault_manager.lock().unwrap().unlock(&password)
}

#[tauri::command]
async fn vault_lock(
    state: tauri::State<'_, AppState>,
) -> Result<(), ScannerError> {
    state.vault_manager.lock().unwrap().lock();
    Ok(())
}

#[tauri::command]
async fn vault_add_document(
    doc_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), ScannerError> {
    let (png_data, name) = {
        let docs = state.documents.lock().unwrap();
        let doc = docs
            .get(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("Document non trouvé".into()))?;
        (doc.png_data.clone(), format!("vault_doc_{}", doc_id))
    };

    // Try to get name from history
    let history = storage::load_history();
    let doc_name = history
        .iter()
        .find(|h| h.id == doc_id)
        .map(|h| h.name.clone())
        .unwrap_or(name);

    state
        .vault_manager
        .lock()
        .unwrap()
        .add_document(&doc_id, &doc_name, &png_data)
}

#[tauri::command]
async fn vault_list_documents(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<vault::VaultDocDto>, ScannerError> {
    state.vault_manager.lock().unwrap().list_documents()
}

#[tauri::command]
async fn vault_open_document(
    doc_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<ScanResultDto, ScannerError> {
    let png_data = state
        .vault_manager
        .lock()
        .unwrap()
        .open_document(&doc_id)?;

    let img = ::image::load_from_memory(&png_data)
        .map_err(|e| ScannerError::SystemError(format!("Décodage: {}", e)))?;
    let width = img.width();
    let height = img.height();

    let new_id = Uuid::new_v4().to_string();
    let image_base64 = BASE64.encode(&png_data);

    {
        let mut docs = state.documents.lock().unwrap();
        docs.insert(
            new_id.clone(),
            DocumentData {
                png_data,
                original_png_data: None,
                width,
                height,
                dpi: 300,
            },
        );
    }

    let now = Local::now();
    Ok(ScanResultDto {
        id: new_id,
        name: format!("Vault_{}.png", now.format("%H%M%S")),
        date: now.format("%d/%m/%Y %H:%M").to_string(),
        width,
        height,
        image_base64,
    })
}

#[tauri::command]
async fn vault_remove_document(
    doc_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), ScannerError> {
    state
        .vault_manager
        .lock()
        .unwrap()
        .remove_document(&doc_id)
}

// ─── v1.2.0: Version History ──────────────────────────────────────

/// Push current state to version history before a destructive operation.
fn push_version(doc_id: &str, png_data: &[u8], state: &tauri::State<'_, AppState>) {
    let mut history = state.version_history.lock().unwrap();
    let stack = history.entry(doc_id.to_string()).or_default();
    stack.push(png_data.to_vec());
    if stack.len() > 20 {
        stack.remove(0); // Keep max 20 versions
    }
}

#[derive(Serialize)]
struct VersionInfo {
    index: usize,
    size_bytes: usize,
}

#[tauri::command]
async fn get_document_versions(
    doc_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<VersionInfo>, ScannerError> {
    let history = state.version_history.lock().unwrap();
    Ok(history
        .get(&doc_id)
        .map(|stack| {
            stack
                .iter()
                .enumerate()
                .map(|(i, data)| VersionInfo {
                    index: i,
                    size_bytes: data.len(),
                })
                .collect()
        })
        .unwrap_or_default())
}

#[tauri::command]
async fn undo_last_change(
    doc_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<ScanResultDto, ScannerError> {
    let png_data = {
        let mut history = state.version_history.lock().unwrap();
        let stack = history
            .get_mut(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("No version history".into()))?;
        stack
            .pop()
            .ok_or_else(|| ScannerError::SystemError("No more versions to undo".into()))?
    };

    let img = ::image::load_from_memory(&png_data)
        .map_err(|e| ScannerError::SystemError(format!("Décodage: {}", e)))?;
    let (width, height) = (img.width(), img.height());
    let image_base64 = BASE64.encode(&png_data);

    let dpi = {
        let docs = state.documents.lock().unwrap();
        docs.get(&doc_id).map(|d| d.dpi).unwrap_or(300)
    };

    {
        let mut docs = state.documents.lock().unwrap();
        docs.insert(
            doc_id.clone(),
            DocumentData {
                png_data,
                original_png_data: None,
                width,
                height,
                dpi,
            },
        );
    }

    state.ocr_cache.lock().unwrap().remove(&doc_id);

    let now = Local::now();
    Ok(ScanResultDto {
        id: doc_id,
        name: format!("Undo_{}.png", now.format("%H%M%S")),
        date: now.format("%d/%m/%Y %H:%M").to_string(),
        width,
        height,
        image_base64,
    })
}

#[tauri::command]
async fn rollback_to_version(
    doc_id: String,
    version_index: usize,
    state: tauri::State<'_, AppState>,
) -> Result<ScanResultDto, ScannerError> {
    let png_data = {
        let mut history = state.version_history.lock().unwrap();
        let stack = history
            .get_mut(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("No version history".into()))?;
        if version_index >= stack.len() {
            return Err(ScannerError::SystemError("Invalid version index".into()));
        }
        let data = stack[version_index].clone();
        stack.truncate(version_index);
        data
    };

    let img = ::image::load_from_memory(&png_data)
        .map_err(|e| ScannerError::SystemError(format!("Décodage: {}", e)))?;
    let (width, height) = (img.width(), img.height());
    let image_base64 = BASE64.encode(&png_data);

    let dpi = {
        let docs = state.documents.lock().unwrap();
        docs.get(&doc_id).map(|d| d.dpi).unwrap_or(300)
    };

    {
        let mut docs = state.documents.lock().unwrap();
        docs.insert(
            doc_id.clone(),
            DocumentData {
                png_data,
                original_png_data: None,
                width,
                height,
                dpi,
            },
        );
    }

    state.ocr_cache.lock().unwrap().remove(&doc_id);

    let now = Local::now();
    Ok(ScanResultDto {
        id: doc_id,
        name: format!("Rollback_{}.png", now.format("%H%M%S")),
        date: now.format("%d/%m/%Y %H:%M").to_string(),
        width,
        height,
        image_base64,
    })
}

// ─── v1.2.0: Language Detection ───────────────────────────────────

#[derive(Serialize)]
struct DetectedLanguage {
    lang_code: String,
    lang_name: String,
    confidence: f64,
    tesseract_code: String,
}

#[tauri::command]
async fn detect_document_language(
    doc_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<DetectedLanguage, ScannerError> {
    let ocr_text = {
        let cache = state.ocr_cache.lock().unwrap();
        cache.get(&doc_id).map(|r| r.text.clone())
    };

    let text = ocr_text.ok_or_else(|| {
        ScannerError::SystemError("Run OCR first to detect language".into())
    })?;

    let info = whatlang::detect(&text)
        .ok_or_else(|| ScannerError::SystemError("Could not detect language".into()))?;

    let tesseract_code = match info.lang() {
        whatlang::Lang::Fra => "fra",
        whatlang::Lang::Eng => "eng",
        whatlang::Lang::Deu => "deu",
        whatlang::Lang::Spa => "spa",
        whatlang::Lang::Ita => "ita",
        whatlang::Lang::Por => "por",
        whatlang::Lang::Nld => "nld",
        whatlang::Lang::Pol => "pol",
        whatlang::Lang::Rus => "rus",
        whatlang::Lang::Jpn => "jpn",
        whatlang::Lang::Cmn => "chi_sim",
        whatlang::Lang::Kor => "kor",
        whatlang::Lang::Ara => "ara",
        whatlang::Lang::Tur => "tur",
        _ => "eng",
    }
    .to_string();

    Ok(DetectedLanguage {
        lang_code: format!("{:?}", info.lang()),
        lang_name: format!("{:?}", info.lang()),
        confidence: info.confidence(),
        tesseract_code,
    })
}

// ─── v1.2.0: Templates ────────────────────────────────────────────

#[tauri::command]
async fn list_templates() -> Result<Vec<templates::DocumentTemplate>, ScannerError> {
    Ok(templates::load_templates())
}

#[tauri::command]
async fn save_template(template: templates::DocumentTemplate) -> Result<Vec<templates::DocumentTemplate>, ScannerError> {
    let mut all = templates::load_templates();
    if let Some(existing) = all.iter_mut().find(|t| t.id == template.id) {
        *existing = template;
    } else {
        all.push(template);
    }
    templates::save_templates(&all)?;
    Ok(all)
}

#[tauri::command]
async fn delete_template(template_id: String) -> Result<Vec<templates::DocumentTemplate>, ScannerError> {
    let mut all = templates::load_templates();
    all.retain(|t| t.id != template_id);
    templates::save_templates(&all)?;
    Ok(all)
}

#[tauri::command]
async fn apply_template(
    doc_id: String,
    template_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<templates::TemplateResult, ScannerError> {
    let png_data = {
        let docs = state.documents.lock().unwrap();
        let doc = docs.get(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("Document non trouvé".into()))?;
        doc.png_data.clone()
    };

    let all = templates::load_templates();
    let template = all.iter().find(|t| t.id == template_id)
        .ok_or_else(|| ScannerError::SystemError("Template non trouvé".into()))?;

    let lang = {
        let s = state.settings.lock().unwrap();
        s.default_ocr_lang.clone()
    };

    templates::apply_template(&png_data, template, &lang)
}

// ─── v1.2.0: Document Comparison ──────────────────────────────────

#[tauri::command]
async fn compare_documents(
    doc_id_a: String,
    doc_id_b: String,
    state: tauri::State<'_, AppState>,
) -> Result<comparison::ComparisonResult, ScannerError> {
    let (png_a, png_b) = {
        let docs = state.documents.lock().unwrap();
        let a = docs.get(&doc_id_a)
            .ok_or_else(|| ScannerError::SystemError("Document A non trouvé".into()))?
            .png_data.clone();
        let b = docs.get(&doc_id_b)
            .ok_or_else(|| ScannerError::SystemError("Document B non trouvé".into()))?
            .png_data.clone();
        (a, b)
    };

    let (text_a, text_b) = {
        let cache = state.ocr_cache.lock().unwrap();
        (
            cache.get(&doc_id_a).map(|r| r.text.clone()),
            cache.get(&doc_id_b).map(|r| r.text.clone()),
        )
    };

    comparison::compare_documents(
        &png_a,
        &png_b,
        text_a.as_deref(),
        text_b.as_deref(),
    )
}

// ─── v1.2.0: PDF Forms ───────────────────────────────────────────

#[tauri::command]
async fn detect_pdf_form_fields(
    file_path: String,
) -> Result<Vec<pdf_forms::FormField>, ScannerError> {
    pdf_forms::detect_form_fields(&file_path)
}

#[tauri::command]
async fn fill_pdf_form(
    file_path: String,
    field_values: Vec<(String, String)>,
) -> Result<(), ScannerError> {
    pdf_forms::fill_form(&file_path, &field_values)
}

// ─── v2.0.0: AI via Groq ─────────────────────────────────────────

#[tauri::command]
async fn ai_ocr_document(
    doc_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, ScannerError> {
    let (image_base64, api_key) = {
        let docs = state.documents.lock().unwrap();
        let doc = docs.get(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("Document non trouvé".into()))?;
        let b64 = BASE64.encode(&doc.png_data);
        let settings = state.settings.lock().unwrap();
        let key = settings.groq_api_key.clone().unwrap_or_default();
        (b64, key)
    };

    if api_key.is_empty() {
        return Err(ScannerError::SystemError("Groq API key not configured".into()));
    }

    let client = groq::GroqClient::new(&api_key);
    let result = client.vision(
        "llama-3.2-90b-vision-preview",
        "You are an OCR system. Extract ALL text visible in this document image. Return only the extracted text, preserving layout as much as possible. Do not add any commentary.",
        "Extract all text from this document image.",
        &image_base64,
    ).await?;

    // Cache the result
    {
        let mut cache = state.ocr_cache.lock().unwrap();
        cache.insert(doc_id.clone(), ocr::OcrResult {
            text: result.clone(),
            words: Vec::new(),
            lang: "ai".to_string(),
        });
    }

    Ok(result)
}

#[tauri::command]
async fn summarize_document(
    doc_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, ScannerError> {
    let (text, api_key) = {
        let cache = state.ocr_cache.lock().unwrap();
        let text = cache.get(&doc_id).map(|r| r.text.clone())
            .ok_or_else(|| ScannerError::SystemError("Run OCR first".into()))?;
        let settings = state.settings.lock().unwrap();
        let key = settings.groq_api_key.clone().unwrap_or_default();
        (text, key)
    };

    if api_key.is_empty() {
        return Err(ScannerError::SystemError("Groq API key not configured".into()));
    }

    let client = groq::GroqClient::new(&api_key);
    client.chat(
        "llama-3.3-70b-versatile",
        "You are a document summarization assistant. Provide a clear, concise summary of the document text. Highlight key information: dates, amounts, names, and important details. Use bullet points for clarity.",
        &text,
        Some(0.3),
        Some(1024),
    ).await
}

#[tauri::command]
async fn translate_document(
    doc_id: String,
    target_lang: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, ScannerError> {
    let (text, api_key) = {
        let cache = state.ocr_cache.lock().unwrap();
        let text = cache.get(&doc_id).map(|r| r.text.clone())
            .ok_or_else(|| ScannerError::SystemError("Run OCR first".into()))?;
        let settings = state.settings.lock().unwrap();
        let key = settings.groq_api_key.clone().unwrap_or_default();
        (text, key)
    };

    if api_key.is_empty() {
        return Err(ScannerError::SystemError("Groq API key not configured".into()));
    }

    let client = groq::GroqClient::new(&api_key);
    client.chat(
        "llama-3.3-70b-versatile",
        &format!("You are a professional translator. Translate the following document text to {}. Preserve formatting and structure. Return only the translation, no commentary.", target_lang),
        &text,
        Some(0.2),
        Some(4096),
    ).await
}

#[tauri::command]
async fn semantic_search(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<search::SearchResult>, ScannerError> {
    // Build corpus from OCR cache + history
    let history = storage::load_history();
    let cache = state.ocr_cache.lock().unwrap();

    let mut corpus: Vec<(String, String, String)> = Vec::new();

    for entry in &history {
        let text = cache
            .get(&entry.id)
            .map(|r| r.text.clone())
            .or_else(|| entry.ocr_text.clone())
            .unwrap_or_default();

        if !text.is_empty() {
            corpus.push((entry.id.clone(), entry.name.clone(), text));
        }
    }

    Ok(search::semantic_search(&query, &corpus, 20))
}

// ─── v2.0.0: Auto Redaction ──────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct SensitiveItem {
    text: String,
    category: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[tauri::command]
async fn detect_sensitive_info(
    doc_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SensitiveItem>, ScannerError> {
    let (words, img_width, img_height) = {
        let cache = state.ocr_cache.lock().unwrap();
        let ocr = cache.get(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("Run OCR first".into()))?;
        let docs = state.documents.lock().unwrap();
        let doc = docs.get(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("Document non trouvé".into()))?;
        (ocr.words.clone(), doc.width, doc.height)
    };

    let patterns = intelligence::get_sensitive_patterns();
    let mut items = Vec::new();

    for word in &words {
        for (category, re) in &patterns {
            if re.is_match(&word.text) {
                items.push(SensitiveItem {
                    text: word.text.clone(),
                    category: category.to_string(),
                    x: word.x as f64 / img_width as f64,
                    y: word.y as f64 / img_height as f64,
                    width: word.w as f64 / img_width as f64,
                    height: word.h as f64 / img_height as f64,
                });
            }
        }
    }

    Ok(items)
}

#[tauri::command]
async fn apply_redactions(
    doc_id: String,
    redactions: Vec<SensitiveItem>,
    state: tauri::State<'_, AppState>,
) -> Result<ScanResultDto, ScannerError> {
    let (png_data, dpi) = {
        let docs = state.documents.lock().unwrap();
        let doc = docs.get(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("Document non trouvé".into()))?;
        (doc.png_data.clone(), doc.dpi)
    };

    push_version(&doc_id, &png_data, &state);

    let mut img = image::load_from_memory(&png_data)
        .map_err(|e| ScannerError::SystemError(format!("Décodage: {}", e)))?;
    let (w, h) = (img.width(), img.height());

    // Draw black rectangles over redacted areas
    let img_buf = img.as_mut_rgba8()
        .ok_or_else(|| ScannerError::SystemError("Image conversion failed".into()))?;

    for redaction in &redactions {
        let rx = (redaction.x * w as f64) as u32;
        let ry = (redaction.y * h as f64) as u32;
        let rw = (redaction.width * w as f64) as u32;
        let rh = (redaction.height * h as f64) as u32;

        for py in ry..((ry + rh).min(h)) {
            for px in rx..((rx + rw).min(w)) {
                img_buf.put_pixel(px, py, image::Rgba([0, 0, 0, 255]));
            }
        }
    }

    let mut buf = Vec::new();
    image::DynamicImage::ImageRgba8(img_buf.clone())
        .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .map_err(|e| ScannerError::SystemError(format!("Encode: {}", e)))?;

    let width = w;
    let height = h;
    let image_base64 = BASE64.encode(&buf);

    {
        let mut docs = state.documents.lock().unwrap();
        docs.insert(doc_id.clone(), DocumentData {
            png_data: buf,
            original_png_data: None,
            width,
            height,
            dpi,
        });
    }

    state.ocr_cache.lock().unwrap().remove(&doc_id);

    let now = Local::now();
    Ok(ScanResultDto {
        id: doc_id,
        name: format!("Redacted_{}.png", now.format("%H%M%S")),
        date: now.format("%d/%m/%Y %H:%M").to_string(),
        width,
        height,
        image_base64,
    })
}

// ─── v2.0.0: Table Extraction ────────────────────────────────────

#[tauri::command]
async fn detect_tables(
    doc_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<tables::DetectedTable>, ScannerError> {
    let (words, img_width, img_height) = {
        let cache = state.ocr_cache.lock().unwrap();
        let ocr = cache.get(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("Run OCR first".into()))?;
        let docs = state.documents.lock().unwrap();
        let doc = docs.get(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("Document non trouvé".into()))?;
        (ocr.words.clone(), doc.width, doc.height)
    };

    Ok(tables::detect_tables(&words, img_width, img_height))
}

#[tauri::command]
async fn export_table_csv(
    table: tables::DetectedTable,
) -> Result<String, ScannerError> {
    Ok(tables::table_to_csv(&table))
}

// ─── Phase 5: Barcode Detection ──────────────────────────────────

#[derive(Serialize)]
struct BarcodeResult {
    text: String,
    format: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[tauri::command]
async fn detect_barcodes(
    doc_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<BarcodeResult>, ScannerError> {
    let (png_data, img_width, img_height) = {
        let docs = state.documents.lock().unwrap();
        let doc = docs.get(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("Document non trouvé".into()))?;
        (doc.png_data.clone(), doc.width, doc.height)
    };

    let img = image::load_from_memory(&png_data)
        .map_err(|e| ScannerError::SystemError(format!("Décodage: {}", e)))?;
    let luma = img.to_luma8();

    let mut results = Vec::new();

    // Use rxing for barcode detection
    use rxing::Reader;
    let mut multi_reader = rxing::MultiFormatReader::default();
    let hints = rxing::DecodingHintDictionary::new();

    // Create a binary bitmap from the image
    let source = rxing::BufferedImageLuminanceSource::new(
        image::DynamicImage::ImageLuma8(luma.clone()),
    );
    let binarizer = rxing::common::HybridBinarizer::new(source);
    let mut bitmap = rxing::BinaryBitmap::new(binarizer);

    match multi_reader.decode_with_hints(&mut bitmap, &hints) {
        Ok(result) => {
            let points = result.getPoints();
            let (x, y, w, h) = if points.len() >= 2 {
                let min_x = points.iter().map(|p| p.x).fold(f32::MAX, f32::min);
                let min_y = points.iter().map(|p| p.y).fold(f32::MAX, f32::min);
                let max_x = points.iter().map(|p| p.x).fold(f32::MIN, f32::max);
                let max_y = points.iter().map(|p| p.y).fold(f32::MIN, f32::max);
                (min_x as f64, min_y as f64, (max_x - min_x) as f64, (max_y - min_y) as f64)
            } else {
                (0.0, 0.0, img_width as f64, img_height as f64)
            };

            results.push(BarcodeResult {
                text: result.getText().to_string(),
                format: format!("{:?}", result.getBarcodeFormat()),
                x: x / img_width as f64,
                y: y / img_height as f64,
                width: w / img_width as f64,
                height: h / img_height as f64,
            });
        }
        Err(_) => {} // No barcode found
    }

    Ok(results)
}

// ─── Phase 5: Statistics ─────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Default)]
struct AppStats {
    total_scans: u64,
    total_exports: u64,
    total_ocr_runs: u64,
    total_pages_scanned: u64,
    formats_used: HashMap<String, u64>,
    scans_by_month: HashMap<String, u64>,
}

fn stats_path() -> std::path::PathBuf {
    storage::config_dir_pub().join("stats.json")
}

fn load_stats() -> AppStats {
    std::fs::read_to_string(stats_path())
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

fn save_stats(stats: &AppStats) -> Result<(), ScannerError> {
    let json = serde_json::to_string_pretty(stats)
        .map_err(|e| ScannerError::SystemError(format!("Serialize stats: {}", e)))?;
    std::fs::write(stats_path(), json)
        .map_err(|e| ScannerError::SystemError(format!("Write stats: {}", e)))?;
    Ok(())
}

#[tauri::command]
async fn get_statistics() -> Result<AppStats, ScannerError> {
    Ok(load_stats())
}

#[tauri::command]
async fn increment_stat(
    stat_name: String,
    value: Option<u64>,
) -> Result<(), ScannerError> {
    let mut stats = load_stats();
    let increment = value.unwrap_or(1);
    match stat_name.as_str() {
        "scans" => stats.total_scans += increment,
        "exports" => stats.total_exports += increment,
        "ocr" => stats.total_ocr_runs += increment,
        "pages" => stats.total_pages_scanned += increment,
        _ => {}
    }
    let month = chrono::Local::now().format("%Y-%m").to_string();
    *stats.scans_by_month.entry(month).or_insert(0) += increment;
    save_stats(&stats)
}

// ─── Phase 5: TIFF Multi-page Export ─────────────────────────────

#[tauri::command]
async fn save_multipage_as_tiff(
    doc_ids: Vec<String>,
    output_path: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, ScannerError> {
    use tiff::encoder::TiffEncoder;
    use tiff::encoder::colortype;

    let file = std::fs::File::create(&output_path)
        .map_err(|e| ScannerError::SystemError(format!("Create TIFF: {}", e)))?;
    let mut encoder = TiffEncoder::new(file)
        .map_err(|e| ScannerError::SystemError(format!("TIFF encoder: {}", e)))?;

    let docs = state.documents.lock().unwrap();

    for doc_id in &doc_ids {
        let doc = docs.get(doc_id)
            .ok_or_else(|| ScannerError::SystemError(format!("Document {} non trouvé", doc_id)))?;

        let img = image::load_from_memory(&doc.png_data)
            .map_err(|e| ScannerError::SystemError(format!("Décodage: {}", e)))?;
        let rgba = img.to_rgba8();

        encoder
            .write_image::<colortype::RGBA8>(rgba.width(), rgba.height(), rgba.as_raw())
            .map_err(|e| ScannerError::SystemError(format!("Write TIFF page: {}", e)))?;
    }

    Ok(output_path)
}

// ─── Phase 5: Smart Compression ──────────────────────────────────

#[derive(Serialize)]
struct CompressionResult {
    output_path: String,
    original_size: u64,
    compressed_size: u64,
    reduction_percent: f64,
}

#[tauri::command]
async fn smart_compress(
    doc_id: String,
    output_path: String,
    state: tauri::State<'_, AppState>,
) -> Result<CompressionResult, ScannerError> {
    let png_data = {
        let docs = state.documents.lock().unwrap();
        let doc = docs.get(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("Document non trouvé".into()))?;
        doc.png_data.clone()
    };

    let original_size = png_data.len() as u64;

    // Analyze content to decide compression strategy
    let img = image::load_from_memory(&png_data)
        .map_err(|e| ScannerError::SystemError(format!("Décodage: {}", e)))?;

    // Heuristic: if image is mostly text (high contrast, few colors), use PNG
    // Otherwise, use JPEG with adaptive quality
    let rgba = img.to_rgba8();
    let mut unique_colors = std::collections::HashSet::new();
    let sample_step = (rgba.width() * rgba.height() / 10000).max(1);
    for (i, pixel) in rgba.pixels().enumerate() {
        if i as u32 % sample_step == 0 {
            unique_colors.insert((pixel[0] / 32, pixel[1] / 32, pixel[2] / 32));
        }
    }

    let is_text_heavy = unique_colors.len() < 50;

    let ext = std::path::Path::new(&output_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();

    match ext.as_str() {
        "jpg" | "jpeg" => {
            let quality = if is_text_heavy { 95 } else { 75 };
            img.save_with_format(&output_path, image::ImageFormat::Jpeg)
                .map_err(|e| ScannerError::SystemError(format!("Save JPEG: {}", e)))?;
            // Re-save with specific quality
            let mut buf = Vec::new();
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality);
            img.write_with_encoder(encoder)
                .map_err(|e| ScannerError::SystemError(format!("Encode JPEG: {}", e)))?;
            std::fs::write(&output_path, &buf)
                .map_err(|e| ScannerError::SystemError(format!("Write: {}", e)))?;
        }
        _ => {
            // Save as PNG (already compressed)
            std::fs::write(&output_path, &png_data)
                .map_err(|e| ScannerError::SystemError(format!("Write: {}", e)))?;
        }
    }

    let compressed_size = std::fs::metadata(&output_path)
        .map(|m| m.len())
        .unwrap_or(0);

    let reduction = if original_size > 0 {
        ((original_size as f64 - compressed_size as f64) / original_size as f64 * 100.0).max(0.0)
    } else {
        0.0
    };

    Ok(CompressionResult {
        output_path,
        original_size,
        compressed_size,
        reduction_percent: (reduction * 100.0).round() / 100.0,
    })
}

// ─── Phase 5: Notion/Obsidian Export ─────────────────────────────

#[tauri::command]
async fn export_to_obsidian(
    doc_id: String,
    vault_path: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, ScannerError> {
    let (png_data, ocr_text) = {
        let docs = state.documents.lock().unwrap();
        let doc = docs.get(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("Document non trouvé".into()))?;
        let cache = state.ocr_cache.lock().unwrap();
        let text = cache.get(&doc_id).map(|r| r.text.clone()).unwrap_or_default();
        (doc.png_data.clone(), text)
    };

    let history = storage::load_history();
    let doc_name = history.iter().find(|h| h.id == doc_id)
        .map(|h| h.name.clone())
        .unwrap_or_else(|| format!("Document_{}", doc_id));

    let safe_name = doc_name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");

    // Create attachment
    let attachments_dir = std::path::Path::new(&vault_path).join("attachments");
    let _ = std::fs::create_dir_all(&attachments_dir);
    let img_filename = format!("{}.png", safe_name);
    let img_path = attachments_dir.join(&img_filename);
    std::fs::write(&img_path, &png_data)
        .map_err(|e| ScannerError::SystemError(format!("Write attachment: {}", e)))?;

    // Create markdown note
    let now = chrono::Local::now();
    let md_content = format!(
        "# {}\n\nDate: {}\n\n![[attachments/{}]]\n\n## OCR Text\n\n{}\n",
        safe_name,
        now.format("%Y-%m-%d %H:%M"),
        img_filename,
        ocr_text,
    );

    let md_path = std::path::Path::new(&vault_path).join(format!("{}.md", safe_name));
    std::fs::write(&md_path, md_content)
        .map_err(|e| ScannerError::SystemError(format!("Write note: {}", e)))?;

    Ok(md_path.to_string_lossy().to_string())
}

// ─── Phase 5: Custom Shortcuts ───────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Default)]
struct ShortcutsConfig {
    shortcuts: HashMap<String, String>, // action -> key combo
}

fn shortcuts_path() -> std::path::PathBuf {
    storage::config_dir_pub().join("shortcuts.json")
}

#[tauri::command]
async fn load_shortcuts() -> Result<HashMap<String, String>, ScannerError> {
    let path = shortcuts_path();
    let config: ShortcutsConfig = std::fs::read_to_string(&path)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    Ok(config.shortcuts)
}

#[tauri::command]
async fn save_shortcuts(
    shortcuts: HashMap<String, String>,
) -> Result<(), ScannerError> {
    let config = ShortcutsConfig { shortcuts };
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| ScannerError::SystemError(format!("Serialize: {}", e)))?;
    std::fs::write(shortcuts_path(), json)
        .map_err(|e| ScannerError::SystemError(format!("Write: {}", e)))?;
    Ok(())
}

// ─── v0.5.0: Email Sharing ────────────────────────────────────────

#[tauri::command]
async fn send_document_by_email(
    doc_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, ScannerError> {
    let png_data = {
        let docs = state.documents.lock().unwrap();
        let doc = docs
            .get(&doc_id)
            .ok_or_else(|| ScannerError::SystemError("Document non trouvé en mémoire".into()))?;
        doc.png_data.clone()
    };

    // Save to temp file
    let tmp_path = std::env::temp_dir().join(format!("photon_email_{}.png", Uuid::new_v4()));
    std::fs::write(&tmp_path, &png_data)
        .map_err(|e| ScannerError::SystemError(format!("Fichier temporaire: {}", e)))?;

    let path_str = tmp_path.to_string_lossy().to_string();

    // Open default mail client via mailto: URL
    let mailto = "mailto:?subject=Document%20Photon&body=Please%20find%20the%20attached%20document.";

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        Command::new("open")
            .arg(mailto)
            .spawn()
            .map_err(|e| ScannerError::SystemError(format!("Ouverture email: {}", e)))?;
    }

    #[cfg(windows)]
    {
        use std::process::Command;
        Command::new("cmd")
            .args(["/c", "start", "", mailto])
            .spawn()
            .map_err(|e| ScannerError::SystemError(format!("Ouverture email: {}", e)))?;
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        Command::new("xdg-open")
            .arg(mailto)
            .spawn()
            .map_err(|e| ScannerError::SystemError(format!("Ouverture email: {}", e)))?;
    }

    Ok(path_str)
}

// ─── App Setup ────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    storage::init_portable_mode();
    let settings = storage::load_settings();
    let _ = storage::ensure_output_dir(&settings.output_dir);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .max_file_size(5_000_000) // 5 MB rotation
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
                .target(tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout))
                .build(),
        )
        .manage(AppState {
            documents: Mutex::new(HashMap::new()),
            multi_page_docs: Mutex::new(HashMap::new()),
            settings: Mutex::new(settings),
            ocr_cache: Mutex::new(HashMap::new()),
            vault_manager: Mutex::new(vault::VaultManager::new()),
            version_history: Mutex::new(HashMap::new()),
        })
        .invoke_handler(tauri::generate_handler![
            // v0.1.0
            list_scanners,
            scan_document,
            import_file,
            save_document_as_pdf,
            save_document_as_image,
            auto_crop_document,
            print_document,
            print_multipage_document,
            load_settings,
            save_app_settings,
            get_documents_dir,
            get_history,
            delete_history_entry,
            get_document_preview,
            // v0.2.0
            run_ocr,
            search_documents,
            // v0.3.0: Rotation & Flip
            rotate_document,
            flip_document,
            // v0.3.0: Adjustments
            preview_adjustments,
            commit_adjustments,
            revert_adjustments,
            // v0.3.0: Processing
            denoise_document,
            deskew_document,
            whiten_document_background,
            // v0.3.0: Multi-page
            create_multipage_document,
            add_page_to_document,
            remove_page_from_document,
            reorder_document_pages,
            list_multipage_documents,
            save_multipage_as_pdf,
            combine_documents_as_pdf,
            // v0.4.0: Profiles
            list_scan_profiles,
            save_scan_profile,
            delete_scan_profile,
            // v0.4.0: Batch scanning
            batch_scan,
            // v0.4.0: Naming template
            preview_naming_template,
            // v0.4.0: Document actions
            rename_document,
            duplicate_document,
            // v0.6.0: Classification & Extraction
            analyze_document,
            // v0.6.0: Tags
            get_document_tags,
            set_document_tags,
            add_document_tag,
            remove_document_tag,
            get_tag_definitions,
            save_tag_definitions_cmd,
            get_all_tags_map,
            // v0.5.0: Email
            send_document_by_email,
            // v1.1.0: Vault
            vault_is_setup,
            vault_is_unlocked,
            vault_set_password,
            vault_unlock,
            vault_lock,
            vault_add_document,
            vault_list_documents,
            vault_open_document,
            vault_remove_document,
            // v1.2.0: Version History
            get_document_versions,
            undo_last_change,
            rollback_to_version,
            // v1.2.0: Language Detection
            detect_document_language,
            // v0.6.0: Automation rules
            list_automation_rules,
            save_automation_rule,
            delete_automation_rule,
            apply_rule_actions,
            // v1.2.0: Templates
            list_templates,
            save_template,
            delete_template,
            apply_template,
            // v1.2.0: Document Comparison
            compare_documents,
            // v1.2.0: PDF Forms
            detect_pdf_form_fields,
            fill_pdf_form,
            // v2.0.0: AI via Groq
            ai_ocr_document,
            summarize_document,
            translate_document,
            // v2.0.0: Semantic Search
            semantic_search,
            // v2.0.0: Auto Redaction
            detect_sensitive_info,
            apply_redactions,
            // v2.0.0: Table Extraction
            detect_tables,
            export_table_csv,
            // Extras: Barcode
            detect_barcodes,
            // Extras: Statistics
            get_statistics,
            increment_stat,
            // Extras: TIFF
            save_multipage_as_tiff,
            // Extras: Compression
            smart_compress,
            // Extras: Obsidian
            export_to_obsidian,
            // Extras: Shortcuts
            load_shortcuts,
            save_shortcuts,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
