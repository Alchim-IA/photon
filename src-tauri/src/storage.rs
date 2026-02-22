use crate::scanner::ScannerError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

// ─── Portable Mode ──────────────────────────────────────────────

static PORTABLE_DIR: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Call once at startup to detect portable mode (portable.marker next to exe).
pub fn init_portable_mode() {
    PORTABLE_DIR.get_or_init(|| {
        let exe = std::env::current_exe().ok()?;
        let exe_dir = exe.parent()?;
        if exe_dir.join("portable.marker").exists() {
            let data = exe_dir.join("data");
            let _ = fs::create_dir_all(&data);
            Some(data)
        } else {
            None
        }
    });
}

pub fn is_portable() -> bool {
    PORTABLE_DIR.get().is_some_and(|v| v.is_some())
}

// ─── Helpers ────────────────────────────────────────────────────

/// Public accessor for the config directory.
pub fn config_dir_pub() -> PathBuf { config_dir() }

fn config_dir() -> PathBuf {
    if let Some(Some(portable)) = PORTABLE_DIR.get() {
        let _ = fs::create_dir_all(portable);
        return portable.clone();
    }
    let dir = dirs_next::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("document-scanner");
    let _ = fs::create_dir_all(&dir);
    dir
}

fn load_json<T: serde::de::DeserializeOwned + Default>(path: &std::path::Path) -> T {
    fs::read_to_string(path)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

fn save_json<T: Serialize + ?Sized>(path: &std::path::Path, data: &T, label: &str) -> Result<(), ScannerError> {
    let json = serde_json::to_string_pretty(data)
        .map_err(|e| ScannerError::SystemError(format!("Sérialisation {}: {}", label, e)))?;
    fs::write(path, json)
        .map_err(|e| ScannerError::SystemError(format!("Écriture {}: {}", label, e)))?;
    Ok(())
}

// ─── Scan Profiles ──────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScanProfile {
    pub id: String,
    pub name: String,
    pub dpi: u32,
    pub color_mode: String,
    pub paper_format: String,
    pub duplex: bool,
    pub auto_crop: bool,
    pub auto_ocr: bool,
}

fn profiles_path() -> PathBuf { config_dir().join("profiles.json") }

pub fn load_profiles() -> Vec<ScanProfile> { load_json(&profiles_path()) }

pub fn save_profiles(profiles: &[ScanProfile]) -> Result<(), ScannerError> {
    save_json(&profiles_path(), profiles, "profils")
}

// ─── App Settings ───────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub output_dir: String,
    pub default_format: String,
    pub auto_crop: bool,
    pub quality: u8,
    pub default_dpi: u32,
    pub default_color_mode: String,
    pub default_paper_format: String,
    #[serde(default)]
    pub auto_ocr: bool,
    #[serde(default = "default_ocr_lang")]
    pub default_ocr_lang: String,
    /// Naming template: {date}, {time}, {counter}, {dpi}, {mode}, {format}
    #[serde(default = "default_naming_template")]
    pub naming_template: String,
    /// Auto-export: if set, copies each scan to this folder automatically
    #[serde(default)]
    pub watch_folder: Option<String>,
    /// Global scan counter for naming template
    #[serde(default)]
    pub scan_counter: u32,
    /// UI language (fr / en)
    #[serde(default = "default_language")]
    pub language: String,
    /// Whether the user has completed the onboarding wizard
    #[serde(default)]
    pub onboarding_complete: bool,
    /// Groq API key for AI features
    #[serde(default)]
    pub groq_api_key: Option<String>,
}

fn default_language() -> String {
    "fr".to_string()
}

fn default_ocr_lang() -> String {
    "fra".to_string()
}

fn default_naming_template() -> String {
    "Scan_{date}_{time}".to_string()
}

impl Default for AppSettings {
    fn default() -> Self {
        let output_dir = dirs_next::document_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Scanner de Documents")
            .to_string_lossy()
            .to_string();

        Self {
            output_dir,
            default_format: "PDF".to_string(),
            auto_crop: true,
            quality: 85,
            default_dpi: 300,
            default_color_mode: "Couleur".to_string(),
            default_paper_format: "A4".to_string(),
            auto_ocr: false,
            default_ocr_lang: "fra".to_string(),
            naming_template: "Scan_{date}_{time}".to_string(),
            watch_folder: None,
            scan_counter: 0,
            language: "fr".to_string(),
            onboarding_complete: false,
            groq_api_key: None,
        }
    }
}

/// Expands a naming template with current values.
pub fn expand_naming_template(
    template: &str,
    dpi: u32,
    color_mode: &str,
    format: &str,
    counter: u32,
) -> String {
    let now = chrono::Local::now();
    template
        .replace("{date}", &now.format("%Y-%m-%d").to_string())
        .replace("{time}", &now.format("%H%M%S").to_string())
        .replace("{counter}", &format!("{:04}", counter))
        .replace("{dpi}", &dpi.to_string())
        .replace("{mode}", color_mode)
        .replace("{format}", format)
}

fn settings_path() -> PathBuf { config_dir().join("settings.json") }

pub fn load_settings() -> AppSettings {
    let mut settings: AppSettings = load_json(&settings_path());
    // Migration: existing users who already configured output_dir skip onboarding
    if !settings.onboarding_complete && !settings.output_dir.is_empty() {
        let default_output = AppSettings::default().output_dir;
        if settings.output_dir != default_output {
            settings.onboarding_complete = true;
            // Persist so we don't re-check every launch
            let _ = save_settings(&settings);
        }
    }
    settings
}

pub fn save_settings(settings: &AppSettings) -> Result<(), ScannerError> {
    save_json(&settings_path(), settings, "paramètres")
}

/// Document metadata stored in history.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DocumentMeta {
    pub id: String,
    pub name: String,
    pub date: String,
    pub file_path: Option<String>,
    pub format: String,
    pub size_bytes: u64,
    pub width: u32,
    pub height: u32,
    pub dpi: u32,
    #[serde(default)]
    pub ocr_text: Option<String>,
    #[serde(default)]
    pub ocr_lang: Option<String>,
}

fn history_path() -> PathBuf { config_dir().join("history.json") }

pub fn load_history() -> Vec<DocumentMeta> { load_json(&history_path()) }

pub fn save_history(history: &[DocumentMeta]) -> Result<(), ScannerError> {
    save_json(&history_path(), history, "historique")
}

/// Adds a document to the history.
pub fn add_to_history(meta: DocumentMeta) -> Result<(), ScannerError> {
    let mut history = load_history();
    history.insert(0, meta);
    // Keep last 100 entries
    history.truncate(100);
    save_history(&history)
}

/// Ensures the output directory exists.
pub fn ensure_output_dir(dir: &str) -> Result<(), ScannerError> {
    fs::create_dir_all(dir)
        .map_err(|e| ScannerError::SystemError(format!("Création dossier: {}", e)))
}

// ─── Tags System ────────────────────────────────────────────────

use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TagDefinition {
    pub name: String,
    pub color: String,
}

fn tags_path() -> PathBuf { config_dir().join("tags.json") }
fn tag_defs_path() -> PathBuf { config_dir().join("tag_definitions.json") }

pub fn load_tags() -> HashMap<String, Vec<String>> { load_json(&tags_path()) }

pub fn save_tags(tags: &HashMap<String, Vec<String>>) -> Result<(), ScannerError> {
    save_json(&tags_path(), tags, "tags")
}

pub fn load_tag_definitions() -> Vec<TagDefinition> {
    let path = tag_defs_path();
    match fs::read_to_string(&path) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
        Err(_) => default_tag_definitions(),
    }
}

pub fn save_tag_definitions(defs: &[TagDefinition]) -> Result<(), ScannerError> {
    save_json(&tag_defs_path(), defs, "tag defs")
}

fn default_tag_definitions() -> Vec<TagDefinition> {
    vec![
        TagDefinition { name: "Facture".into(), color: "#3b82f6".into() },
        TagDefinition { name: "Contrat".into(), color: "#8b5cf6".into() },
        TagDefinition { name: "Identité".into(), color: "#ef4444".into() },
        TagDefinition { name: "Santé".into(), color: "#10b981".into() },
        TagDefinition { name: "Emploi".into(), color: "#f59e0b".into() },
        TagDefinition { name: "Banque".into(), color: "#06b6d4".into() },
        TagDefinition { name: "Financier".into(), color: "#ec4899".into() },
        TagDefinition { name: "Important".into(), color: "#ef4444".into() },
        TagDefinition { name: "À traiter".into(), color: "#f97316".into() },
        TagDefinition { name: "Archivé".into(), color: "#6b7280".into() },
    ]
}

// ─── Automation Rules ───────────────────────────────────────────

use crate::intelligence::AutomationRule;

fn rules_path() -> PathBuf { config_dir().join("rules.json") }

pub fn load_rules() -> Vec<AutomationRule> { load_json(&rules_path()) }

pub fn save_rules(rules: &[AutomationRule]) -> Result<(), ScannerError> {
    save_json(&rules_path(), rules, "règles")
}
