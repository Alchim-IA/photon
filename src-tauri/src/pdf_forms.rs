use serde::{Deserialize, Serialize};
use crate::scanner::ScannerError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormField {
    pub name: String,
    pub field_type: String, // "text", "checkbox", "radio", "dropdown"
    pub value: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub page: usize,
    pub options: Option<Vec<String>>, // For dropdown/radio
}

/// Detect form fields in a PDF using lopdf.
/// Looks for AcroForm entries in the PDF catalog.
pub fn detect_form_fields(path: &str) -> Result<Vec<FormField>, ScannerError> {
    let doc = lopdf::Document::load(path)
        .map_err(|e| ScannerError::SystemError(format!("Load PDF: {}", e)))?;

    let mut fields = Vec::new();

    // Get AcroForm from catalog
    let catalog = doc.catalog()
        .map_err(|e| ScannerError::SystemError(format!("Get catalog: {}", e)))?;

    let acroform = match catalog.get(b"AcroForm") {
        Ok(obj) => resolve_object(&doc, obj),
        Err(_) => return Ok(fields), // No form
    };

    let acroform_dict = match acroform {
        Some(lopdf::Object::Dictionary(ref d)) => d,
        _ => return Ok(fields),
    };

    // Get Fields array
    let field_refs = match acroform_dict.get(b"Fields") {
        Ok(lopdf::Object::Array(ref arr)) => arr.clone(),
        _ => return Ok(fields),
    };

    for field_ref in &field_refs {
        if let lopdf::Object::Reference(id) = field_ref {
            if let Ok(obj) = doc.get_object(*id) {
                if let lopdf::Object::Dictionary(ref dict) = obj {
                    if let Some(field) = parse_form_field(&doc, dict, 0) {
                        fields.push(field);
                    }
                }
            }
        }
    }

    Ok(fields)
}

fn resolve_object<'a>(doc: &'a lopdf::Document, obj: &'a lopdf::Object) -> Option<&'a lopdf::Object> {
    match obj {
        lopdf::Object::Reference(r) => doc.get_object(*r).ok(),
        other => Some(other),
    }
}

fn parse_form_field(doc: &lopdf::Document, dict: &lopdf::Dictionary, page: usize) -> Option<FormField> {
    let name = dict
        .get(b"T")
        .ok()
        .and_then(|o| match o {
            lopdf::Object::String(s, _) => Some(String::from_utf8_lossy(s).to_string()),
            _ => None,
        })
        .unwrap_or_else(|| "unnamed".to_string());

    let ft = dict
        .get(b"FT")
        .ok()
        .and_then(|o| match o {
            lopdf::Object::Name(n) => Some(String::from_utf8_lossy(n).to_string()),
            _ => None,
        })
        .unwrap_or_default();

    let field_type = match ft.as_str() {
        "Tx" => "text",
        "Btn" => "checkbox",
        "Ch" => "dropdown",
        _ => "text",
    };

    let value = dict
        .get(b"V")
        .ok()
        .and_then(|o| match o {
            lopdf::Object::String(s, _) => Some(String::from_utf8_lossy(s).to_string()),
            lopdf::Object::Name(n) => Some(String::from_utf8_lossy(n).to_string()),
            _ => None,
        })
        .unwrap_or_default();

    // Get Rect (position)
    let (x, y, width, height) = dict
        .get(b"Rect")
        .ok()
        .and_then(|o| {
            let arr = match o {
                lopdf::Object::Array(a) => a,
                lopdf::Object::Reference(r) => {
                    if let Ok(lopdf::Object::Array(a)) = doc.get_object(*r) {
                        a
                    } else {
                        return None;
                    }
                }
                _ => return None,
            };
            if arr.len() < 4 { return None; }
            let x1 = obj_to_f64(&arr[0])?;
            let y1 = obj_to_f64(&arr[1])?;
            let x2 = obj_to_f64(&arr[2])?;
            let y2 = obj_to_f64(&arr[3])?;
            Some((x1.min(x2), y1.min(y2), (x2 - x1).abs(), (y2 - y1).abs()))
        })
        .unwrap_or((0.0, 0.0, 100.0, 20.0));

    // Get options for dropdown/radio
    let options = if field_type == "dropdown" {
        dict.get(b"Opt").ok().and_then(|o| {
            if let lopdf::Object::Array(arr) = o {
                Some(
                    arr.iter()
                        .filter_map(|item| match item {
                            lopdf::Object::String(s, _) => {
                                Some(String::from_utf8_lossy(s).to_string())
                            }
                            _ => None,
                        })
                        .collect(),
                )
            } else {
                None
            }
        })
    } else {
        None
    };

    Some(FormField {
        name,
        field_type: field_type.to_string(),
        value,
        x,
        y,
        width,
        height,
        page,
        options,
    })
}

fn obj_to_f64(obj: &lopdf::Object) -> Option<f64> {
    match obj {
        lopdf::Object::Integer(i) => Some(*i as f64),
        lopdf::Object::Real(f) => Some(*f as f64),
        _ => None,
    }
}

/// Fill form fields in a PDF by setting field values.
pub fn fill_form(path: &str, field_values: &[(String, String)]) -> Result<(), ScannerError> {
    let mut doc = lopdf::Document::load(path)
        .map_err(|e| ScannerError::SystemError(format!("Load PDF: {}", e)))?;

    let catalog = doc.catalog()
        .map_err(|e| ScannerError::SystemError(format!("Get catalog: {}", e)))?
        .clone();

    let acroform = match catalog.get(b"AcroForm") {
        Ok(obj) => {
            match obj {
                lopdf::Object::Reference(r) => *r,
                _ => return Err(ScannerError::SystemError("AcroForm not a reference".into())),
            }
        }
        Err(_) => return Err(ScannerError::SystemError("No AcroForm in PDF".into())),
    };

    // Create a lookup of field name -> new value
    let values: std::collections::HashMap<&str, &str> = field_values
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    // Get field references
    let field_ids: Vec<lopdf::ObjectId> = {
        if let Ok(obj) = doc.get_object(acroform) {
            if let lopdf::Object::Dictionary(ref dict) = obj {
                if let Ok(lopdf::Object::Array(ref arr)) = dict.get(b"Fields") {
                    arr.iter()
                        .filter_map(|o| {
                            if let lopdf::Object::Reference(id) = o {
                                Some(*id)
                            } else {
                                None
                            }
                        })
                        .collect()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    };

    for field_id in field_ids {
        if let Ok(obj) = doc.get_object_mut(field_id) {
            if let lopdf::Object::Dictionary(ref mut dict) = obj {
                let field_name = dict
                    .get(b"T")
                    .ok()
                    .and_then(|o| match o {
                        lopdf::Object::String(s, _) => {
                            Some(String::from_utf8_lossy(s).to_string())
                        }
                        _ => None,
                    })
                    .unwrap_or_default();

                if let Some(new_val) = values.get(field_name.as_str()) {
                    dict.set(
                        "V",
                        lopdf::Object::String(
                            new_val.as_bytes().to_vec(),
                            lopdf::StringFormat::Literal,
                        ),
                    );
                }
            }
        }
    }

    doc.save(path)
        .map_err(|e| ScannerError::SystemError(format!("Save PDF: {}", e)))?;

    Ok(())
}
