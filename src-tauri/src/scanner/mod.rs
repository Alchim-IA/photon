pub mod wia;
pub mod ica;
pub mod sane;
pub mod escl;
pub mod twain;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Serialize, Deserialize)]
pub enum ScannerError {
    #[error("Scanner occupé")]
    Busy,
    #[error("Périphérique hors ligne")]
    Offline,
    #[error("Bourrage papier")]
    PaperJam,
    #[error("Aucun driver trouvé")]
    NoDriver,
    #[error("Permissions refusées")]
    PermissionDenied,
    #[error("Aucun scanner trouvé")]
    NoDeviceFound,
    #[error("Annulé par l'utilisateur")]
    Cancelled,
    #[error("Format non supporté: {0}")]
    UnsupportedFormat(String),
    #[error("Erreur système: {0}")]
    SystemError(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScannerCapabilities {
    pub resolutions: Vec<u32>,
    pub color_modes: Vec<String>,
    pub supports_duplex: bool,
    pub supports_adf: bool,
    pub max_width_mm: f64,
    pub max_height_mm: f64,
}

impl Default for ScannerCapabilities {
    fn default() -> Self {
        Self {
            resolutions: vec![150, 300, 600],
            color_modes: vec!["Couleur".into(), "Niveaux de gris".into(), "Noir et blanc".into()],
            supports_duplex: false,
            supports_adf: false,
            max_width_mm: 215.9,
            max_height_mm: 297.0,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScannerDevice {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub capabilities: ScannerCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanOptions {
    pub device_id: String,
    pub dpi: u32,
    pub color_mode: String,
    pub duplex: bool,
    pub paper_format: String,
}

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub image_data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub trait ScannerBackend: Send + Sync {
    fn list_devices(&self) -> Result<Vec<ScannerDevice>, ScannerError>;
    fn scan(&self, options: ScanOptions) -> Result<ScanResult, ScannerError>;
}

/// Returns the platform-appropriate scanner backend.
/// On Windows, returns a combined WIA + TWAIN backend.
pub fn get_backend() -> Box<dyn ScannerBackend + Send + Sync> {
    #[cfg(windows)]
    return Box::new(CombinedWindowsBackend::new());
    #[cfg(target_os = "macos")]
    return Box::new(escl::EsclBackend::new());
    #[cfg(target_os = "linux")]
    return Box::new(sane::SaneBackend::new());
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    panic!("Plateforme non supportée");
}

// ─── Combined Windows Backend (WIA + TWAIN) ─────────────────────

#[cfg(windows)]
struct CombinedWindowsBackend {
    wia: wia::WiaBackend,
    twain: twain::TwainBackend,
}

#[cfg(windows)]
impl CombinedWindowsBackend {
    fn new() -> Self {
        Self {
            wia: wia::WiaBackend::new(),
            twain: twain::TwainBackend::new(),
        }
    }
}

#[cfg(windows)]
impl ScannerBackend for CombinedWindowsBackend {
    fn list_devices(&self) -> Result<Vec<ScannerDevice>, ScannerError> {
        let mut devices = Vec::new();

        // WIA devices first
        match self.wia.list_devices() {
            Ok(wia_devices) => devices.extend(wia_devices),
            Err(e) => log::warn!("WIA énumération échouée: {}", e),
        }

        // Then TWAIN devices (skip duplicates by name)
        match self.twain.list_devices() {
            Ok(twain_devices) => {
                for td in twain_devices {
                    // Check if a WIA device with the same base name already exists
                    let base_name = td.name.trim_end_matches(" (TWAIN)");
                    let is_duplicate = devices.iter().any(|d| {
                        d.name.contains(base_name) || base_name.contains(&d.name)
                    });
                    if !is_duplicate {
                        devices.push(td);
                    } else {
                        log::debug!("TWAIN: source '{}' ignorée (doublon WIA)", td.name);
                    }
                }
            }
            Err(e) => log::debug!("TWAIN énumération échouée: {}", e),
        }

        Ok(devices)
    }

    fn scan(&self, options: ScanOptions) -> Result<ScanResult, ScannerError> {
        if options.device_id.starts_with("twain://") {
            self.twain.scan(options)
        } else {
            self.wia.scan(options)
        }
    }
}

/// Paper format dimensions in mm (width, height).
pub fn paper_dimensions(format: &str) -> (f64, f64) {
    match format {
        "A4" => (210.0, 297.0),
        "A3" => (297.0, 420.0),
        "Letter" => (215.9, 279.4),
        "Legal" => (215.9, 355.6),
        _ => (210.0, 297.0),
    }
}

/// Convert mm to pixels at given DPI.
pub fn mm_to_pixels(mm: f64, dpi: u32) -> u32 {
    (mm / 25.4 * dpi as f64).round() as u32
}

/// Map color mode string to a numeric identifier used by backends.
pub fn color_mode_id(mode: &str) -> u32 {
    match mode {
        "Couleur" | "Color" => 1,
        "Niveaux de gris" | "Grayscale" => 2,
        "Noir et blanc" | "Black and White" | "BW" => 4,
        _ => 1,
    }
}
