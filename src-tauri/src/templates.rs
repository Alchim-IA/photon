use serde::{Deserialize, Serialize};
use crate::scanner::ScannerError;
use crate::storage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionZone {
    pub label: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentTemplate {
    pub id: String,
    pub name: String,
    pub zones: Vec<ExtractionZone>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateResult {
    pub template_name: String,
    pub fields: Vec<ExtractedField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedField {
    pub label: String,
    pub text: String,
    pub zone: ExtractionZone,
}

fn templates_path() -> std::path::PathBuf {
    storage::config_dir_pub().join("templates.json")
}

pub fn load_templates() -> Vec<DocumentTemplate> {
    let path = templates_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

pub fn save_templates(templates: &[DocumentTemplate]) -> Result<(), ScannerError> {
    let json = serde_json::to_string_pretty(templates)
        .map_err(|e| ScannerError::SystemError(format!("Serialize templates: {}", e)))?;
    std::fs::write(templates_path(), json)
        .map_err(|e| ScannerError::SystemError(format!("Write templates: {}", e)))?;
    Ok(())
}

/// Apply a template to a document image: crop each zone and run OCR on it.
pub fn apply_template(
    png_data: &[u8],
    template: &DocumentTemplate,
    lang: &str,
) -> Result<TemplateResult, ScannerError> {
    let img = image::load_from_memory(png_data)
        .map_err(|e| ScannerError::SystemError(format!("Load image: {}", e)))?;

    let (img_w, img_h) = (img.width() as f64, img.height() as f64);
    let mut fields = Vec::new();

    for zone in &template.zones {
        let x = (zone.x * img_w) as u32;
        let y = (zone.y * img_h) as u32;
        let w = (zone.width * img_w) as u32;
        let h = (zone.height * img_h) as u32;

        // Clamp to image bounds
        let x = x.min(img.width().saturating_sub(1));
        let y = y.min(img.height().saturating_sub(1));
        let w = w.min(img.width() - x);
        let h = h.min(img.height() - y);

        if w == 0 || h == 0 {
            fields.push(ExtractedField {
                label: zone.label.clone(),
                text: String::new(),
                zone: zone.clone(),
            });
            continue;
        }

        let cropped = img.crop_imm(x, y, w, h);
        let mut buf = Vec::new();
        cropped
            .write_to(
                &mut std::io::Cursor::new(&mut buf),
                image::ImageFormat::Png,
            )
            .map_err(|e| ScannerError::SystemError(format!("Encode crop: {}", e)))?;

        let text = crate::ocr::extract_text(&buf, lang).unwrap_or_default();

        fields.push(ExtractedField {
            label: zone.label.clone(),
            text,
            zone: zone.clone(),
        });
    }

    Ok(TemplateResult {
        template_name: template.name.clone(),
        fields,
    })
}
