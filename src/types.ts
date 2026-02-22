// ─── Error extraction helper ─────────────────────────────────────
// Tauri serializes Rust enums as JSON: "Busy" or {"SystemError":"msg"}
export function extractError(err: unknown): string {
  if (typeof err === "string") return err;
  if (err && typeof err === "object") {
    // Tauri v2 serialized enum: {"SystemError": "message"}
    const vals = Object.values(err as Record<string, unknown>);
    if (vals.length === 1 && typeof vals[0] === "string") return vals[0];
    if ("message" in err && typeof (err as { message: unknown }).message === "string") return (err as { message: string }).message;
    return JSON.stringify(err);
  }
  return extractError(err);
}

// ─── Theme ───────────────────────────────────────────────────────
export type ThemeMode = "light" | "dark" | "auto";

export function getSystemTheme(): "light" | "dark" {
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export function applyTheme(mode: ThemeMode) {
  const resolved = mode === "auto" ? getSystemTheme() : mode;
  document.documentElement.setAttribute("data-theme", resolved);
}

export function loadThemePreference(): ThemeMode {
  return (localStorage.getItem("theme-mode") as ThemeMode) || "dark";
}

export function saveThemePreference(mode: ThemeMode) {
  localStorage.setItem("theme-mode", mode);
}

// ─── Types ────────────────────────────────────────────────────────
export interface ScannerCapabilities {
  resolutions: number[];
  color_modes: string[];
  supports_duplex: boolean;
  supports_adf: boolean;
}

export interface ScannerDevice {
  id: string;
  name: string;
  vendor: string;
  capabilities: ScannerCapabilities;
}

export interface ScanResultDto {
  id: string;
  name: string;
  date: string;
  width: number;
  height: number;
  image_base64: string;
}

export interface ScannedDocument {
  id: string;
  name: string;
  date: string;
  width: number;
  height: number;
  dataUrl: string;
}

export interface ScanConfig {
  dpi: number;
  colorMode: string;
  paperFormat: string;
  duplex: boolean;
  adf: boolean;
}

export interface AppSettings {
  output_dir: string;
  default_format: string;
  auto_crop: boolean;
  quality: number;
  default_dpi: number;
  default_color_mode: string;
  default_paper_format: string;
  auto_ocr: boolean;
  default_ocr_lang: string;
  naming_template: string;
  watch_folder: string | null;
  scan_counter: number;
  language?: string;
  onboarding_complete?: boolean;
  groq_api_key?: string | null;
}

export interface ScanProfile {
  id: string;
  name: string;
  dpi: number;
  color_mode: string;
  paper_format: string;
  duplex: boolean;
  auto_crop: boolean;
  auto_ocr: boolean;
}

export interface HistoryEntryDto {
  id: string;
  name: string;
  date: string;
  format: string;
  file_path: string | null;
  has_preview: boolean;
  has_ocr: boolean;
  ocr_text: string | null;
}

export interface ImageAdjustments {
  brightness: number;
  contrast: number;
  saturation: number;
  sharpness: number;
}

export interface AdjustmentPreviewResult {
  image_base64: string;
  width: number;
  height: number;
}

export interface MultiPageDocDto {
  id: string;
  name: string;
  page_ids: string[];
  page_count: number;
  created_at: string;
}

// ─── v0.6.0 Types ────────────────────────────────────────────────

export interface ClassificationResult {
  doc_type: string;
  confidence: number;
  scores: [string, number][];
}

export interface ExtractedData {
  fields: Record<string, string[]>;
}

export interface SmartSuggestion {
  suggested_name: string;
  suggested_folder: string;
  suggested_tags: string[];
  classification: ClassificationResult;
  extracted_data: ExtractedData;
}

export interface AnalysisResultDto {
  classification: ClassificationResult;
  extracted_data: ExtractedData;
  suggestion: SmartSuggestion;
  auto_tags: string[];
  rule_results: RuleExecutionResult[];
}

export interface TagDefinition {
  name: string;
  color: string;
}

export interface AutomationRule {
  id: string;
  name: string;
  enabled: boolean;
  condition_logic: "And" | "Or";
  conditions: RuleCondition[];
  actions: RuleAction[];
}

export interface RuleCondition {
  field: string;
  operator: string;
  value: string;
}

export interface RuleAction {
  action_type: string;
  value: string;
}

export interface RuleExecutionResult {
  rule_name: string;
  actions: RuleAction[];
}

// ─── v1.0: PDF Export & Annotations ──────────────────────────────
export type PdfALevel = "A1b" | "A2b";

export type WatermarkPosition = "Center" | "TopLeft" | "TopRight" | "BottomLeft" | "BottomRight" | "Diagonal";

export interface WatermarkConfig {
  text: string;
  opacity: number;
  rotation: number;
  font_size: number;
  color: string;
  position: WatermarkPosition;
}

export interface SignatureConfig {
  image_base64: string;
  page_index: number;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface PdfExportOptions {
  pdfa: PdfALevel | null;
  user_password: string | null;
  owner_password: string | null;
  watermark: WatermarkConfig | null;
  signature: SignatureConfig | null;
}

export interface PdfSaveResult {
  path: string;
  sha256: string | null;
}

export type AnnotationTypeName = "Highlight" | "Ellipse" | "TextNote";

export interface AnnotationData {
  annotation_type: AnnotationTypeName;
  x: number;
  y: number;
  width: number;
  height: number;
  color: string;
  text: string | null;
}

export interface PageAnnotationsData {
  page_index: number;
  annotations: AnnotationData[];
}

// ─── Inferred types (from App.tsx usage) ─────────────────────────

export interface VaultDocument {
  id: string;
  name: string;
  date: string;
  format: string;
  size_bytes: number;
}

export interface SemanticResult {
  doc_id: string;
  doc_name: string;
  score: number;
  snippet: string;
}

export interface SensitiveInfo {
  text: string;
  category: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface DetectedTable {
  rows: string[][];
  row_count: number;
  col_count: number;
}

export interface AppStats {
  total_scans: number;
  total_exports: number;
  total_ocr_runs: number;
  total_pages_scanned: number;
  formats_used: Record<string, number>;
}

// ─── Helpers ─────────────────────────────────────────────────────

export function dtoToDoc(result: ScanResultDto): ScannedDocument {
  return {
    id: result.id,
    name: result.name,
    date: result.date,
    width: result.width,
    height: result.height,
    dataUrl: `data:image/png;base64,${result.image_base64}`,
  };
}
