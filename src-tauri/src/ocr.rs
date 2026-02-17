use crate::scanner::ScannerError;
use serde::{Deserialize, Serialize};

/// A single word recognized by OCR, with its bounding box.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrWord {
    pub text: String,
    /// Bounding box in pixels: (x, y, width, height)
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub confidence: f32,
}

/// Full OCR result containing the plain text and per-word bounding boxes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResult {
    pub text: String,
    pub words: Vec<OcrWord>,
    pub lang: String,
}

/// Extracts plain text from a PNG image using Tesseract.
pub fn extract_text(png_data: &[u8], lang: &str) -> Result<String, ScannerError> {
    let mut tess = tesseract::Tesseract::new(None, Some(lang))
        .map_err(|e| ScannerError::SystemError(format!("Initialisation Tesseract: {}", e)))?
        .set_image_from_mem(png_data)
        .map_err(|e| ScannerError::SystemError(format!("Chargement image OCR: {}", e)))?;

    let text = tess
        .get_text()
        .map_err(|e| ScannerError::SystemError(format!("Extraction texte OCR: {}", e)))?;

    Ok(text.trim().to_string())
}

/// Extracts text with per-word bounding boxes from a PNG image.
pub fn extract_text_with_boxes(png_data: &[u8], lang: &str) -> Result<OcrResult, ScannerError> {
    let mut tess = tesseract::Tesseract::new(None, Some(lang))
        .map_err(|e| ScannerError::SystemError(format!("Initialisation Tesseract: {}", e)))?
        .set_image_from_mem(png_data)
        .map_err(|e| ScannerError::SystemError(format!("Chargement image OCR: {}", e)))?;

    let text = tess
        .get_text()
        .map_err(|e| ScannerError::SystemError(format!("Extraction texte OCR: {}", e)))?
        .trim()
        .to_string();

    // Try to get word-level bounding boxes via TSV output
    let words = match tess.get_tsv_text(1) {
        Ok(tsv) => parse_tsv_words(&tsv),
        Err(_) => Vec::new(),
    };

    Ok(OcrResult {
        text,
        words,
        lang: lang.to_string(),
    })
}

/// Parses Tesseract TSV output into word bounding boxes.
fn parse_tsv_words(tsv: &str) -> Vec<OcrWord> {
    let mut words = Vec::new();

    for line in tsv.lines().skip(1) {
        // TSV columns: level, page_num, block_num, par_num, line_num, word_num,
        //              left, top, width, height, conf, text
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 12 {
            continue;
        }

        let conf: f32 = cols[10].parse().unwrap_or(-1.0);
        let text = cols[11].trim();

        // Skip empty words and low-confidence results
        if text.is_empty() || conf < 0.0 {
            continue;
        }

        let x: i32 = cols[6].parse().unwrap_or(0);
        let y: i32 = cols[7].parse().unwrap_or(0);
        let w: i32 = cols[8].parse().unwrap_or(0);
        let h: i32 = cols[9].parse().unwrap_or(0);

        words.push(OcrWord {
            text: text.to_string(),
            x,
            y,
            w,
            h,
            confidence: conf,
        });
    }

    words
}
