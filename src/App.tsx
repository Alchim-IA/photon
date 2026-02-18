import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { save } from "@tauri-apps/plugin-dialog";
import {
  DndContext,
  rectIntersection,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  rectSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { useTranslation, type Language } from "./contexts/LanguageContext";
import { useFocusTrap } from "./hooks/useFocusTrap";
import { OnboardingWizard } from "./components/onboarding/OnboardingWizard";
import { TourTooltip } from "./components/onboarding/TourTooltip";
import { useTour } from "./components/onboarding/useTour";
import { selectDirectory } from "./utils/selectDirectory";
import "./App.css";

// ─── Theme ───────────────────────────────────────────────────────
type ThemeMode = "light" | "dark" | "auto";

function getSystemTheme(): "light" | "dark" {
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

function applyTheme(mode: ThemeMode) {
  const resolved = mode === "auto" ? getSystemTheme() : mode;
  document.documentElement.setAttribute("data-theme", resolved);
}

function loadThemePreference(): ThemeMode {
  return (localStorage.getItem("theme-mode") as ThemeMode) || "dark";
}

function saveThemePreference(mode: ThemeMode) {
  localStorage.setItem("theme-mode", mode);
}

// ─── Types ────────────────────────────────────────────────────────
interface ScannerCapabilities {
  resolutions: number[];
  color_modes: string[];
  supports_duplex: boolean;
  supports_adf: boolean;
}

interface ScannerDevice {
  id: string;
  name: string;
  vendor: string;
  capabilities: ScannerCapabilities;
}

interface ScanResultDto {
  id: string;
  name: string;
  date: string;
  width: number;
  height: number;
  image_base64: string;
}

interface ScannedDocument {
  id: string;
  name: string;
  date: string;
  width: number;
  height: number;
  dataUrl: string;
}

interface ScanConfig {
  dpi: number;
  colorMode: string;
  paperFormat: string;
  duplex: boolean;
  adf: boolean;
}

interface AppSettings {
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
}

interface ScanProfile {
  id: string;
  name: string;
  dpi: number;
  color_mode: string;
  paper_format: string;
  duplex: boolean;
  auto_crop: boolean;
  auto_ocr: boolean;
}

interface HistoryEntryDto {
  id: string;
  name: string;
  date: string;
  format: string;
  file_path: string | null;
  has_preview: boolean;
  has_ocr: boolean;
  ocr_text: string | null;
}

interface ImageAdjustments {
  brightness: number;
  contrast: number;
  saturation: number;
  sharpness: number;
}

interface AdjustmentPreviewResult {
  image_base64: string;
  width: number;
  height: number;
}

interface MultiPageDocDto {
  id: string;
  name: string;
  page_ids: string[];
  page_count: number;
  created_at: string;
}

// ─── v0.6.0 Types ────────────────────────────────────────────────

interface ClassificationResult {
  doc_type: string;
  confidence: number;
  scores: [string, number][];
}

interface ExtractedData {
  fields: Record<string, string[]>;
}

interface SmartSuggestion {
  suggested_name: string;
  suggested_folder: string;
  suggested_tags: string[];
  classification: ClassificationResult;
  extracted_data: ExtractedData;
}

interface AnalysisResultDto {
  classification: ClassificationResult;
  extracted_data: ExtractedData;
  suggestion: SmartSuggestion;
  auto_tags: string[];
  rule_results: RuleExecutionResult[];
}

interface TagDefinition {
  name: string;
  color: string;
}

interface AutomationRule {
  id: string;
  name: string;
  enabled: boolean;
  condition_logic: "And" | "Or";
  conditions: RuleCondition[];
  actions: RuleAction[];
}

interface RuleCondition {
  field: string;
  operator: string;
  value: string;
}

interface RuleAction {
  action_type: string;
  value: string;
}

interface RuleExecutionResult {
  rule_name: string;
  actions: RuleAction[];
}

// ─── Icons ───────────────────────────────────────────────────────
const Icons = {
  scanner: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <path d="M6 18H4a2 2 0 01-2-2v-5a2 2 0 012-2h16a2 2 0 012 2v5a2 2 0 01-2 2h-2" />
      <path d="M6 9V3a1 1 0 011-1h10a1 1 0 011 1v6" />
      <rect x="6" y="14" width="12" height="8" rx="1" />
    </svg>
  ),
  refresh: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M21 2v6h-6" /><path d="M3 12a9 9 0 0115.4-6.4L21 8" /><path d="M3 22v-6h6" /><path d="M21 12a9 9 0 01-15.4 6.4L3 16" /></svg>),
  scan: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M3 7V5a2 2 0 012-2h2" /><path d="M17 3h2a2 2 0 012 2v2" /><path d="M21 17v2a2 2 0 01-2 2h-2" /><path d="M7 21H5a2 2 0 01-2-2v-2" /><line x1="7" y1="12" x2="17" y2="12" /></svg>),
  pdf: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" /><polyline points="14,2 14,8 20,8" /><path d="M9 15v-2h1.5a1.5 1.5 0 010 3H9" /></svg>),
  image: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" /><circle cx="8.5" cy="8.5" r="1.5" /><polyline points="21,15 16,10 5,21" /></svg>),
  print: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><polyline points="6,9 6,2 18,2 18,9" /><path d="M6 18H4a2 2 0 01-2-2v-5a2 2 0 012-2h16a2 2 0 012 2v5a2 2 0 01-2 2h-2" /><rect x="6" y="14" width="12" height="8" /></svg>),
  crop: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M6 2v4h12v12h4" /><path d="M18 22v-4H6V6H2" /></svg>),
  settings: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 01-2.83 2.83l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z" /></svg>),
  close: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>),
  zoomIn: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><circle cx="11" cy="11" r="8" /><line x1="21" y1="21" x2="16.65" y2="16.65" /><line x1="11" y1="8" x2="11" y2="14" /><line x1="8" y1="11" x2="14" y2="11" /></svg>),
  zoomOut: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><circle cx="11" cy="11" r="8" /><line x1="21" y1="21" x2="16.65" y2="16.65" /><line x1="8" y1="11" x2="14" y2="11" /></svg>),
  folder: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z" /></svg>),
  chevronDown: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="6,9 12,15 18,9" /></svg>),
  empty: (<svg viewBox="0 0 80 80" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round"><rect x="16" y="8" width="48" height="64" rx="4" /><line x1="28" y1="24" x2="52" y2="24" /><line x1="28" y1="32" x2="48" y2="32" /><line x1="28" y1="40" x2="52" y2="40" /><line x1="28" y1="48" x2="44" y2="48" /><path d="M40 60l6-6 6 6" /><line x1="46" y1="54" x2="46" y2="66" /></svg>),
  delete: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><polyline points="3,6 5,6 21,6" /><path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2" /></svg>),
  ocr: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" /><polyline points="14,2 14,8 20,8" /><line x1="8" y1="13" x2="16" y2="13" /><line x1="8" y1="17" x2="12" y2="17" /></svg>),
  search: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><circle cx="11" cy="11" r="8" /><line x1="21" y1="21" x2="16.65" y2="16.65" /></svg>),
  copy: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" /><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" /></svg>),
  sun: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="5" /><path d="M12 1v2m0 18v2m-9-11h2m18 0h2m-3.3-6.7l-1.4 1.4M6.7 17.3l-1.4 1.4m0-13.4l1.4 1.4m10.6 10.6l1.4 1.4" /></svg>),
  moon: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M21 12.79A9 9 0 1111.21 3 7 7 0 0021 12.79z" /></svg>),
  auto: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="10" /><path d="M12 2a10 10 0 010 20V2z" /></svg>),
  rotateRight: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M21 2v6h-6" /><path d="M21 8A9 9 0 1 0 6.7 17.3" /></svg>),
  rotateLeft: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M3 2v6h6" /><path d="M3 8a9 9 0 1 1 14.3 9.3" /></svg>),
  flipH: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M8 3H5a2 2 0 00-2 2v14a2 2 0 002 2h3" /><path d="M16 3h3a2 2 0 012 2v14a2 2 0 01-2 2h-3" /><line x1="12" y1="2" x2="12" y2="22" strokeDasharray="2 2" /></svg>),
  flipV: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M3 8V5a2 2 0 012-2h14a2 2 0 012 2v3" /><path d="M3 16v3a2 2 0 002 2h14a2 2 0 002-2v-3" /><line x1="2" y1="12" x2="22" y2="12" strokeDasharray="2 2" /></svg>),
  sliders: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><line x1="4" y1="21" x2="4" y2="14" /><line x1="4" y1="10" x2="4" y2="3" /><line x1="12" y1="21" x2="12" y2="12" /><line x1="12" y1="8" x2="12" y2="3" /><line x1="20" y1="21" x2="20" y2="16" /><line x1="20" y1="12" x2="20" y2="3" /><line x1="1" y1="14" x2="7" y2="14" /><line x1="9" y1="8" x2="15" y2="8" /><line x1="17" y1="16" x2="23" y2="16" /></svg>),
  deskew: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" transform="rotate(-5 12 12)" /><line x1="7" y1="12" x2="17" y2="12" /></svg>),
  whiten: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="10" /><circle cx="12" cy="12" r="4" fill="currentColor" opacity="0.3" /></svg>),
  noise: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M4 14h4l2-6 4 12 2-6h4" /></svg>),
  pages: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><rect x="2" y="4" width="16" height="18" rx="2" /><path d="M8 2h12a2 2 0 012 2v14" /></svg>),
  plus: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></svg>),
  check: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="20,6 9,17 4,12" /></svg>),
  undo: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M3 7v6h6" /><path d="M3 13a9 9 0 0116.5-5" /></svg>),
  batch: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><rect x="2" y="4" width="16" height="16" rx="2" /><rect x="6" y="2" width="16" height="16" rx="2" /></svg>),
  profile: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01z" /></svg>),
  rename: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M17 3a2.83 2.83 0 114 4L7.5 20.5 2 22l1.5-5.5L17 3z" /></svg>),
  duplicate: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" /><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" /></svg>),
  brain: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M12 2a7 7 0 0 0-7 7c0 3 2 5.5 4 7l3 3 3-3c2-1.5 4-4 4-7a7 7 0 0 0-7-7z" /><circle cx="12" cy="9" r="2" /></svg>),
  tag: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M20.59 13.41l-7.17 7.17a2 2 0 01-2.83 0L2 12V2h10l8.59 8.59a2 2 0 010 2.82z" /><line x1="7" y1="7" x2="7.01" y2="7" /></svg>),
  rules: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><polyline points="22,12 18,12 15,21 9,3 6,12 2,12" /></svg>),
  sparkle: (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M12 2l2.4 7.2L22 12l-7.6 2.8L12 22l-2.4-7.2L2 12l7.6-2.8z" /></svg>),
};

// ─── Sortable Preview Page (large, for central preview area) ────
function SortablePreviewPage({
  uniqueId,
  index,
  doc,
  isSelected,
  onSelect,
  onRemove,
  onContextMenu,
}: {
  uniqueId: string;
  index: number;
  doc: ScannedDocument | undefined;
  isSelected: boolean;
  onSelect: () => void;
  onRemove: () => void;
  onContextMenu: (e: React.MouseEvent) => void;
}) {
  const { attributes, listeners, setNodeRef, transform, transition } = useSortable({ id: uniqueId });
  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
  };

  return (
    <div
      ref={setNodeRef}
      style={style}
      {...attributes}
      {...listeners}
      className={`multipage-preview-item${isSelected ? " selected" : ""}`}
      onClick={onSelect}
      onContextMenu={onContextMenu}
    >
      <div className="multipage-preview-item-number">{index + 1}</div>
      {doc ? (
        <img src={doc.dataUrl} alt={`Page ${index + 1}`} className="multipage-preview-item-thumb" />
      ) : (
        <div className="multipage-preview-item-placeholder">?</div>
      )}
      <button className="multipage-preview-item-remove" onClick={(e) => { e.stopPropagation(); onRemove(); }} aria-label="Remove page">
        {Icons.close}
      </button>
    </div>
  );
}

// ─── App ─────────────────────────────────────────────────────────
function App() {
  const { t, language, setLanguage } = useTranslation();
  const tour = useTour();

  const [themeMode, setThemeMode] = useState<ThemeMode>(loadThemePreference);
  const [scanners, setScanners] = useState<ScannerDevice[]>([]);
  const [selectedScanner, setSelectedScanner] = useState<string>("");
  const [isScanning, setIsScanning] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [scanProgress, setScanProgress] = useState(0);
  const [statusMessage, setStatusMessage] = useState("");
  const [statusType, setStatusType] = useState<"ready" | "scanning" | "error">("ready");

  // Onboarding
  const [showOnboarding, setShowOnboarding] = useState(false);

  // v1.0.0: Auto-update
  const [updateAvailable, setUpdateAvailable] = useState<{ version: string; body: string } | null>(null);
  const [isUpdating, setIsUpdating] = useState(false);
  const pendingUpdateRef = useRef<{ downloadAndInstall: () => Promise<void> } | null>(null);

  useEffect(() => {
    applyTheme(themeMode);
    saveThemePreference(themeMode);
    if (themeMode === "auto") {
      const mq = window.matchMedia("(prefers-color-scheme: dark)");
      const handler = () => applyTheme("auto");
      mq.addEventListener("change", handler);
      return () => mq.removeEventListener("change", handler);
    }
  }, [themeMode]);

  const [documents, setDocuments] = useState<ScannedDocument[]>([]);
  const [selectedDocument, setSelectedDocument] = useState<ScannedDocument | null>(null);
  const [activeView, setActiveView] = useState<"preview" | "history" | "ocr">("preview");
  const [zoomLevel, setZoomLevel] = useState(100);

  const [ocrText, setOcrText] = useState<string | null>(null);
  const [isOcrRunning, setIsOcrRunning] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<HistoryEntryDto[] | null>(null);

  const [showSettings, setShowSettings] = useState(false);
  const [settingsTab, setSettingsTab] = useState<"general" | "scan" | "export" | "app">("general");

  // v0.3.0: Right panel mode
  const [rightPanelMode, setRightPanelMode] = useState<"config" | "edit" | "intelligence">("config");

  // v0.3.0: Image adjustments
  const [adjustments, setAdjustments] = useState<ImageAdjustments>({ brightness: 0, contrast: 0, saturation: 0, sharpness: 0 });
  const [isAdjusting, setIsAdjusting] = useState(false);
  const adjustmentTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // v0.3.0: Multi-page
  const [multipageDoc, setMultipageDoc] = useState<MultiPageDocDto | null>(null);

  // v0.4.0: Profiles
  const [scanProfiles, setScanProfiles] = useState<ScanProfile[]>([]);
  const [selectedProfileId, setSelectedProfileId] = useState<string | null>(null);

  // v0.4.0: Batch scanning
  const [batchMode, setBatchMode] = useState(false);
  const [batchPageCount, setBatchPageCount] = useState(5);

  // File drag-and-drop import
  const [isDragOver, setIsDragOver] = useState(false);
  const [isImporting, setIsImporting] = useState(false);

  // v0.6.0: Intelligence
  const [analysisResult, setAnalysisResult] = useState<AnalysisResultDto | null>(null);
  const [isAnalyzing, setIsAnalyzing] = useState(false);
  const [tagDefinitions, setTagDefinitions] = useState<TagDefinition[]>([]);
  const [documentTags, setDocumentTags] = useState<Record<string, string[]>>({});
  const [automationRules, setAutomationRules] = useState<AutomationRule[]>([]);
  const [showRulesModal, setShowRulesModal] = useState(false);
  const [editingRule, setEditingRule] = useState<AutomationRule | null>(null);

  // v0.4.0: Context menu
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; docId: string; pageIndex?: number } | null>(null);
  const [renamingDocId, setRenamingDocId] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");

  // a11y: Focus traps for modals
  const closeSettings = useCallback(() => setShowSettings(false), []);
  const closeRulesModal = useCallback(() => setShowRulesModal(false), []);
  const settingsRef = useFocusTrap(showSettings, closeSettings);
  const rulesRef = useFocusTrap(showRulesModal, closeRulesModal);

  const selectedDocIdRef = useRef<string | null>(null);
  useEffect(() => {
    selectedDocIdRef.current = selectedDocument?.id ?? null;
    setAnalysisResult(null);
  }, [selectedDocument?.id]);

  const [config, setConfig] = useState<ScanConfig>({
    dpi: 300,
    colorMode: "Couleur",
    paperFormat: "A4",
    duplex: false,
    adf: false,
  });

  const [settings, setSettings] = useState<AppSettings>({
    output_dir: "",
    default_format: "PDF",
    auto_crop: true,
    quality: 85,
    default_dpi: 300,
    default_color_mode: "Couleur",
    default_paper_format: "A4",
    auto_ocr: false,
    default_ocr_lang: "fra",
    naming_template: "Scan_{date}_{time}",
    watch_folder: null,
    scan_counter: 0,
    language: "fr",
    onboarding_complete: false,
  });

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 8 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates })
  );

  // ── Keyboard shortcuts (stable ref to avoid re-registering every render) ──
  const shortcutRef = useRef({ selectedDocument, selectedScanner, isScanning, saveAsPdf: () => {}, saveAsImage: () => {}, startScan: () => {}, printDoc: () => {}, runOcr: () => {}, setRightPanelMode });
  shortcutRef.current = { selectedDocument, selectedScanner, isScanning, saveAsPdf: () => saveAsPdf(), saveAsImage: () => saveAsImage(), startScan: () => startScan(), printDoc: () => printDoc(), runOcr: () => runOcr(), setRightPanelMode };
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const meta = e.metaKey || e.ctrlKey;
      if (!meta) return;
      const s = shortcutRef.current;
      switch (e.key.toLowerCase()) {
        case "s":
          if (e.shiftKey) { e.preventDefault(); if (s.selectedDocument) s.saveAsPdf(); }
          else { e.preventDefault(); if (s.selectedScanner && !s.isScanning) s.startScan(); }
          break;
        case "e": e.preventDefault(); if (s.selectedDocument) s.saveAsImage(); break;
        case "p": e.preventDefault(); if (s.selectedDocument) s.printDoc(); break;
        case "o": e.preventDefault(); s.runOcr(); break;
        case "z": if (e.shiftKey) { e.preventDefault(); s.setRightPanelMode("edit"); } break;
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  // ── Load scanners ──
  const tRef = useRef(t);
  tRef.current = t;
  const loadScanners = useCallback(async () => {
    const t = tRef.current;
    setIsRefreshing(true);
    setStatusMessage(t("status.searchingScanners"));
    setStatusType("scanning");
    try {
      const list = await invoke<ScannerDevice[]>("list_scanners");
      setScanners(list);
      if (list.length > 0 && !selectedScanner) setSelectedScanner(list[0].id);
      setStatusMessage(list.length > 0 ? t("status.scannersFound", { count: list.length }) : t("status.noScanners"));
      setStatusType("ready");
    } catch (err) {
      setScanners([]);
      setStatusMessage(t("status.error", { error: String(err) }));
      setStatusType("error");
    } finally {
      setIsRefreshing(false);
    }
  }, [selectedScanner]);

  useEffect(() => {
    loadScanners();
    invoke<AppSettings>("load_settings")
      .then((s) => {
        setSettings(s);
        setConfig((c) => ({ ...c, dpi: s.default_dpi, colorMode: s.default_color_mode, paperFormat: s.default_paper_format }));
        if (s.language) setLanguage(s.language as Language);
        if (!s.onboarding_complete) setShowOnboarding(true);
      })
      .catch(() => {});
    invoke<string>("get_documents_dir")
      .then((dir) => setSettings((s) => ({ ...s, output_dir: dir })))
      .catch(() => {});
    invoke<ScanProfile[]>("list_scan_profiles").then(setScanProfiles).catch(() => {});
    invoke<TagDefinition[]>("get_tag_definitions").then(setTagDefinitions).catch(() => {});
    invoke<Record<string, string[]>>("get_all_tags_map").then(setDocumentTags).catch(() => {});
    invoke<AutomationRule[]>("list_automation_rules").then(setAutomationRules).catch(() => {});
    setStatusMessage(t("status.ready"));
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // ── Onboarding complete handler ──
  const handleOnboardingComplete = async (partial: { language: Language; output_dir: string }) => {
    const updated = { ...settings, ...partial, onboarding_complete: true };
    try {
      await invoke("save_app_settings", { settings: updated });
      setSettings(updated);
      setLanguage(partial.language);
      setShowOnboarding(false);
      setTimeout(() => tour.start(), 400);
    } catch { setShowOnboarding(false); }
  };

  // ── Auto-update check ──
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const { check } = await import("@tauri-apps/plugin-updater");
        const update = await check();
        if (update && !cancelled) {
          pendingUpdateRef.current = update;
          setUpdateAvailable({ version: update.version, body: update.body ?? "" });
        }
      } catch { /* updater unavailable in dev */ }
    })();
    return () => { cancelled = true; };
  }, []);

  const installUpdate = async () => {
    const update = pendingUpdateRef.current;
    if (!update) return;
    setIsUpdating(true);
    try {
      await update.downloadAndInstall();
      const { relaunch } = await import("@tauri-apps/plugin-process");
      await relaunch();
    } catch {
      setIsUpdating(false);
    }
  };

  // ── File drag-and-drop import ──
  const importFiles = useCallback(async (paths: string[]) => {
    const supported = [".pdf", ".tif", ".tiff", ".png", ".jpg", ".jpeg", ".bmp", ".webp"];
    const validPaths = paths.filter((p) => supported.some((ext) => p.toLowerCase().endsWith(ext)));
    if (validPaths.length === 0) return;

    setIsImporting(true);
    setStatusMessage(t("status.importing"));
    setStatusType("scanning");

    // Collect all imported pages
    const allNewDocs: ScannedDocument[] = [];

    for (const filePath of validPaths) {
      try {
        const results = await invoke<ScanResultDto[]>("import_file", { filePath });
        const newDocs: ScannedDocument[] = results.map((r) => ({
          id: r.id,
          name: r.name,
          date: r.date,
          width: r.width,
          height: r.height,
          dataUrl: `data:image/png;base64,${r.image_base64}`,
        }));

        setDocuments((prev) => [...prev, ...newDocs]);
        allNewDocs.push(...newDocs);
      } catch (err) {
        setStatusMessage(t("status.importError", { error: String(err) }));
        setStatusType("error");
      }
    }

    if (allNewDocs.length > 0) {
      // Select first imported document
      setSelectedDocument(allNewDocs[0]);

      // Use existing multipage or create one automatically
      let mpDoc = multipageDoc;
      if (!mpDoc) {
        try {
          const name = `Import_${new Date().toISOString().slice(0, 10)}`;
          mpDoc = await invoke<MultiPageDocDto>("create_multipage_document", { name });
        } catch { /* ignore */ }
      }

      // Add all pages to multipage document
      if (mpDoc) {
        for (const doc of allNewDocs) {
          try {
            mpDoc = await invoke<MultiPageDocDto>("add_page_to_document", {
              multipageId: mpDoc.id,
              docId: doc.id,
              position: null,
            });
          } catch { /* ignore individual page add errors */ }
        }
        setMultipageDoc(mpDoc);
        setActiveView("preview");
      }

      setStatusMessage(t("status.importComplete", { count: allNewDocs.length }));
      setStatusType("ready");
    }
    setIsImporting(false);
  }, [multipageDoc, t]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    const appWindow = getCurrentWindow();
    const unlisten = appWindow.onDragDropEvent((event) => {
      if (event.payload.type === "over") {
        setIsDragOver(true);
      } else if (event.payload.type === "drop") {
        setIsDragOver(false);
        if (event.payload.paths?.length) {
          importFiles(event.payload.paths);
        }
      } else if (event.payload.type === "leave") {
        setIsDragOver(false);
      }
    });
    return () => { unlisten.then((fn) => fn()); };
  }, [importFiles]);

  // ── Scan ──
  const startScan = async () => {
    if (!selectedScanner || isScanning) return;
    setIsScanning(true);
    setScanProgress(0);
    setStatusMessage(t("status.scanning"));
    setStatusType("scanning");
    const progressInterval = setInterval(() => { setScanProgress((p) => (p >= 95 ? p : p + Math.random() * 8)); }, 300);
    try {
      const result = await invoke<ScanResultDto>("scan_document", {
        options: { device_id: selectedScanner, dpi: config.dpi, color_mode: config.colorMode, duplex: config.duplex, paper_format: config.paperFormat },
      });
      clearInterval(progressInterval);
      setScanProgress(100);
      const newDoc = dtoToDoc(result);
      setDocuments((docs) => [newDoc, ...docs]);
      setSelectedDocument(newDoc);
      setActiveView("preview");
      setStatusMessage(t("status.scanComplete"));
      setStatusType("ready");
    } catch (err) {
      clearInterval(progressInterval);
      setScanProgress(0);
      setStatusMessage(t("status.error", { error: String(err) }));
      setStatusType("error");
    } finally {
      setTimeout(() => { setIsScanning(false); setScanProgress(0); }, 600);
    }
  };

  // ── Save as PDF ──
  const saveAsPdf = async () => {
    // Multipage mode: save all pages as a single PDF
    if (multipageDoc && multipageDoc.page_count > 0) {
      return saveMultipagePdf();
    }
    if (!selectedDocument) return;
    try {
      const path = await save({ defaultPath: selectedDocument.name.replace(/\.\w+$/, ".pdf"), filters: [{ name: "PDF", extensions: ["pdf"] }] });
      if (!path) return;
      setStatusMessage(t("status.savingPdf"));
      setStatusType("scanning");
      await invoke<string>("save_document_as_pdf", { docId: selectedDocument.id, outputPath: path });
      setStatusMessage(t("status.pdfSaved", { filename: path.split(/[/\\]/).pop() ?? "" }));
      setStatusType("ready");
    } catch (err) {
      setStatusMessage(t("status.pdfError", { error: String(err) }));
      setStatusType("error");
    }
  };

  // ── Save as Image ──
  const saveAsImage = async () => {
    // Multipage mode: save all pages as individual images
    if (multipageDoc && multipageDoc.page_count > 0) {
      try {
        const fmt = settings.default_format === "PDF" ? "PNG" : settings.default_format;
        const ext = fmt.toLowerCase();
        const baseName = multipageDoc.name || "Document";
        const path = await save({
          defaultPath: `${baseName}_page1.${ext}`,
          filters: [{ name: "PNG", extensions: ["png"] }, { name: "JPEG", extensions: ["jpg", "jpeg"] }, { name: "TIFF", extensions: ["tiff", "tif"] }, { name: "BMP", extensions: ["bmp"] }],
        });
        if (!path) return;
        setStatusMessage(t("status.saving"));
        setStatusType("scanning");
        const detectedFormat = path.split(".").pop()?.toUpperCase() || "PNG";
        const dir = path.replace(/[/\\][^/\\]+$/, "");
        for (let i = 0; i < multipageDoc.page_ids.length; i++) {
          const pageId = multipageDoc.page_ids[i];
          const pagePath = i === 0 ? path : `${dir}/${baseName}_page${i + 1}.${ext}`;
          await invoke<string>("save_document_as_image", { docId: pageId, outputPath: pagePath, format: detectedFormat, quality: settings.quality });
        }
        setStatusMessage(t("status.imageSaved", { filename: `${multipageDoc.page_count} pages` }));
        setStatusType("ready");
      } catch (err) {
        setStatusMessage(t("status.saveError", { error: String(err) }));
        setStatusType("error");
      }
      return;
    }
    if (!selectedDocument) return;
    try {
      const fmt = settings.default_format === "PDF" ? "PNG" : settings.default_format;
      const ext = fmt.toLowerCase();
      const path = await save({
        defaultPath: selectedDocument.name.replace(/\.\w+$/, `.${ext}`),
        filters: [{ name: "PNG", extensions: ["png"] }, { name: "JPEG", extensions: ["jpg", "jpeg"] }, { name: "TIFF", extensions: ["tiff", "tif"] }, { name: "BMP", extensions: ["bmp"] }],
      });
      if (!path) return;
      setStatusMessage(t("status.saving"));
      setStatusType("scanning");
      const detectedFormat = path.split(".").pop()?.toUpperCase() || "PNG";
      await invoke<string>("save_document_as_image", { docId: selectedDocument.id, outputPath: path, format: detectedFormat, quality: settings.quality });
      setStatusMessage(t("status.imageSaved", { filename: path.split(/[/\\]/).pop() ?? "" }));
      setStatusType("ready");
    } catch (err) {
      setStatusMessage(t("status.saveError", { error: String(err) }));
      setStatusType("error");
    }
  };

  // ── Print ──
  const printDoc = async () => {
    // Multipage mode: generate temp PDF and print it
    if (multipageDoc && multipageDoc.page_count > 0) {
      try {
        setStatusMessage(t("status.printing"));
        setStatusType("scanning");
        await invoke("print_multipage_document", { multipageId: multipageDoc.id });
        setStatusMessage(t("status.printed"));
        setStatusType("ready");
      } catch (err) {
        setStatusMessage(t("status.printError", { error: String(err) }));
        setStatusType("error");
      }
      return;
    }
    if (!selectedDocument) return;
    try {
      setStatusMessage(t("status.printing"));
      await invoke("print_document", { docId: selectedDocument.id });
      setStatusMessage(t("status.printed"));
      setStatusType("ready");
    } catch (err) {
      setStatusMessage(t("status.printError", { error: String(err) }));
      setStatusType("error");
    }
  };

  // ── Auto-crop ──
  const autoCrop = async () => {
    if (!selectedDocument) return;
    try {
      setStatusMessage(t("status.cropping"));
      setStatusType("scanning");
      const result = await invoke<ScanResultDto>("auto_crop_document", { docId: selectedDocument.id });
      const updated = dtoToDoc(result);
      setSelectedDocument(updated);
      setDocuments((docs) => docs.map((d) => (d.id === updated.id ? updated : d)));
      setStatusMessage(t("status.cropComplete"));
      setStatusType("ready");
    } catch (err) {
      setStatusMessage(t("status.cropError", { error: String(err) }));
      setStatusType("error");
    }
  };

  const deleteDocument = (docId: string) => {
    setDocuments((docs) => docs.filter((d) => d.id !== docId));
    if (selectedDocument?.id === docId) setSelectedDocument(null);
    invoke("delete_history_entry", { docId }).catch(() => {});
  };

  // ── OCR ──
  const runOcr = async () => {
    if (!selectedDocument || isOcrRunning) return;
    setIsOcrRunning(true);
    setStatusMessage(t("status.ocrRunning"));
    setStatusType("scanning");
    try {
      const text = await invoke<string>("run_ocr", { docId: selectedDocument.id, lang: settings.default_ocr_lang });
      setOcrText(text);
      setActiveView("ocr");
      setStatusMessage(t("status.ocrComplete"));
      setStatusType("ready");
    } catch (err) {
      setStatusMessage(t("status.ocrError", { error: String(err) }));
      setStatusType("error");
    } finally {
      setIsOcrRunning(false);
    }
  };

  const copyOcrText = async () => {
    if (!ocrText) return;
    try {
      await navigator.clipboard.writeText(ocrText);
      setStatusMessage(t("status.textCopied"));
      setStatusType("ready");
    } catch {
      setStatusMessage(t("status.copyError"));
      setStatusType("error");
    }
  };

  const handleSearch = async (query: string) => {
    setSearchQuery(query);
    if (!query.trim()) { setSearchResults(null); return; }
    try {
      const results = await invoke<HistoryEntryDto[]>("search_documents", { query });
      setSearchResults(results);
    } catch { setSearchResults(null); }
  };

  const handleSaveSettings = async () => {
    try {
      const updatedSettings = { ...settings, language };
      await invoke("save_app_settings", { settings: updatedSettings });
      setShowSettings(false);
      setStatusMessage(t("status.settingsSaved"));
      setStatusType("ready");
    } catch (err) {
      setStatusMessage(t("status.settingsError", { error: String(err) }));
      setStatusType("error");
    }
  };

  const selectOutputDir = async () => {
    try {
      const dir = await selectDirectory();
      if (dir) setSettings((s) => ({ ...s, output_dir: dir }));
    } catch { /* Dialog not available */ }
  };

  // ─── Profiles ────────────────────────────────────────────────────
  const applyProfile = (profile: ScanProfile) => {
    setSelectedProfileId(profile.id);
    setConfig({ dpi: profile.dpi, colorMode: profile.color_mode, paperFormat: profile.paper_format, duplex: profile.duplex, adf: config.adf });
    setSettings((s) => ({ ...s, auto_crop: profile.auto_crop, auto_ocr: profile.auto_ocr }));
    setStatusMessage(t("status.profileApplied", { name: profile.name }));
    setStatusType("ready");
  };

  const saveCurrentAsProfile = async (name: string) => {
    const profile: ScanProfile = { id: crypto.randomUUID(), name, dpi: config.dpi, color_mode: config.colorMode, paper_format: config.paperFormat, duplex: config.duplex, auto_crop: settings.auto_crop, auto_ocr: settings.auto_ocr };
    try {
      const updated = await invoke<ScanProfile[]>("save_scan_profile", { profile });
      setScanProfiles(updated);
      setSelectedProfileId(profile.id);
      setStatusMessage(t("status.profileSaved", { name }));
      setStatusType("ready");
    } catch (err) {
      setStatusMessage(t("status.profileError", { error: String(err) }));
      setStatusType("error");
    }
  };

  const deleteProfile = async (profileId: string) => {
    try {
      const updated = await invoke<ScanProfile[]>("delete_scan_profile", { profileId });
      setScanProfiles(updated);
      if (selectedProfileId === profileId) setSelectedProfileId(null);
    } catch (err) {
      setStatusMessage(t("status.error", { error: String(err) }));
      setStatusType("error");
    }
  };

  // ─── Batch scanning ──────────────────────────────────────────────
  const startBatchScan = async () => {
    if (!selectedScanner || isScanning) return;
    setIsScanning(true);
    setScanProgress(0);
    setStatusMessage(t("status.batchScanning", { count: batchPageCount }));
    setStatusType("scanning");
    const progressInterval = setInterval(() => { setScanProgress((p) => (p >= 95 ? p : p + Math.random() * 3)); }, 500);
    try {
      const results = await invoke<ScanResultDto[]>("batch_scan", {
        options: { device_id: selectedScanner, dpi: config.dpi, color_mode: config.colorMode, duplex: config.duplex, paper_format: config.paperFormat },
        pageCount: batchPageCount,
      });
      clearInterval(progressInterval);
      setScanProgress(100);
      const newDocs = results.map(dtoToDoc);
      setDocuments((docs) => [...newDocs.reverse(), ...docs]);
      if (newDocs.length > 0) { setSelectedDocument(newDocs[0]); setActiveView("preview"); }
      setStatusMessage(t("status.batchDone", { count: results.length }));
      setStatusType("ready");
    } catch (err) {
      clearInterval(progressInterval);
      setScanProgress(0);
      setStatusMessage(t("status.batchError", { error: String(err) }));
      setStatusType("error");
    } finally {
      setTimeout(() => { setIsScanning(false); setScanProgress(0); }, 600);
    }
  };

  // ─── Document actions ────────────────────────────────────────────
  const renameDoc = async (docId: string, newName: string) => {
    try {
      await invoke("rename_document", { docId, newName });
      setDocuments((docs) => docs.map((d) => d.id === docId ? { ...d, name: newName } : d));
      if (selectedDocument?.id === docId) setSelectedDocument((prev) => prev ? { ...prev, name: newName } : prev);
      setRenamingDocId(null);
      setStatusMessage(t("status.renamed"));
      setStatusType("ready");
    } catch (err) {
      setStatusMessage(t("status.error", { error: String(err) }));
      setStatusType("error");
    }
  };

  const duplicateDoc = async (docId: string) => {
    try {
      const result = await invoke<ScanResultDto>("duplicate_document", { docId });
      const newDoc = dtoToDoc(result);
      setDocuments((docs) => [newDoc, ...docs]);
      setSelectedDocument(newDoc);
      setStatusMessage(t("status.duplicated"));
      setStatusType("ready");
    } catch (err) {
      setStatusMessage(t("status.error", { error: String(err) }));
      setStatusType("error");
    }
  };

  // ─── Intelligence ────────────────────────────────────────────────
  const analyzeDocument = async () => {
    if (!selectedDocument) return;
    try {
      setIsAnalyzing(true);
      setStatusMessage(t("status.analyzing"));
      setStatusType("scanning");
      const result = await invoke<AnalysisResultDto>("analyze_document", { docId: selectedDocument.id });
      setAnalysisResult(result);
      if (result.auto_tags.length > 0) {
        const currentTags = documentTags[selectedDocument.id] || [];
        const newTags = [...new Set([...currentTags, ...result.auto_tags])];
        await invoke("set_document_tags", { docId: selectedDocument.id, tags: newTags });
        setDocumentTags((prev) => ({ ...prev, [selectedDocument.id]: newTags }));
      }
      setRightPanelMode("intelligence");
      const docTypeLabel = t(`docTypes.${result.classification.doc_type}`) || result.classification.doc_type;
      setStatusMessage(t("status.analysisDone", { type: docTypeLabel, confidence: Math.round(result.classification.confidence * 100) }));
      setStatusType("ready");
    } catch (err) {
      setStatusMessage(t("status.analysisError", { error: String(err) }));
      setStatusType("error");
    } finally {
      setIsAnalyzing(false);
    }
  };

  const addTag = async (docId: string, tag: string) => {
    try {
      const tags = await invoke<string[]>("add_document_tag", { docId, tag });
      setDocumentTags((prev) => ({ ...prev, [docId]: tags }));
    } catch { /* ignore */ }
  };

  const removeTag = async (docId: string, tag: string) => {
    try {
      const tags = await invoke<string[]>("remove_document_tag", { docId, tag });
      setDocumentTags((prev) => ({ ...prev, [docId]: tags }));
    } catch { /* ignore */ }
  };

  const applySuggestion = async () => {
    if (!selectedDocument || !analysisResult) return;
    const { suggestion } = analysisResult;
    await invoke("rename_document", { docId: selectedDocument.id, newName: suggestion.suggested_name });
    setDocuments((docs) => docs.map((d) => d.id === selectedDocument.id ? { ...d, name: suggestion.suggested_name } : d));
    setSelectedDocument((prev) => prev ? { ...prev, name: suggestion.suggested_name } : prev);
    const currentTags = documentTags[selectedDocument.id] || [];
    const newTags = [...new Set([...currentTags, ...suggestion.suggested_tags])];
    await invoke("set_document_tags", { docId: selectedDocument.id, tags: newTags });
    setDocumentTags((prev) => ({ ...prev, [selectedDocument.id]: newTags }));
    setStatusMessage(t("status.suggestionsApplied"));
    setStatusType("ready");
  };

  const saveRule = async (rule: AutomationRule) => {
    try {
      const updated = await invoke<AutomationRule[]>("save_automation_rule", { rule });
      setAutomationRules(updated);
      setEditingRule(null);
      setStatusMessage(t("status.ruleSaved", { name: rule.name }));
      setStatusType("ready");
    } catch (err) {
      setStatusMessage(t("status.error", { error: String(err) }));
      setStatusType("error");
    }
  };

  const deleteRule = async (ruleId: string) => {
    try {
      const updated = await invoke<AutomationRule[]>("delete_automation_rule", { ruleId });
      setAutomationRules(updated);
    } catch (err) {
      setStatusMessage(t("status.error", { error: String(err) }));
      setStatusType("error");
    }
  };

  // ─── Rotation & Flip ────────────────────────────────────────────
  const rotateDocument = async (direction: string, targetDocId?: string) => {
    const docId = targetDocId || selectedDocument?.id;
    if (!docId) return;
    try {
      setStatusMessage(t("status.rotating", { deg: direction }));
      setStatusType("scanning");
      const result = await invoke<ScanResultDto>("rotate_document", { docId, direction });
      const updated = dtoToDoc(result);
      if (selectedDocument?.id === updated.id) setSelectedDocument(updated);
      setDocuments((docs) => docs.map((d) => (d.id === updated.id ? updated : d)));
      setStatusMessage(t("status.rotateComplete", { deg: direction }));
      setStatusType("ready");
    } catch (err) {
      setStatusMessage(t("status.rotateError", { error: String(err) }));
      setStatusType("error");
    }
  };

  const flipDocument = async (axis: string, targetDocId?: string) => {
    const docId = targetDocId || selectedDocument?.id;
    if (!docId) return;
    try {
      setStatusMessage(t("status.flipping"));
      setStatusType("scanning");
      const result = await invoke<ScanResultDto>("flip_document", { docId, axis });
      const updated = dtoToDoc(result);
      if (selectedDocument?.id === updated.id) setSelectedDocument(updated);
      setDocuments((docs) => docs.map((d) => (d.id === updated.id ? updated : d)));
      setStatusMessage(t("status.flipComplete"));
      setStatusType("ready");
    } catch (err) {
      setStatusMessage(t("status.flipError", { error: String(err) }));
      setStatusType("error");
    }
  };

  // ─── Image Adjustments ──────────────────────────────────────────
  const handleAdjustmentChange = (key: keyof ImageAdjustments, value: number) => {
    const newAdj = { ...adjustments, [key]: value };
    setAdjustments(newAdj);
    if (adjustmentTimerRef.current) clearTimeout(adjustmentTimerRef.current);
    adjustmentTimerRef.current = setTimeout(() => { previewAdjustments(newAdj); }, 150);
  };

  const previewAdjustments = async (adj: ImageAdjustments) => {
    const docId = selectedDocIdRef.current;
    if (!docId) return;
    try {
      setIsAdjusting(true);
      const result = await invoke<AdjustmentPreviewResult>("preview_adjustments", { docId, adjustments: adj });
      const dataUrl = `data:image/png;base64,${result.image_base64}`;
      setSelectedDocument((prev) => prev && prev.id === docId ? { ...prev, dataUrl, width: result.width, height: result.height } : prev);
      setDocuments((docs) => docs.map((d) => d.id === docId ? { ...d, dataUrl, width: result.width, height: result.height } : d));
    } catch { /* Preview failed */ } finally {
      setIsAdjusting(false);
    }
  };

  const commitAdjustments = async () => {
    if (!selectedDocument) return;
    try {
      setStatusMessage(t("status.adjusting"));
      setStatusType("scanning");
      const result = await invoke<ScanResultDto>("commit_adjustments", { docId: selectedDocument.id, adjustments });
      const updated = dtoToDoc(result);
      setSelectedDocument(updated);
      setDocuments((docs) => docs.map((d) => (d.id === updated.id ? updated : d)));
      setAdjustments({ brightness: 0, contrast: 0, saturation: 0, sharpness: 0 });
      setStatusMessage(t("status.adjustmentsApplied"));
      setStatusType("ready");
    } catch (err) {
      setStatusMessage(t("status.adjustmentsError", { error: String(err) }));
      setStatusType("error");
    }
  };

  const revertAdjustments = async () => {
    if (!selectedDocument) return;
    try {
      const result = await invoke<ScanResultDto>("revert_adjustments", { docId: selectedDocument.id });
      const updated = dtoToDoc(result);
      setSelectedDocument(updated);
      setDocuments((docs) => docs.map((d) => (d.id === updated.id ? updated : d)));
      setAdjustments({ brightness: 0, contrast: 0, saturation: 0, sharpness: 0 });
      setStatusMessage(t("status.adjustmentsCancelled"));
      setStatusType("ready");
    } catch (err) {
      setStatusMessage(t("status.error", { error: String(err) }));
      setStatusType("error");
    }
  };

  // ─── Processing operations ──────────────────────────────────────
  const denoiseDocument = async (strength: number) => {
    if (!selectedDocument) return;
    try {
      setStatusMessage(t("status.denoising"));
      setStatusType("scanning");
      const result = await invoke<ScanResultDto>("denoise_document", { docId: selectedDocument.id, strength });
      const updated = dtoToDoc(result);
      setSelectedDocument(updated);
      setDocuments((docs) => docs.map((d) => (d.id === updated.id ? updated : d)));
      setStatusMessage(t("status.denoiseComplete"));
      setStatusType("ready");
    } catch (err) {
      setStatusMessage(t("status.denoiseError", { error: String(err) }));
      setStatusType("error");
    }
  };

  const deskewDocument = async () => {
    if (!selectedDocument) return;
    try {
      setStatusMessage(t("status.deskewing"));
      setStatusType("scanning");
      const result = await invoke<ScanResultDto>("deskew_document", { docId: selectedDocument.id });
      const updated = dtoToDoc(result);
      setSelectedDocument(updated);
      setDocuments((docs) => docs.map((d) => (d.id === updated.id ? updated : d)));
      setStatusMessage(t("status.deskewComplete"));
      setStatusType("ready");
    } catch (err) {
      setStatusMessage(t("status.deskewError", { error: String(err) }));
      setStatusType("error");
    }
  };

  const whitenBackground = async () => {
    if (!selectedDocument) return;
    try {
      setStatusMessage(t("status.whitening"));
      setStatusType("scanning");
      const result = await invoke<ScanResultDto>("whiten_document_background", { docId: selectedDocument.id, threshold: 200 });
      const updated = dtoToDoc(result);
      setSelectedDocument(updated);
      setDocuments((docs) => docs.map((d) => (d.id === updated.id ? updated : d)));
      setStatusMessage(t("status.whiteningComplete"));
      setStatusType("ready");
    } catch (err) {
      setStatusMessage(t("status.whiteningError", { error: String(err) }));
      setStatusType("error");
    }
  };



  const addPageToMultipage = async (docId: string) => {
    if (!multipageDoc) return;
    try {
      const updated = await invoke<MultiPageDocDto>("add_page_to_document", { multipageId: multipageDoc.id, docId, position: null });
      setMultipageDoc(updated);
      setStatusMessage(t("status.pageAdded", { count: updated.page_count }));
      setStatusType("ready");
    } catch (err) {
      setStatusMessage(t("status.error", { error: String(err) }));
      setStatusType("error");
    }
  };

  const removePageFromMultipage = async (pageIndex: number) => {
    if (!multipageDoc) return;
    try {
      const updated = await invoke<MultiPageDocDto>("remove_page_from_document", { multipageId: multipageDoc.id, pageIndex });
      setMultipageDoc(updated);
    } catch (err) {
      setStatusMessage(t("status.error", { error: String(err) }));
      setStatusType("error");
    }
  };

  const handleDragEnd = async (event: DragEndEvent) => {
    if (!multipageDoc) return;
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    const activeIdx = parseInt((active.id as string).split("-")[1], 10);
    const overIdx = parseInt((over.id as string).split("-")[1], 10);
    if (isNaN(activeIdx) || isNaN(overIdx)) return;
    const previousDoc = multipageDoc;
    const newIds = arrayMove(multipageDoc.page_ids, activeIdx, overIdx);
    setMultipageDoc({ ...multipageDoc, page_ids: newIds, page_count: newIds.length });
    const usedIndices = new Set<number>();
    const safeOrder = newIds.map((id) => {
      for (let i = 0; i < multipageDoc.page_ids.length; i++) {
        if (multipageDoc.page_ids[i] === id && !usedIndices.has(i)) { usedIndices.add(i); return i; }
      }
      return 0;
    });
    try {
      const updated = await invoke<MultiPageDocDto>("reorder_document_pages", { multipageId: multipageDoc.id, newOrder: safeOrder });
      setMultipageDoc(updated);
    } catch (err) {
      setMultipageDoc(previousDoc);
      setStatusMessage(t("status.reorderError", { error: String(err) }));
      setStatusType("error");
    }
  };

  const saveMultipagePdf = async () => {
    if (!multipageDoc || multipageDoc.page_count === 0) return;
    try {
      const path = await save({ defaultPath: `${multipageDoc.name}.pdf`, filters: [{ name: "PDF", extensions: ["pdf"] }] });
      if (!path) return;
      setStatusMessage(t("status.savingMultipagePdf"));
      setStatusType("scanning");
      await invoke<string>("save_multipage_as_pdf", { multipageId: multipageDoc.id, outputPath: path });
      setStatusMessage(t("status.multipagePdfSaved", { filename: path.split(/[/\\]/).pop() ?? "" }));
      setStatusType("ready");
    } catch (err) {
      setStatusMessage(t("status.error", { error: String(err) }));
      setStatusType("error");
    }
  };

  // ── Helpers ──
  const dtoToDoc = (result: ScanResultDto): ScannedDocument => ({
    id: result.id, name: result.name, date: result.date, width: result.width, height: result.height,
    dataUrl: `data:image/png;base64,${result.image_base64}`,
  });

  const currentScanner = scanners.find((s) => s.id === selectedScanner);
  const dpiOptions = currentScanner?.capabilities.resolutions ?? [150, 300, 600, 1200];
  const colorOptions = currentScanner?.capabilities.color_modes ?? ["Couleur", "Niveaux de gris", "Noir et blanc"];
  const hasDocument = selectedDocument !== null;
  const hasMultipage = multipageDoc !== null && multipageDoc.page_count > 0;
  const canExport = hasDocument || hasMultipage;
  const hasAdjustments = adjustments.brightness !== 0 || adjustments.contrast !== 0 || adjustments.saturation !== 0 || adjustments.sharpness !== 0;

  return (
    <div className="app">
      {/* a11y: Skip link */}
      <a href="#main-content" className="skip-link">{t("a11y.skipToContent")}</a>

      {/* Drag-and-drop overlay */}
      {isDragOver && (
        <div className="drop-overlay" aria-hidden="true">
          <div className="drop-overlay-content">
            <svg width="64" height="64" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
              <polyline points="17 8 12 3 7 8" />
              <line x1="12" y1="3" x2="12" y2="15" />
            </svg>
            <h2>{t("dropzone.title")}</h2>
            <p>{t("dropzone.subtitle")}</p>
          </div>
        </div>
      )}

      {/* Import loading overlay */}
      {isImporting && (
        <div className="drop-overlay importing" aria-hidden="true">
          <div className="drop-overlay-content">
            <div className="import-spinner" />
            <h2>{t("status.importing")}</h2>
          </div>
        </div>
      )}

      {/* Background */}
      <div className="bg-mesh" aria-hidden="true">
        <div className="orb orb-1" /><div className="orb orb-2" /><div className="orb orb-3" />
      </div>

      {/* Sidebar */}
      <aside className="sidebar">
        <div className="sidebar-header">
          <div className="sidebar-logo">
            <img src="/logo.svg" alt="Photon" className="sidebar-logo-img" />
            <div>
              <div className="sidebar-title">{t("app.title")}</div>
              <div className="sidebar-subtitle">{t("app.subtitle")}</div>
            </div>
          </div>
        </div>

        <div className="sidebar-section">
          <div className="sidebar-section-header">
            <span className="sidebar-section-title">{t("sidebar.devices")}</span>
            <button className={`btn btn-icon btn-ghost ${isRefreshing ? "refreshing" : ""}`} onClick={loadScanners} aria-label={t("sidebar.refresh")} disabled={isRefreshing}>
              {Icons.refresh}
            </button>
          </div>

          <div id="sidebar-scanners" className="scanner-list" role="radiogroup" aria-label={t("a11y.scannerList")}>
            {scanners.length === 0 ? (
              <div className="scanner-empty">
                <div className="scanner-empty-icon">{Icons.scanner}</div>
                <p>{t("sidebar.noScannerTitle")}</p>
                <p className="scanner-empty-hint">{t("sidebar.noScannerHint")}</p>
              </div>
            ) : (
              scanners.map((scanner) => (
                <button
                  key={scanner.id}
                  role="radio"
                  aria-checked={selectedScanner === scanner.id}
                  className={`scanner-item ${selectedScanner === scanner.id ? "active" : ""}`}
                  onClick={() => setSelectedScanner(scanner.id)}
                >
                  <div className="scanner-item-header">
                    <div className="scanner-status-dot online" />
                    <span className="scanner-name">{scanner.name}</span>
                  </div>
                  <div className="scanner-vendor">{scanner.vendor}</div>
                </button>
              ))
            )}
          </div>
        </div>

        <div className="theme-switcher">
          <button className={`theme-btn ${themeMode === "light" ? "active" : ""}`} onClick={() => setThemeMode("light")} title={t("sidebar.themeLight")}>{Icons.sun}</button>
          <button className={`theme-btn ${themeMode === "dark" ? "active" : ""}`} onClick={() => setThemeMode("dark")} title={t("sidebar.themeDark")}>{Icons.moon}</button>
          <button className={`theme-btn ${themeMode === "auto" ? "active" : ""}`} onClick={() => setThemeMode("auto")} title={t("sidebar.themeAuto")}>{Icons.auto} Auto</button>
        </div>
      </aside>

      {/* Main Content */}
      <main id="main-content" className="main-content">
        {/* Action Bar */}
        <div className="action-bar">
          <button id="btn-scan" className={`btn btn-accent btn-scan ${isScanning ? "scanning" : ""}`} onClick={startScan} disabled={isScanning || !selectedScanner}>
            {Icons.scan}
            {isScanning ? t("actions.scanning") : t("actions.scan")}
          </button>

          <div className="action-bar-divider" />

          <button id="btn-save-pdf" className="btn" onClick={saveAsPdf} disabled={!canExport} title={t("actions.savePdf")}>{Icons.pdf}<span>{t("actions.pdf")}</span></button>
          <button className="btn" onClick={saveAsImage} disabled={!canExport} title={t("actions.saveImage")}>{Icons.image}<span>{t("actions.image")}</span></button>
          <button className="btn" onClick={printDoc} disabled={!canExport} title={t("actions.print")}>{Icons.print}<span>{t("actions.print")}</span></button>

          <div className="action-bar-divider" />

          <button className="btn" onClick={autoCrop} disabled={!hasDocument} title={t("actions.autoCrop")}>{Icons.crop}<span>{t("actions.crop")}</span></button>
          <button id="btn-ocr" className="btn btn-ocr" onClick={runOcr} disabled={!hasDocument || isOcrRunning} title={t("actions.ocrTooltip")}>
            {Icons.ocr}
            <span>{isOcrRunning ? t("actions.ocrRunning") : t("actions.ocr")}</span>
          </button>
          <button id="btn-analyze" className="btn" onClick={analyzeDocument} disabled={!hasDocument || isAnalyzing} title={t("actions.analyzeTooltip")}>
            {Icons.brain}
            <span>{isAnalyzing ? t("actions.analyzing") : t("actions.analyze")}</span>
          </button>

          <div className="action-bar-divider" />

          <button className="btn btn-icon btn-ghost" onClick={() => setShowRulesModal(true)} aria-label={t("actions.rules")}>{Icons.rules}</button>

          <div className="action-bar-spacer" />

          <button id="btn-settings" className="btn btn-icon btn-ghost" onClick={() => setShowSettings(true)} aria-label={t("actions.settings")}>{Icons.settings}</button>
        </div>

        {/* Content Area */}
        <div className="content-area">
          {/* Preview / History Panel */}
          <div className="preview-panel">
            <div className="preview-tabs" role="tablist" aria-label={t("a11y.previewTabs")}>
              <button role="tab" aria-selected={activeView === "preview"} className={`preview-tab ${activeView === "preview" ? "active" : ""}`} onClick={() => setActiveView("preview")}>{t("preview.tabPreview")}</button>
              <button role="tab" aria-selected={activeView === "history"} className={`preview-tab ${activeView === "history" ? "active" : ""}`} onClick={() => setActiveView("history")}>{t("preview.tabHistory", { count: documents.length })}</button>
              <button role="tab" aria-selected={activeView === "ocr"} className={`preview-tab ${activeView === "ocr" ? "active" : ""}`} onClick={() => setActiveView("ocr")} disabled={!ocrText}>{t("preview.tabOcr")}</button>

              {activeView === "preview" && (
                <div className="preview-tabs-right">
                  <button className="btn btn-icon btn-sm btn-ghost" onClick={() => setZoomLevel((z) => Math.max(25, z - 25))} aria-label={t("a11y.zoomOut")}>{Icons.zoomOut}</button>
                  <span className="zoom-label">{zoomLevel}%</span>
                  <button className="btn btn-icon btn-sm btn-ghost" onClick={() => setZoomLevel((z) => Math.min(200, z + 25))} aria-label={t("a11y.zoomIn")}>{Icons.zoomIn}</button>
                </div>
              )}
            </div>

            {activeView === "preview" ? (
              <div className="preview-content" role="tabpanel">
                {isScanning ? (
                  <div className="scan-in-progress">
                    <div className="scan-animation"><div className="scan-page"><div className="scan-line-sweep" /></div></div>
                    <div className="scan-status-text">{t("preview.scanningStatus")}</div>
                    <div className="scan-progress-inline">
                      <div className="scan-progress-bar" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(Math.min(scanProgress, 100))} aria-label={t("status.scanning")}>
                        <div className="scan-progress-fill" style={{ width: `${Math.min(scanProgress, 100)}%` }} />
                      </div>
                      <span className="scan-progress-pct">{Math.round(Math.min(scanProgress, 100))}%</span>
                    </div>
                  </div>
                ) : multipageDoc && multipageDoc.page_count > 0 ? (
                  <div className="multipage-preview">
                    <div className="multipage-preview-header">
                      <span className="multipage-preview-title">{multipageDoc.name}</span>
                      <span className="multipage-preview-count">{t("multipage.pageCount", { count: multipageDoc.page_count })}</span>
                      <div className="multipage-preview-actions">
                        <button className="btn btn-sm btn-accent" onClick={saveMultipagePdf} disabled={multipageDoc.page_count === 0}>{Icons.pdf} {t("multipage.savePdf")}</button>
                        <button className="btn btn-sm" onClick={() => setMultipageDoc(null)}>{Icons.close} {t("multipage.close")}</button>
                      </div>
                    </div>
                    <DndContext sensors={sensors} collisionDetection={rectIntersection} onDragEnd={handleDragEnd}>
                      <SortableContext items={multipageDoc.page_ids.map((_, i) => `page-${i}`)} strategy={rectSortingStrategy}>
                        <div className="multipage-preview-grid">
                          {multipageDoc.page_ids.map((pid, i) => {
                            const pageDoc = documents.find((d) => d.id === pid);
                            return (
                              <SortablePreviewPage
                                key={`page-${i}`}
                                uniqueId={`page-${i}`}
                                index={i}
                                doc={pageDoc}
                                isSelected={selectedDocument?.id === pid}
                                onSelect={() => { if (pageDoc) setSelectedDocument(pageDoc); }}
                                onRemove={() => removePageFromMultipage(i)}
                                onContextMenu={(e) => { e.preventDefault(); if (pageDoc) setContextMenu({ x: e.clientX, y: e.clientY, docId: pid, pageIndex: i }); }}
                              />
                            );
                          })}
                        </div>
                      </SortableContext>
                    </DndContext>
                  </div>
                ) : selectedDocument ? (
                  <div className="preview-image-container" style={{ width: `${zoomLevel * 2.8}px` }}>
                    <img src={selectedDocument.dataUrl} alt={selectedDocument.name} className="preview-image" />
                  </div>
                ) : (
                  <div className="preview-empty">
                    <div className="preview-empty-icon">{Icons.empty}</div>
                    <div className="preview-empty-title">{t("preview.emptyTitle")}</div>
                    <div className="preview-empty-desc">{t("preview.emptyDesc")}</div>
                    {scanners.length > 0 && (
                      <button className="btn btn-accent" onClick={startScan} style={{ marginTop: 8 }}>{Icons.scan} {t("actions.scan")}</button>
                    )}
                  </div>
                )}
              </div>
            ) : activeView === "ocr" ? (
              <div className="ocr-content" role="tabpanel">
                {ocrText ? (
                  <>
                    <div className="ocr-toolbar">
                      <button className="btn btn-sm" onClick={copyOcrText} title={t("ocr.copyTooltip")}>{Icons.copy}<span>{t("ocr.copy")}</span></button>
                      <button className="btn btn-sm" onClick={runOcr} disabled={isOcrRunning || !selectedDocument}>{Icons.refresh}<span>{t("ocr.rerun")}</span></button>
                    </div>
                    <div className="ocr-text-container"><pre className="ocr-text">{ocrText}</pre></div>
                  </>
                ) : (
                  <div className="preview-empty">
                    <div className="preview-empty-icon">{Icons.ocr}</div>
                    <div className="preview-empty-title">{t("ocr.emptyTitle")}</div>
                    <div className="preview-empty-desc">{t("ocr.emptyDesc")}</div>
                    {hasDocument && (
                      <button className="btn btn-accent" onClick={runOcr} disabled={isOcrRunning} style={{ marginTop: 8 }}>{Icons.ocr} {t("ocr.launch")}</button>
                    )}
                  </div>
                )}
              </div>
            ) : (
              <div role="tabpanel">
                <div className="history-search">
                  <div className="history-search-icon">{Icons.search}</div>
                  <input type="text" className="glass-input history-search-input" placeholder={t("history.searchPlaceholder")} value={searchQuery} onChange={(e) => handleSearch(e.target.value)} />
                </div>
                <div className="history-grid">
                  {(() => {
                    const displayDocs = searchQuery.trim() ? documents.filter((doc) => searchResults?.some((r) => r.id === doc.id) ?? false) : documents;
                    return displayDocs.length === 0 ? (
                      <div className="history-empty">
                        <div style={{ width: 48, height: 48, opacity: 0.3 }}>{Icons.folder}</div>
                        <p>{searchQuery.trim() ? t("history.noResults") : t("history.noDocuments")}</p>
                        <p className="history-empty-hint">{searchQuery.trim() ? t("history.noResultsHint") : t("history.noDocumentsHint")}</p>
                      </div>
                    ) : (
                      displayDocs.map((doc) => (
                        <button
                          key={doc.id}
                          className={`history-item ${selectedDocument?.id === doc.id ? "active" : ""}`}
                          onClick={() => { setSelectedDocument(doc); setActiveView("preview"); }}
                          onContextMenu={(e) => { e.preventDefault(); setContextMenu({ x: e.clientX, y: e.clientY, docId: doc.id }); }}
                          aria-pressed={selectedDocument?.id === doc.id}
                          aria-label={`${doc.name}, ${doc.date}`}
                        >
                          <div className="history-thumb">
                            <img src={doc.dataUrl} alt={doc.name} />
                            {searchResults?.find((r) => r.id === doc.id)?.has_ocr && (
                              <div className="ocr-badge" title={t("history.ocrBadge")}>T</div>
                            )}
                          </div>
                          <div className="history-meta">
                            {renamingDocId === doc.id ? (
                              <input className="glass-input history-rename-input" value={renameValue} onChange={(e) => setRenameValue(e.target.value)}
                                onBlur={() => { if (renameValue.trim()) renameDoc(doc.id, renameValue.trim()); else setRenamingDocId(null); }}
                                onKeyDown={(e) => { if (e.key === "Enter" && renameValue.trim()) renameDoc(doc.id, renameValue.trim()); if (e.key === "Escape") setRenamingDocId(null); }}
                                autoFocus onClick={(e) => e.stopPropagation()} />
                            ) : (
                              <div className="history-name">{doc.name}</div>
                            )}
                            <div className="history-date">{doc.date}</div>
                            <button className="history-delete" onClick={(e) => { e.stopPropagation(); deleteDocument(doc.id); }} aria-label={t("history.deleteTooltip")}>{Icons.delete}</button>
                          </div>
                        </button>
                      ))
                    );
                  })()}
                </div>
              </div>
            )}
          </div>

          {/* Right Panel */}
          <div className="config-panel">
            <div id="panel-tabs" className="panel-mode-tabs" role="tablist" aria-label={t("a11y.panelTabs")}>
              <button role="tab" aria-selected={rightPanelMode === "config"} className={`panel-mode-tab ${rightPanelMode === "config" ? "active" : ""}`} onClick={() => setRightPanelMode("config")}>{t("panels.config")}</button>
              <button role="tab" aria-selected={rightPanelMode === "edit"} className={`panel-mode-tab ${rightPanelMode === "edit" ? "active" : ""}`} onClick={() => setRightPanelMode("edit")} disabled={!hasDocument}>{t("panels.edit")}</button>
              <button role="tab" aria-selected={rightPanelMode === "intelligence"} className={`panel-mode-tab ${rightPanelMode === "intelligence" ? "active" : ""}`} onClick={() => setRightPanelMode("intelligence")} disabled={!hasDocument}>{t("panels.ai")}</button>
            </div>

            {rightPanelMode === "config" ? (
              <>
                <div className="config-header">{t("config.header")}</div>
                {scanProfiles.length > 0 && (
                  <div className="config-section">
                    <div className="config-label">{t("config.profiles")}</div>
                    <div className="chip-group">
                      {scanProfiles.map((p) => (
                        <button key={p.id} className={`chip ${selectedProfileId === p.id ? "active" : ""}`} onClick={() => applyProfile(p)} onContextMenu={(e) => { e.preventDefault(); deleteProfile(p.id); }}
                          title={t("config.profileTooltip", { dpi: p.dpi, mode: p.color_mode })}>
                          {Icons.profile} {p.name}
                        </button>
                      ))}
                    </div>
                  </div>
                )}
                <div className="config-section">
                  <button className="btn btn-sm" onClick={() => { const name = prompt(t("config.profileNamePrompt")); if (name?.trim()) saveCurrentAsProfile(name.trim()); }}>
                    {Icons.profile} {t("config.saveAsProfile")}
                  </button>
                </div>

                <div className="config-section">
                  <div className="config-label">{t("config.resolution")}</div>
                  <div className="chip-group">
                    {dpiOptions.map((dpi) => (
                      <button key={dpi} className={`chip ${config.dpi === dpi ? "active" : ""}`} onClick={() => setConfig((c) => ({ ...c, dpi }))}>{dpi}</button>
                    ))}
                  </div>
                </div>
                <div className="config-section">
                  <div className="config-label">{t("config.colorMode")}</div>
                  <div className="chip-group">
                    {colorOptions.map((mode) => (
                      <button key={mode} className={`chip ${config.colorMode === mode ? "active" : ""}`} onClick={() => setConfig((c) => ({ ...c, colorMode: mode }))}>{t(`colorModes.${mode}`) || mode}</button>
                    ))}
                  </div>
                </div>
                <div className="config-section">
                  <div className="config-label">{t("config.paperFormat")}</div>
                  <div className="select-wrapper">
                    <select className="glass-select" value={config.paperFormat} onChange={(e) => setConfig((c) => ({ ...c, paperFormat: e.target.value }))}>
                      <option value="A4">A4 (210 x 297 mm)</option>
                      <option value="A3">A3 (297 x 420 mm)</option>
                      <option value="Letter">Letter (216 x 279 mm)</option>
                      <option value="Legal">Legal (216 x 356 mm)</option>
                    </select>
                    <div className="select-arrow">{Icons.chevronDown}</div>
                  </div>
                </div>
                <div className="config-section">
                  <div className="config-label">{t("config.options")}</div>
                  <div className="toggle-row">
                    <span className="toggle-label">{t("config.duplex")}</span>
                    <input type="checkbox" className="toggle" checked={config.duplex} onChange={(e) => setConfig((c) => ({ ...c, duplex: e.target.checked }))} disabled={!currentScanner?.capabilities.supports_duplex} />
                  </div>
                  <div className="toggle-row">
                    <span className="toggle-label">{t("config.adf")}</span>
                    <input type="checkbox" className="toggle" checked={config.adf} onChange={(e) => setConfig((c) => ({ ...c, adf: e.target.checked }))} disabled={!currentScanner?.capabilities.supports_adf} />
                  </div>
                </div>
                <div className="config-section">
                  <div className="config-label">{t("config.batchScan")}</div>
                  <div className="toggle-row">
                    <span className="toggle-label">{t("config.batchMode")}</span>
                    <input type="checkbox" className="toggle" checked={batchMode} onChange={(e) => setBatchMode(e.target.checked)} />
                  </div>
                  {batchMode && (
                    <div style={{ marginTop: 8 }}>
                      <div className="adjustment-slider-header">
                        <span>{t("config.pageCount")}</span>
                        <span className="adjustment-value">{batchPageCount}</span>
                      </div>
                      <input type="range" className="glass-range" min={2} max={50} step={1} value={batchPageCount} onChange={(e) => setBatchPageCount(Number(e.target.value))} />
                      <button className="btn btn-sm btn-accent" onClick={startBatchScan} disabled={isScanning || !selectedScanner} style={{ marginTop: 8, width: "100%" }}>
                        {Icons.batch} {t("config.scanPages", { count: batchPageCount })}
                      </button>
                    </div>
                  )}
                </div>
              </>
            ) : rightPanelMode === "edit" ? (
              <>
                <div className="config-header">{t("edit.header")}</div>
                <div className="config-section">
                  <div className="config-label">{t("edit.rotationFlip")}</div>
                  <div className="edit-btn-row">
                    <button className="btn btn-sm" onClick={() => rotateDocument("270")} disabled={!hasDocument} title={t("edit.rotateLeft")}>{Icons.rotateLeft}</button>
                    <button className="btn btn-sm" onClick={() => rotateDocument("90")} disabled={!hasDocument} title={t("edit.rotateRight")}>{Icons.rotateRight}</button>
                    <button className="btn btn-sm" onClick={() => rotateDocument("180")} disabled={!hasDocument} title={t("edit.rotate180")}>180°</button>
                    <button className="btn btn-sm" onClick={() => flipDocument("horizontal")} disabled={!hasDocument} title={t("edit.flipH")}>{Icons.flipH}</button>
                    <button className="btn btn-sm" onClick={() => flipDocument("vertical")} disabled={!hasDocument} title={t("edit.flipV")}>{Icons.flipV}</button>
                  </div>
                </div>
                <div className="config-section">
                  <div className="config-label">{t("edit.adjustments")}</div>
                  {(["brightness", "contrast", "saturation", "sharpness"] as const).map((key) => (
                    <div className="adjustment-slider" key={key}>
                      <div className="adjustment-slider-header">
                        <label htmlFor={`slider-${key}`}>{t(`edit.${key}`)}</label>
                        <span className="adjustment-value" aria-hidden="true">{key === "sharpness" ? adjustments[key] : `${adjustments[key] > 0 ? "+" : ""}${adjustments[key]}`}</span>
                      </div>
                      <input id={`slider-${key}`} type="range" className="glass-range" min={key === "sharpness" ? 0 : -100} max={100} step={1} value={adjustments[key]}
                        onChange={(e) => handleAdjustmentChange(key, Number(e.target.value))} disabled={!hasDocument} aria-label={t(`edit.${key}`)} />
                    </div>
                  ))}
                  {hasAdjustments && (
                    <div className="edit-btn-row" style={{ marginTop: 8 }}>
                      <button className="btn btn-sm" onClick={revertAdjustments}>{Icons.undo} {t("edit.cancel")}</button>
                      <button className="btn btn-sm btn-accent" onClick={commitAdjustments}>{Icons.check} {t("edit.apply")}</button>
                    </div>
                  )}
                </div>
                <div className="config-section">
                  <div className="config-label">{t("edit.processing")}</div>
                  <div className="edit-action-list">
                    <button className="btn btn-sm edit-action-btn" onClick={deskewDocument} disabled={!hasDocument}>{Icons.deskew}<span>{t("edit.deskew")}</span></button>
                    <button className="btn btn-sm edit-action-btn" onClick={whitenBackground} disabled={!hasDocument}>{Icons.whiten}<span>{t("edit.whitenBg")}</span></button>
                    <button className="btn btn-sm edit-action-btn" onClick={() => denoiseDocument(1)} disabled={!hasDocument}>{Icons.noise}<span>{t("edit.denoiseLight")}</span></button>
                    <button className="btn btn-sm edit-action-btn" onClick={() => denoiseDocument(2)} disabled={!hasDocument}>{Icons.noise}<span>{t("edit.denoiseStrong")}</span></button>
                  </div>
                </div>
              </>
            ) : (
              <>
                <div className="config-header">{t("intelligence.header")}</div>
                {selectedDocument && (
                  <div className="config-section">
                    <div className="config-label">{Icons.tag} {t("intelligence.tags")}</div>
                    <div className="tags-container">
                      {(documentTags[selectedDocument.id] || []).map((tag) => {
                        const def = tagDefinitions.find((d) => d.name === tag);
                        return (
                          <span key={tag} className="tag-chip" style={{ background: def?.color || "var(--accent-color)" }}>
                            {tag}
                            <button className="tag-remove" onClick={() => removeTag(selectedDocument.id, tag)} aria-label={`Remove ${tag}`}>&times;</button>
                          </span>
                        );
                      })}
                      <select className="glass-select tag-add-select" value="" onChange={(e) => { if (e.target.value) addTag(selectedDocument.id, e.target.value); e.target.value = ""; }}>
                        <option value="">{t("intelligence.addTag")}</option>
                        {tagDefinitions.filter((d) => !(documentTags[selectedDocument.id] || []).includes(d.name)).map((d) => <option key={d.name} value={d.name}>{d.name}</option>)}
                      </select>
                    </div>
                  </div>
                )}
                <div className="config-section">
                  <div className="config-label">{Icons.brain} {t("intelligence.header")}</div>
                  <button className="btn btn-sm btn-accent" onClick={analyzeDocument} disabled={!hasDocument || isAnalyzing} style={{ width: "100%" }}>
                    {Icons.sparkle} {isAnalyzing ? t("intelligence.analyzing") : t("intelligence.analyze")}
                  </button>
                </div>
                {analysisResult && (
                  <>
                    <div className="config-section">
                      <div className="config-label">{t("intelligence.classification")}</div>
                      <div className="intelligence-result">
                        <div className="intelligence-type">{t(`docTypes.${analysisResult.classification.doc_type}`) || analysisResult.classification.doc_type}</div>
                        <div className="intelligence-confidence">{t("intelligence.confidence", { percent: Math.round(analysisResult.classification.confidence * 100) })}</div>
                        <div className="intelligence-scores">
                          {analysisResult.classification.scores.slice(0, 5).map(([name, score]) => (
                            <div key={name} className="score-bar">
                              <span className="score-label">{t(`docTypes.${name}`) || name}</span>
                              <div className="score-track"><div className="score-fill" style={{ width: `${Math.min(score * 3, 100)}%` }} /></div>
                              <span className="score-value">{score.toFixed(1)}</span>
                            </div>
                          ))}
                        </div>
                      </div>
                    </div>
                    {Object.keys(analysisResult.extracted_data.fields).length > 0 && (
                      <div className="config-section">
                        <div className="config-label">{t("intelligence.extractedData")}</div>
                        <div className="extracted-fields">
                          {Object.entries(analysisResult.extracted_data.fields).map(([key, values]) => (
                            <div key={key} className="extracted-field">
                              <span className="field-name">{key}</span>
                              <span className="field-values">{values.join(", ")}</span>
                            </div>
                          ))}
                        </div>
                      </div>
                    )}
                    <div className="config-section">
                      <div className="config-label">{Icons.sparkle} {t("intelligence.suggestion")}</div>
                      <div className="suggestion-card">
                        <div className="suggestion-row"><span className="suggestion-label">{t("intelligence.suggestedName")}</span><span className="suggestion-value">{analysisResult.suggestion.suggested_name}</span></div>
                        <div className="suggestion-row"><span className="suggestion-label">{t("intelligence.suggestedFolder")}</span><span className="suggestion-value">{analysisResult.suggestion.suggested_folder}</span></div>
                        <div className="suggestion-row"><span className="suggestion-label">{t("intelligence.suggestedTags")}</span><span className="suggestion-value">{analysisResult.suggestion.suggested_tags.join(", ")}</span></div>
                        <button className="btn btn-sm btn-accent" onClick={applySuggestion} style={{ marginTop: 8, width: "100%" }}>{Icons.check} {t("intelligence.applySuggestions")}</button>
                      </div>
                    </div>
                    {analysisResult.rule_results.length > 0 && (
                      <div className="config-section">
                        <div className="config-label">{Icons.rules} {t("intelligence.matchingRules")}</div>
                        {analysisResult.rule_results.map((rr, i) => (
                          <div key={i} className="rule-result">
                            <div className="rule-result-name">{rr.rule_name}</div>
                            <div className="rule-result-actions">
                              {rr.actions.map((a, j) => <span key={j} className="rule-action-chip">{a.action_type}: {a.value}</span>)}
                            </div>
                            <button className="btn btn-sm" style={{ marginTop: 4 }} onClick={async () => {
                              if (!selectedDocument) return;
                              await invoke("apply_rule_actions", { docId: selectedDocument.id, actions: rr.actions });
                              for (const action of rr.actions) {
                                if (action.action_type === "Rename") {
                                  setDocuments((docs) => docs.map((d) => d.id === selectedDocument.id ? { ...d, name: action.value } : d));
                                  setSelectedDocument((prev) => prev ? { ...prev, name: action.value } : prev);
                                }
                                if (action.action_type === "AddTag") {
                                  setDocumentTags((prev) => {
                                    const tags = [...(prev[selectedDocument.id] || [])];
                                    if (!tags.includes(action.value)) tags.push(action.value);
                                    return { ...prev, [selectedDocument.id]: tags };
                                  });
                                }
                              }
                              setStatusMessage(t("status.ruleApplied", { name: rr.rule_name }));
                              setStatusType("ready");
                            }}>{t("actions.apply")}</button>
                          </div>
                        ))}
                      </div>
                    )}
                  </>
                )}
              </>
            )}
          </div>
        </div>

        {/* Status Bar */}
        <div className="status-bar" role="status">
          <div className={`status-dot ${statusType}`} aria-hidden="true" />
          <span className="status-text" aria-live="polite" aria-atomic="true">{statusMessage}</span>
          {isAdjusting && <span className="status-text" style={{ opacity: 0.5 }}>{t("status.previewing")}</span>}
          <div className="status-spacer" />
          {(isScanning || scanProgress > 0) && (
            <>
              <div className="progress-bar-track" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(Math.min(scanProgress, 100))}>
                <div className="progress-bar-fill" style={{ width: `${Math.min(scanProgress, 100)}%` }} />
              </div>
              <span className="progress-text">{Math.round(Math.min(scanProgress, 100))}%</span>
            </>
          )}
        </div>
      </main>

      {/* Context Menu */}
      {contextMenu && (
        <div className="context-menu-overlay" role="presentation" onClick={() => setContextMenu(null)} onContextMenu={(e) => { e.preventDefault(); setContextMenu(null); }}>
          <div className="context-menu" role="menu" style={{ top: contextMenu.y, left: contextMenu.x }}>
            {contextMenu.pageIndex !== undefined ? (
              <>
                <button role="menuitem" className="context-menu-item" onClick={() => { rotateDocument("270", contextMenu.docId); setContextMenu(null); }}>
                  {Icons.rotateLeft} {t("contextMenu.rotateLeft")}
                </button>
                <button role="menuitem" className="context-menu-item" onClick={() => { rotateDocument("90", contextMenu.docId); setContextMenu(null); }}>
                  {Icons.rotateRight} {t("contextMenu.rotateRight")}
                </button>
                <button role="menuitem" className="context-menu-item" onClick={() => { rotateDocument("180", contextMenu.docId); setContextMenu(null); }}>
                  ↻ {t("contextMenu.rotate180")}
                </button>
                <div className="context-menu-divider" />
                <button role="menuitem" className="context-menu-item" onClick={() => { flipDocument("horizontal", contextMenu.docId); setContextMenu(null); }}>
                  {Icons.flipH} {t("contextMenu.flipH")}
                </button>
                <button role="menuitem" className="context-menu-item" onClick={() => { flipDocument("vertical", contextMenu.docId); setContextMenu(null); }}>
                  {Icons.flipV} {t("contextMenu.flipV")}
                </button>
                <div className="context-menu-divider" />
                <button role="menuitem" className="context-menu-item" onClick={() => { removePageFromMultipage(contextMenu.pageIndex!); setContextMenu(null); }}>
                  {Icons.close} {t("contextMenu.removeFromPages")}
                </button>
                <button role="menuitem" className="context-menu-item context-menu-danger" onClick={() => { deleteDocument(contextMenu.docId); setContextMenu(null); }}>
                  {Icons.delete} {t("contextMenu.delete")}
                </button>
              </>
            ) : (
              <>
                <button role="menuitem" className="context-menu-item" onClick={() => { setRenamingDocId(contextMenu.docId); setRenameValue(documents.find((d) => d.id === contextMenu.docId)?.name ?? ""); setContextMenu(null); setActiveView("history"); }}>
                  {Icons.rename} {t("contextMenu.rename")}
                </button>
                <button role="menuitem" className="context-menu-item" onClick={() => { duplicateDoc(contextMenu.docId); setContextMenu(null); }}>
                  {Icons.duplicate} {t("contextMenu.duplicate")}
                </button>
                <button role="menuitem" className="context-menu-item" onClick={() => { if (multipageDoc) addPageToMultipage(contextMenu.docId); setContextMenu(null); }} disabled={!multipageDoc}>
                  {Icons.pages} {t("contextMenu.addToPages")}
                </button>
                <div className="context-menu-divider" />
                <button role="menuitem" className="context-menu-item context-menu-danger" onClick={() => { deleteDocument(contextMenu.docId); setContextMenu(null); }}>
                  {Icons.delete} {t("contextMenu.delete")}
                </button>
              </>
            )}
          </div>
        </div>
      )}

      {/* Settings Modal */}
      {showSettings && (
        <div className="settings-overlay" onClick={(e) => e.target === e.currentTarget && setShowSettings(false)}>
          <div ref={settingsRef} className="settings-modal" role="dialog" aria-modal="true" aria-labelledby="settings-title">
            <div className="settings-header">
              <div className="settings-header-left">
                <img src="/logo.svg" alt="" className="settings-logo" />
                <div>
                  <span id="settings-title" className="settings-title">{t("settings.title")}</span>
                  <div className="settings-version">Photon v1.0.0</div>
                </div>
              </div>
              <button className="btn btn-icon btn-ghost" onClick={() => setShowSettings(false)} aria-label={t("a11y.close")}>{Icons.close}</button>
            </div>

            <div className="settings-tabs" role="tablist">
              {(["general", "scan", "export", "app"] as const).map((tab) => (
                <button key={tab} role="tab" aria-selected={settingsTab === tab} className={`settings-tab ${settingsTab === tab ? "active" : ""}`}
                  onClick={() => setSettingsTab(tab)}>{t(`settings.tab.${tab}`)}</button>
              ))}
            </div>

            <div className="settings-body">
              {settingsTab === "general" && (
                <>
                  <div className="settings-group">
                    <div className="settings-row-label" style={{ marginBottom: 6 }}>{t("settings.outputDir")}</div>
                    <div style={{ display: "flex", gap: 8 }}>
                      <input type="text" className="glass-input" value={settings.output_dir} onChange={(e) => setSettings((s) => ({ ...s, output_dir: e.target.value }))} placeholder={t("settings.outputDirPlaceholder")} />
                      <button className="btn btn-icon" onClick={selectOutputDir} aria-label={t("settings.browse")}>{Icons.folder}</button>
                    </div>
                  </div>
                  <div className="settings-group">
                    <div className="settings-row">
                      <div><div className="settings-row-label">{t("settings.defaultFormat")}</div><div className="settings-row-desc">{t("settings.defaultFormatDesc")}</div></div>
                      <select className="glass-select" style={{ width: 110 }} value={settings.default_format} onChange={(e) => setSettings((s) => ({ ...s, default_format: e.target.value }))}>
                        <option value="PDF">PDF</option><option value="PNG">PNG</option><option value="JPEG">JPEG</option><option value="TIFF">TIFF</option>
                      </select>
                    </div>
                  </div>
                  <div className="settings-group">
                    <div className="settings-row">
                      <div><div className="settings-row-label">{t("settings.autoCrop")}</div><div className="settings-row-desc">{t("settings.autoCropDesc")}</div></div>
                      <input type="checkbox" className="toggle" checked={settings.auto_crop} onChange={(e) => setSettings((s) => ({ ...s, auto_crop: e.target.checked }))} />
                    </div>
                  </div>
                </>
              )}

              {settingsTab === "scan" && (
                <>
                  <div className="settings-group">
                    <div className="settings-row">
                      <div className="settings-row-label">{t("settings.resolution")}</div>
                      <select className="glass-select" style={{ width: 110 }} value={settings.default_dpi} onChange={(e) => setSettings((s) => ({ ...s, default_dpi: Number(e.target.value) }))}>
                        <option value={150}>150 DPI</option><option value={300}>300 DPI</option><option value={600}>600 DPI</option><option value={1200}>1200 DPI</option>
                      </select>
                    </div>
                    <div className="settings-row">
                      <div className="settings-row-label">{t("settings.colorMode")}</div>
                      <select className="glass-select" style={{ width: 150 }} value={settings.default_color_mode} onChange={(e) => setSettings((s) => ({ ...s, default_color_mode: e.target.value }))}>
                        <option value="Couleur">{t("colorModes.Couleur")}</option><option value="Niveaux de gris">{t("colorModes.Niveaux de gris")}</option><option value="Noir et blanc">{t("colorModes.Noir et blanc")}</option>
                      </select>
                    </div>
                    <div className="settings-row">
                      <div className="settings-row-label">{t("settings.paperFormat")}</div>
                      <select className="glass-select" style={{ width: 110 }} value={settings.default_paper_format} onChange={(e) => setSettings((s) => ({ ...s, default_paper_format: e.target.value }))}>
                        <option value="A4">A4</option><option value="A3">A3</option><option value="Letter">Letter</option><option value="Legal">Legal</option>
                      </select>
                    </div>
                  </div>
                  <div className="settings-group">
                    <div className="settings-row" style={{ marginBottom: 8 }}>
                      <div className="settings-row-label">{t("settings.quality")}</div>
                      <span className="range-value">{settings.quality}%</span>
                    </div>
                    <div className="range-wrapper">
                      <input type="range" className="glass-range" min={10} max={100} step={5} value={settings.quality} onChange={(e) => setSettings((s) => ({ ...s, quality: Number(e.target.value) }))} />
                    </div>
                  </div>
                  <div className="settings-group">
                    <div className="settings-row">
                      <div><div className="settings-row-label">{t("settings.autoOcr")}</div><div className="settings-row-desc">{t("settings.autoOcrDesc")}</div></div>
                      <input type="checkbox" className="toggle" checked={settings.auto_ocr} onChange={(e) => setSettings((s) => ({ ...s, auto_ocr: e.target.checked }))} />
                    </div>
                    <div className="settings-row">
                      <div className="settings-row-label">{t("settings.ocrLanguage")}</div>
                      <select className="glass-select" style={{ width: 150 }} value={settings.default_ocr_lang} onChange={(e) => setSettings((s) => ({ ...s, default_ocr_lang: e.target.value }))}>
                        <option value="fra">Français</option><option value="eng">English</option><option value="deu">Deutsch</option><option value="spa">Español</option>
                        <option value="ita">Italiano</option><option value="por">Português</option><option value="nld">Nederlands</option><option value="fra+eng">Français + English</option>
                      </select>
                    </div>
                  </div>
                </>
              )}

              {settingsTab === "export" && (
                <>
                  <div className="settings-group">
                    <div className="settings-row-label" style={{ marginBottom: 4 }}>{t("settings.namingTemplate")}</div>
                    <div className="settings-row-desc" style={{ marginBottom: 8 }}>{t("settings.namingTemplateDesc")}</div>
                    <input type="text" className="glass-input" value={settings.naming_template} onChange={(e) => setSettings((s) => ({ ...s, naming_template: e.target.value }))} placeholder="Scan_{date}_{time}" />
                  </div>
                  <div className="settings-group">
                    <div className="settings-row-label" style={{ marginBottom: 4 }}>{t("settings.watchFolder")}</div>
                    <div className="settings-row-desc" style={{ marginBottom: 8 }}>{t("settings.watchFolderDesc")}</div>
                    <div style={{ display: "flex", gap: 8 }}>
                      <input type="text" className="glass-input" value={settings.watch_folder ?? ""} onChange={(e) => setSettings((s) => ({ ...s, watch_folder: e.target.value || null }))} placeholder={t("settings.watchFolderPlaceholder")} />
                      <button className="btn btn-icon" onClick={async () => {
                        try {
                          const dir = await selectDirectory();
                          if (dir) setSettings((s) => ({ ...s, watch_folder: dir }));
                        } catch { /* Dialog not available */ }
                      }} aria-label={t("settings.browse")}>{Icons.folder}</button>
                    </div>
                  </div>
                </>
              )}

              {settingsTab === "app" && (
                <>
                  <div className="settings-group">
                    <div className="settings-row">
                      <div className="settings-row-label">{t("settings.language")}</div>
                      <select className="glass-select" style={{ width: 150 }} value={language} onChange={(e) => setLanguage(e.target.value as Language)}>
                        <option value="fr">Français</option>
                        <option value="en">English</option>
                      </select>
                    </div>
                  </div>
                </>
              )}
            </div>

            <div className="settings-footer">
              <button className="btn" onClick={() => setShowSettings(false)}>{t("settings.cancel")}</button>
              <button className="btn btn-accent" onClick={handleSaveSettings}>{t("settings.save")}</button>
            </div>
          </div>
        </div>
      )}

      {/* Rules Modal */}
      {showRulesModal && (
        <div className="settings-overlay" onClick={(e) => e.target === e.currentTarget && setShowRulesModal(false)}>
          <div ref={rulesRef} className="settings-modal" role="dialog" aria-modal="true" aria-labelledby="rules-title">
            <div className="settings-header">
              <span id="rules-title" className="settings-title">{t("rules.title")}</span>
              <button className="btn btn-icon btn-ghost" onClick={() => setShowRulesModal(false)} aria-label={t("a11y.close")}>{Icons.close}</button>
            </div>
            <div className="settings-body">
              {editingRule ? (
                <RuleEditor key={editingRule.id} rule={editingRule} onSave={(r) => saveRule(r)} onCancel={() => setEditingRule(null)} />
              ) : (
                <>
                  <button className="btn btn-sm btn-accent" onClick={() => setEditingRule({
                    id: crypto.randomUUID(), name: t("rules.newRuleDefault"), enabled: true, condition_logic: "And",
                    conditions: [{ field: "DocumentType", operator: "Equals", value: "Facture" }],
                    actions: [{ action_type: "AddTag", value: "Facture" }],
                  })} style={{ marginBottom: 12 }}>
                    {Icons.plus} {t("rules.newRule")}
                  </button>
                  {automationRules.length === 0 ? (
                    <div style={{ textAlign: "center", opacity: 0.5, padding: 20 }}>{t("rules.noRules")}</div>
                  ) : (
                    automationRules.map((rule) => (
                      <div key={rule.id} className="rule-card">
                        <div className="rule-card-header">
                          <input type="checkbox" className="toggle" checked={rule.enabled} onChange={(e) => saveRule({ ...rule, enabled: e.target.checked })} />
                          <span className="rule-card-name">{rule.name}</span>
                          <div style={{ flex: 1 }} />
                          <button className="btn btn-icon btn-sm btn-ghost" onClick={() => setEditingRule(rule)} aria-label={t("rules.edit")}>{Icons.rename}</button>
                          <button className="btn btn-icon btn-sm btn-ghost" onClick={() => deleteRule(rule.id)} aria-label={t("rules.delete")}>{Icons.delete}</button>
                        </div>
                        <div className="rule-card-detail">
                          <span className="rule-logic">{rule.condition_logic === "And" ? t("rules.logicAnd") : t("rules.logicOr")}</span>
                          {rule.conditions.map((c, i) => <span key={i} className="rule-cond-chip">{c.field} {c.operator} "{c.value}"</span>)}
                          <span style={{ opacity: 0.5 }}>&rarr;</span>
                          {rule.actions.map((a, i) => <span key={i} className="rule-action-chip">{a.action_type}: {a.value}</span>)}
                        </div>
                      </div>
                    ))
                  )}
                </>
              )}
            </div>
            <div className="settings-footer">
              <button className="btn" onClick={() => { setEditingRule(null); setShowRulesModal(false); }}>{t("rules.close")}</button>
            </div>
          </div>
        </div>
      )}

      {/* Onboarding Wizard */}
      {showOnboarding && <OnboardingWizard onComplete={handleOnboardingComplete} />}

      {/* Guided Tour */}
      {tour.isActive && tour.currentStep && (
        <TourTooltip step={tour.currentStep} stepIndex={tour.stepIndex} totalSteps={tour.totalSteps} onNext={tour.next} onPrev={tour.prev} onSkip={tour.skip} />
      )}

      {/* Update Toast */}
      {updateAvailable && (
        <div className="update-toast" role="alert">
          <div>
            <div className="update-toast-text">{t("update.available")}</div>
            <div className="update-toast-version">v{updateAvailable.version}</div>
          </div>
          <button className="btn btn-sm btn-accent" onClick={installUpdate} disabled={isUpdating}>
            {isUpdating ? t("update.installing") : t("update.install")}
          </button>
          <button className="btn btn-icon btn-ghost btn-sm" onClick={() => setUpdateAvailable(null)} aria-label={t("a11y.close")}>&times;</button>
        </div>
      )}
    </div>
  );
}

// ─── Rule Editor Component ───────────────────────────────────────

function RuleEditor({
  rule, onSave, onCancel,
}: {
  rule: AutomationRule;
  onSave: (rule: AutomationRule) => void;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState<AutomationRule>(rule);
  const conditionFields = ["DocumentType", "Tag", "TextContains", "AmountAbove", "AmountBelow", "HasField"];
  const conditionOperators = ["Equals", "NotEquals", "Contains", "Regex", "GreaterThan", "LessThan"];
  const actionTypes = ["Rename", "MoveToFolder", "AddTag", "ApplyProfile"];

  const addCondition = () => setDraft((d) => ({ ...d, conditions: [...d.conditions, { field: "DocumentType", operator: "Equals", value: "" }] }));
  const removeCondition = (i: number) => setDraft((d) => ({ ...d, conditions: d.conditions.filter((_, idx) => idx !== i) }));
  const updateCondition = (i: number, key: keyof RuleCondition, value: string) => setDraft((d) => ({ ...d, conditions: d.conditions.map((c, idx) => idx === i ? { ...c, [key]: value } : c) }));
  const addAction = () => setDraft((d) => ({ ...d, actions: [...d.actions, { action_type: "AddTag", value: "" }] }));
  const removeAction = (i: number) => setDraft((d) => ({ ...d, actions: d.actions.filter((_, idx) => idx !== i) }));
  const updateAction = (i: number, key: keyof RuleAction, value: string) => setDraft((d) => ({ ...d, actions: d.actions.map((a, idx) => idx === i ? { ...a, [key]: value } : a) }));

  return (
    <div className="rule-editor">
      <div className="rule-editor-section">
        <div className="rule-editor-label">{t("rules.ruleName")}</div>
        <input className="glass-input" value={draft.name} onChange={(e) => setDraft((d) => ({ ...d, name: e.target.value }))} />
      </div>
      <div className="rule-editor-section">
        <div className="rule-editor-label">{t("rules.logic")}</div>
        <div className="chip-group">
          <button className={`chip ${draft.condition_logic === "And" ? "active" : ""}`} onClick={() => setDraft((d) => ({ ...d, condition_logic: "And" }))}>{t("rules.logicAnd")}</button>
          <button className={`chip ${draft.condition_logic === "Or" ? "active" : ""}`} onClick={() => setDraft((d) => ({ ...d, condition_logic: "Or" }))}>{t("rules.logicOr")}</button>
        </div>
      </div>
      <div className="rule-editor-section">
        <div className="rule-editor-label">{t("rules.conditions")}</div>
        <div className="rule-rows">
          {draft.conditions.map((c, i) => (
            <div key={i} className="rule-row">
              <select className="glass-select" value={c.field} onChange={(e) => updateCondition(i, "field", e.target.value)}>
                {conditionFields.map((f) => <option key={f} value={f}>{f}</option>)}
              </select>
              <select className="glass-select" value={c.operator} onChange={(e) => updateCondition(i, "operator", e.target.value)}>
                {conditionOperators.map((o) => <option key={o} value={o}>{o}</option>)}
              </select>
              <input className="glass-input" value={c.value} onChange={(e) => updateCondition(i, "value", e.target.value)} placeholder={t("rules.valuePlaceholder")} />
              <button className="btn btn-icon btn-sm btn-ghost" onClick={() => removeCondition(i)} aria-label="Remove">{Icons.close}</button>
            </div>
          ))}
        </div>
        <button className="btn btn-sm" onClick={addCondition} style={{ marginTop: 8 }}>{Icons.plus} {t("rules.addCondition")}</button>
      </div>
      <div className="rule-editor-section">
        <div className="rule-editor-label">{t("rules.actions")}</div>
        <div className="rule-rows">
          {draft.actions.map((a, i) => (
            <div key={i} className="rule-row">
              <select className="glass-select" value={a.action_type} onChange={(e) => updateAction(i, "action_type", e.target.value)}>
                {actionTypes.map((at) => <option key={at} value={at}>{at}</option>)}
              </select>
              <input className="glass-input" value={a.value} onChange={(e) => updateAction(i, "value", e.target.value)} placeholder={t("rules.valuePlaceholder")} />
              <button className="btn btn-icon btn-sm btn-ghost" onClick={() => removeAction(i)} aria-label="Remove">{Icons.close}</button>
            </div>
          ))}
        </div>
        <button className="btn btn-sm" onClick={addAction} style={{ marginTop: 8 }}>{Icons.plus} {t("rules.addAction")}</button>
      </div>
      <div className="rule-editor-actions">
        <button className="btn btn-sm" onClick={onCancel}>{t("edit.cancel")}</button>
        <button className="btn btn-sm btn-accent" onClick={() => onSave(draft)}>{t("rules.save")}</button>
      </div>
    </div>
  );
}

export default App;
