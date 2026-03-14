import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { save } from "@tauri-apps/plugin-dialog";
import { arrayMove } from "@dnd-kit/sortable";
import type { DragEndEvent } from "@dnd-kit/core";
import { useTranslation, type Language } from "./contexts/LanguageContext";
import { OnboardingWizard } from "./components/onboarding/OnboardingWizard";
import { TourTooltip } from "./components/onboarding/TourTooltip";
import { useTour } from "./components/onboarding/useTour";
import { selectDirectory } from "./utils/selectDirectory";
import { logger } from "./utils/debugLogger";
import "./App.css";

// ─── Components ──────────────────────────────────────────────────
import { Sidebar } from "./components/Sidebar";
import { ActionBar } from "./components/ActionBar";
import { StatusBar } from "./components/StatusBar";
import { Preview } from "./components/Preview";
import { ConfigPanel } from "./components/ConfigPanel";
import { EditPanel } from "./components/EditPanel";
import { HistoryGrid } from "./components/HistoryGrid";
import { OcrPanel } from "./components/OcrPanel";
import { MultipagePanel } from "./components/MultipagePanel";
import { IntelligencePanel } from "./components/IntelligencePanel";
import { ContextMenu } from "./components/ContextMenu";
import { DropOverlay } from "./components/DropOverlay";
import Icons from "./components/Icons";

// ─── Modals ──────────────────────────────────────────────────────
import { VaultModal } from "./components/modals/VaultModal";
import { SettingsModal } from "./components/modals/SettingsModal";
import { ExportDialog } from "./components/modals/ExportDialog";
import { RulesModal } from "./components/modals/RulesModal";
import { StatsModal } from "./components/modals/StatsModal";
import { WatermarkDialog } from "./components/modals/WatermarkDialog";
import { SignatureDialog } from "./components/modals/SignatureDialog";
import { RedactionModal } from "./components/modals/RedactionModal";
import { DebugLogPanel } from "./components/DebugLogPanel";

// ─── Types ───────────────────────────────────────────────────────
import type {
  ThemeMode,
  ScannerDevice,
  ScannedDocument,
  ScanConfig,
  AppSettings,
  ScanProfile,
  HistoryEntryDto,
  ImageAdjustments,
  AdjustmentPreviewResult,
  MultiPageDocDto,
  AnalysisResultDto,
  TagDefinition,
  AutomationRule,
  RuleAction,
  WatermarkPosition,
  WatermarkConfig,
  SignatureConfig,
  PdfExportOptions,
  PdfSaveResult,
  AnnotationTypeName,
  AnnotationData,
  PageAnnotationsData,
  VaultDocument,
  SemanticResult,
  SensitiveInfo,
  DetectedTable,
  AppStats,
  ScanResultDto,
} from "./types";

import {
  applyTheme,
  loadThemePreference,
  saveThemePreference,
  extractError,
  dtoToDoc,
} from "./types";

// ─── App ─────────────────────────────────────────────────────────
function App() {
  const { t, language, setLanguage } = useTranslation();
  const tour = useTour();

  // ── Core state ──
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

  // Documents
  const [documents, setDocuments] = useState<ScannedDocument[]>([]);
  const [selectedDocument, setSelectedDocument] = useState<ScannedDocument | null>(null);
  const [activeView, setActiveView] = useState<"preview" | "history" | "ocr">("preview");
  const [zoomLevel, setZoomLevel] = useState(100);

  // OCR
  const [ocrText, setOcrText] = useState<string | null>(null);
  const [isOcrRunning, setIsOcrRunning] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<HistoryEntryDto[] | null>(null);

  // Settings
  const [showSettings, setShowSettings] = useState(false);

  // Right panel mode
  const [rightPanelMode, setRightPanelMode] = useState<"config" | "edit" | "intelligence">("config");

  // Image adjustments
  const [adjustments, setAdjustments] = useState<ImageAdjustments>({ brightness: 0, contrast: 0, saturation: 0, sharpness: 0 });
  const [isAdjusting, setIsAdjusting] = useState(false);
  const adjustmentTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Multi-page
  const [multipageDoc, setMultipageDoc] = useState<MultiPageDocDto | null>(null);

  // Profiles
  const [scanProfiles, setScanProfiles] = useState<ScanProfile[]>([]);
  const [selectedProfileId, setSelectedProfileId] = useState<string | null>(null);

  // Batch scanning
  const [batchMode, setBatchMode] = useState(false);
  const [batchPageCount, setBatchPageCount] = useState(5);

  // File drag-and-drop import
  const [isDragOver, setIsDragOver] = useState(false);
  const [isImporting, setIsImporting] = useState(false);

  // Intelligence
  const [analysisResult, setAnalysisResult] = useState<AnalysisResultDto | null>(null);
  const [isAnalyzing, setIsAnalyzing] = useState(false);
  const [tagDefinitions, setTagDefinitions] = useState<TagDefinition[]>([]);
  const [documentTags, setDocumentTags] = useState<Record<string, string[]>>({});
  const [automationRules, setAutomationRules] = useState<AutomationRule[]>([]);
  const [showRulesModal, setShowRulesModal] = useState(false);
  const [editingRule, setEditingRule] = useState<AutomationRule | null>(null);

  // Context menu
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; docId: string; pageIndex?: number; isPreview?: boolean } | null>(null);
  const [renamingDocId, setRenamingDocId] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");

  // Scan config
  const [config, setConfig] = useState<ScanConfig>({
    dpi: 300,
    colorMode: "Couleur",
    paperFormat: "A4",
    duplex: false,
    adf: false,
  });

  // App settings
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

  // v1.0: Annotations
  const [annotations, setAnnotations] = useState<Record<string, AnnotationData[]>>({});
  const [annotationTool, setAnnotationTool] = useState<AnnotationTypeName | null>(null);
  const [isDrawing, setIsDrawing] = useState(false);
  const [drawStart, setDrawStart] = useState<{ x: number; y: number } | null>(null);
  const [currentDrawPos, setCurrentDrawPos] = useState<{ x: number; y: number } | null>(null);

  // v1.0: Signature
  const [signatureImage, setSignatureImage] = useState<string | null>(null);
  const [signaturePlacement, setSignaturePlacement] = useState<{ x: number; y: number; w: number; h: number } | null>(null);
  const [isPlacingSignature, setIsPlacingSignature] = useState(false);
  const [sigDragStart, setSigDragStart] = useState<{ x: number; y: number } | null>(null);
  const [sigDragCurrent, setSigDragCurrent] = useState<{ x: number; y: number } | null>(null);
  const [showSignatureDialog, setShowSignatureDialog] = useState(false);
  const [isDrawingSig, setIsDrawingSig] = useState(false);
  const sigCanvasRef = useRef<HTMLCanvasElement | null>(null);

  // v1.0: Export
  const [showExportDialog, setShowExportDialog] = useState(false);
  const [exportPdfa, setExportPdfa] = useState<"none" | "a1b" | "a2b">("none");
  const [exportUserPassword, setExportUserPassword] = useState("");
  const [exportOwnerPassword, setExportOwnerPassword] = useState("");
  const [lastExportHash, setLastExportHash] = useState<string | null>(null);

  // v1.0: Watermark
  const [showWatermarkDialog, setShowWatermarkDialog] = useState(false);
  const [exportWatermarkEnabled, setExportWatermarkEnabled] = useState(false);
  const [exportWatermarkText, setExportWatermarkText] = useState("CONFIDENTIEL");
  const [exportWatermarkOpacity, setExportWatermarkOpacity] = useState(0.3);
  const [exportWatermarkRotation, setExportWatermarkRotation] = useState(-45);
  const [exportWatermarkFontSize, setExportWatermarkFontSize] = useState(48);
  const [exportWatermarkColor, setExportWatermarkColor] = useState("#888888");
  const [exportWatermarkPosition, setExportWatermarkPosition] = useState<WatermarkPosition>("Diagonal");

  // v1.0: Vault
  const [showVault, setShowVault] = useState(false);
  const [vaultUnlocked, setVaultUnlocked] = useState(false);
  const [vaultSetup, setVaultSetup] = useState(false);
  const [vaultPassword, setVaultPassword] = useState("");
  const [vaultDocs, setVaultDocs] = useState<VaultDocument[]>([]);
  const [vaultViewMode, setVaultViewMode] = useState<"grid" | "list">("grid");
  const [vaultFilter, setVaultFilter] = useState("");

  // v1.0: Stats
  const [showStats, setShowStats] = useState(false);
  const [appStats, setAppStats] = useState<AppStats | null>(null);

  // v1.0: Redaction
  const [showRedaction, setShowRedaction] = useState(false);
  const [sensitiveItems, setSensitiveItems] = useState<SensitiveInfo[]>([]);

  // Debug panel
  const [showDebugPanel, setShowDebugPanel] = useState(false);

  // v1.0: AI features
  const [aiSummary, setAiSummary] = useState("");
  const [aiTranslation, setAiTranslation] = useState("");
  const [aiTargetLang, setAiTargetLang] = useState("en");
  const [isAiLoading, setIsAiLoading] = useState(false);
  const [semanticQuery, setSemanticQuery] = useState("");
  const [semanticResults, setSemanticResults] = useState<SemanticResult[]>([]);
  const [detectedTables, setDetectedTables] = useState<DetectedTable[]>([]);

  // ── Refs ──
  const selectedDocIdRef = useRef<string | null>(null);
  useEffect(() => {
    selectedDocIdRef.current = selectedDocument?.id ?? null;
    setAnalysisResult(null);
  }, [selectedDocument?.id]);

  // ── Theme ──
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

  // ── Keyboard shortcuts ──
  const shortcutRef = useRef({ selectedDocument, selectedScanner, isScanning, saveAsPdf: () => {}, saveAsImage: () => {}, startScan: () => {}, printDoc: () => {}, runOcr: () => {}, setRightPanelMode });
  shortcutRef.current = { selectedDocument, selectedScanner, isScanning, saveAsPdf: () => saveAsPdf(), saveAsImage: () => saveAsImage(), startScan: () => startScan(), printDoc: () => printDoc(), runOcr: () => runOcr(), setRightPanelMode };
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // F12 toggles debug panel
      if (e.key === "F12") {
        e.preventDefault();
        setShowDebugPanel((v) => !v);
        return;
      }
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
    logger.info("Scanners", "Recherche de scanners...");
    try {
      const list = await invoke<ScannerDevice[]>("list_scanners");
      setScanners(list);
      if (list.length > 0 && !selectedScanner) setSelectedScanner(list[0].id);
      logger.info("Scanners", `${list.length} scanner(s) détecté(s)`, list.map((s) => `${s.vendor} ${s.name} (${s.id})`).join(", "));
      setStatusMessage(list.length > 0 ? t("status.scannersFound", { count: list.length }) : t("status.noScanners"));
      setStatusType("ready");
    } catch (err) {
      setScanners([]);
      logger.error("Scanners", "Échec de la recherche de scanners", extractError(err));
      setStatusMessage(t("status.error", { error: extractError(err) }));
      setStatusType("error");
    } finally {
      setIsRefreshing(false);
    }
  }, [selectedScanner]);

  // ── Initial load ──
  useEffect(() => {
    logger.info("App", "Initialisation de l'application...");
    loadScanners();
    invoke<AppSettings>("load_settings")
      .then((s) => {
        setSettings(s);
        setConfig((c) => ({ ...c, dpi: s.default_dpi, colorMode: s.default_color_mode, paperFormat: s.default_paper_format }));
        if (s.language) setLanguage(s.language as Language);
        if (!s.onboarding_complete) setShowOnboarding(true);
        logger.info("App", "Paramètres chargés", `Format: ${s.default_format}, DPI: ${s.default_dpi}, Langue: ${s.language}`);
      })
      .catch((err) => { logger.error("App", "Échec du chargement des paramètres", extractError(err)); });
    invoke<string>("get_documents_dir")
      .then((dir) => setSettings((s) => ({ ...s, output_dir: dir })))
      .catch((err) => { logger.warn("App", "Impossible de récupérer le dossier documents", extractError(err)); });
    invoke<ScanProfile[]>("list_scan_profiles").then(setScanProfiles).catch((err) => { logger.warn("App", "Échec chargement profils", extractError(err)); });
    invoke<TagDefinition[]>("get_tag_definitions").then(setTagDefinitions).catch((err) => { logger.warn("App", "Échec chargement tags", extractError(err)); });
    invoke<Record<string, string[]>>("get_all_tags_map").then(setDocumentTags).catch((err) => { logger.warn("App", "Échec chargement map tags", extractError(err)); });
    invoke<AutomationRule[]>("list_automation_rules").then(setAutomationRules).catch((err) => { logger.warn("App", "Échec chargement règles", extractError(err)); });
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

  // ── File drag-and-drop import ──
  const importFiles = useCallback(async (paths: string[]) => {
    const supported = [".pdf", ".tif", ".tiff", ".png", ".jpg", ".jpeg", ".bmp", ".webp"];
    const validPaths = paths.filter((p) => supported.some((ext) => p.toLowerCase().endsWith(ext)));
    if (validPaths.length === 0) return;

    setIsImporting(true);
    setStatusMessage(t("status.importing"));
    setStatusType("scanning");
    logger.info("Import", `Importation de ${validPaths.length} fichier(s)`, validPaths.join(", "));

    const allNewDocs: ScannedDocument[] = [];

    for (const filePath of validPaths) {
      try {
        const results = await invoke<ScanResultDto[]>("import_file", { filePath });
        const newDocs: ScannedDocument[] = results.map((r) => dtoToDoc(r));
        setDocuments((prev) => [...prev, ...newDocs]);
        allNewDocs.push(...newDocs);
        logger.debug("Import", `Importé: ${filePath.split(/[/\\]/).pop()} (${results.length} page(s))`);
      } catch (err) {
        logger.error("Import", `Échec import: ${filePath.split(/[/\\]/).pop()}`, extractError(err));
        setStatusMessage(t("status.importError", { error: extractError(err) }));
        setStatusType("error");
      }
    }

    if (allNewDocs.length > 0) {
      setSelectedDocument(allNewDocs[0]);

      let mpDoc = multipageDoc;
      if (!mpDoc) {
        try {
          const name = `Import_${new Date().toISOString().slice(0, 10)}`;
          mpDoc = await invoke<MultiPageDocDto>("create_multipage_document", { name });
        } catch { /* ignore */ }
      }

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
    logger.info("Scan", `Numérisation lancée`, `Scanner: ${selectedScanner}, DPI: ${config.dpi}, Mode: ${config.colorMode}, Format: ${config.paperFormat}`);
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
      logger.info("Scan", `Numérisation terminée: ${result.name}`, `ID: ${result.id}, Dimensions: ${result.width}x${result.height}`);
      setStatusMessage(t("status.scanComplete"));
      setStatusType("ready");
    } catch (err) {
      clearInterval(progressInterval);
      setScanProgress(0);
      logger.error("Scan", "Échec de la numérisation", extractError(err));
      setStatusMessage(t("status.error", { error: extractError(err) }));
      setStatusType("error");
    } finally {
      setTimeout(() => { setIsScanning(false); setScanProgress(0); }, 600);
    }
  };

  // ── Save as PDF ──
  const saveAsPdf = async () => {
    if (multipageDoc && multipageDoc.page_count > 0) {
      return saveMultipagePdf();
    }
    if (!selectedDocument) return;
    try {
      const path = await save({ defaultPath: selectedDocument.name.replace(/\.\w+$/, ".pdf"), filters: [{ name: "PDF", extensions: ["pdf"] }] });
      if (!path) return;
      setStatusMessage(t("status.savingPdf"));
      setStatusType("scanning");
      logger.info("Export", `Sauvegarde PDF: ${path}`);
      await invoke<string>("save_document_as_pdf", { docId: selectedDocument.id, outputPath: path });
      logger.info("Export", `PDF sauvegardé avec succès: ${path.split(/[/\\]/).pop()}`);
      setStatusMessage(t("status.pdfSaved", { filename: path.split(/[/\\]/).pop() ?? "" }));
      setStatusType("ready");
    } catch (err) {
      logger.error("Export", "Échec sauvegarde PDF", extractError(err));
      setStatusMessage(t("status.pdfError", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  // ── Save as Image ──
  const saveAsImage = async () => {
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
        logger.info("Export", `Sauvegarde images multi-pages (${multipageDoc.page_count} pages)`, `Format: ${fmt}, Chemin: ${path}`);
        const detectedFormat = path.split(".").pop()?.toUpperCase() || "PNG";
        const dir = path.replace(/[/\\][^/\\]+$/, "");
        for (let i = 0; i < multipageDoc.page_ids.length; i++) {
          const pageId = multipageDoc.page_ids[i];
          const pagePath = i === 0 ? path : `${dir}/${baseName}_page${i + 1}.${ext}`;
          await invoke<string>("save_document_as_image", { docId: pageId, outputPath: pagePath, format: detectedFormat, quality: settings.quality });
        }
        logger.info("Export", `${multipageDoc.page_count} images sauvegardées`);
        setStatusMessage(t("status.imageSaved", { filename: `${multipageDoc.page_count} pages` }));
        setStatusType("ready");
      } catch (err) {
        logger.error("Export", "Échec sauvegarde images multi-pages", extractError(err));
        setStatusMessage(t("status.saveError", { error: extractError(err) }));
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
      logger.info("Export", `Sauvegarde image: ${path}`, `Format: ${detectedFormat}, Qualité: ${settings.quality}`);
      await invoke<string>("save_document_as_image", { docId: selectedDocument.id, outputPath: path, format: detectedFormat, quality: settings.quality });
      logger.info("Export", `Image sauvegardée: ${path.split(/[/\\]/).pop()}`);
      setStatusMessage(t("status.imageSaved", { filename: path.split(/[/\\]/).pop() ?? "" }));
      setStatusType("ready");
    } catch (err) {
      logger.error("Export", "Échec sauvegarde image", extractError(err));
      setStatusMessage(t("status.saveError", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  // ── Print ──
  const printDoc = async () => {
    if (multipageDoc && multipageDoc.page_count > 0) {
      try {
        setStatusMessage(t("status.printing"));
        setStatusType("scanning");
        logger.info("Print", `Impression multi-pages (${multipageDoc.page_count} pages)`);
        await invoke("print_multipage_document", { multipageId: multipageDoc.id });
        logger.info("Print", "Impression multi-pages envoyée");
        setStatusMessage(t("status.printed"));
        setStatusType("ready");
      } catch (err) {
        logger.error("Print", "Échec impression multi-pages", extractError(err));
        setStatusMessage(t("status.printError", { error: extractError(err) }));
        setStatusType("error");
      }
      return;
    }
    if (!selectedDocument) return;
    try {
      setStatusMessage(t("status.printing"));
      logger.info("Print", `Impression: ${selectedDocument.name}`);
      await invoke("print_document", { docId: selectedDocument.id });
      logger.info("Print", "Impression envoyée");
      setStatusMessage(t("status.printed"));
      setStatusType("ready");
    } catch (err) {
      logger.error("Print", "Échec impression", extractError(err));
      setStatusMessage(t("status.printError", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  // ── Auto-crop ──
  const autoCrop = async () => {
    if (!selectedDocument) return;
    try {
      setStatusMessage(t("status.cropping"));
      setStatusType("scanning");
      logger.info("Processing", `Recadrage auto: ${selectedDocument.name}`);
      const result = await invoke<ScanResultDto>("auto_crop_document", { docId: selectedDocument.id });
      const updated = dtoToDoc(result);
      setSelectedDocument(updated);
      setDocuments((docs) => docs.map((d) => (d.id === updated.id ? updated : d)));
      logger.info("Processing", `Recadrage terminé: ${result.width}x${result.height}`);
      setStatusMessage(t("status.cropComplete"));
      setStatusType("ready");
    } catch (err) {
      logger.error("Processing", "Échec recadrage auto", extractError(err));
      setStatusMessage(t("status.cropError", { error: extractError(err) }));
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
    logger.info("OCR", `OCR lancé: ${selectedDocument.name}`, `Langue: ${settings.default_ocr_lang}`);
    try {
      const text = await invoke<string>("run_ocr", { docId: selectedDocument.id, lang: settings.default_ocr_lang });
      setOcrText(text);
      setActiveView("ocr");
      logger.info("OCR", `OCR terminé: ${text.length} caractères extraits`);
      setStatusMessage(t("status.ocrComplete"));
      setStatusType("ready");
    } catch (err) {
      logger.error("OCR", "Échec OCR", extractError(err));
      setStatusMessage(t("status.ocrError", { error: extractError(err) }));
      setStatusType("error");
    } finally {
      setIsOcrRunning(false);
    }
  };

  const copyOcrText = async () => {
    if (!ocrText) return;
    try {
      await navigator.clipboard.writeText(ocrText);
      logger.debug("OCR", "Texte copié dans le presse-papier");
      setStatusMessage(t("status.textCopied"));
      setStatusType("ready");
    } catch (err) {
      logger.error("OCR", "Échec copie texte", extractError(err));
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
      logger.info("Settings", "Sauvegarde des paramètres...");
      await invoke("save_app_settings", { settings: updatedSettings });
      logger.info("Settings", "Paramètres sauvegardés");
      setShowSettings(false);
      setStatusMessage(t("status.settingsSaved"));
      setStatusType("ready");
    } catch (err) {
      logger.error("Settings", "Échec sauvegarde paramètres", extractError(err));
      setStatusMessage(t("status.settingsError", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  const selectOutputDir = async () => {
    try {
      const dir = await selectDirectory();
      if (dir) setSettings((s) => ({ ...s, output_dir: dir }));
    } catch { /* Dialog not available */ }
  };

  const selectWatchFolder = async () => {
    try {
      const dir = await selectDirectory();
      if (dir) setSettings((s) => ({ ...s, watch_folder: dir }));
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
      setStatusMessage(t("status.profileError", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  const deleteProfile = async (profileId: string) => {
    try {
      const updated = await invoke<ScanProfile[]>("delete_scan_profile", { profileId });
      setScanProfiles(updated);
      if (selectedProfileId === profileId) setSelectedProfileId(null);
    } catch (err) {
      setStatusMessage(t("status.error", { error: extractError(err) }));
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
    logger.info("Scan", `Numérisation par lot lancée (${batchPageCount} pages)`, `Scanner: ${selectedScanner}`);
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
      logger.info("Scan", `Lot terminé: ${results.length} page(s) numérisées`);
      setStatusMessage(t("status.batchDone", { count: results.length }));
      setStatusType("ready");
    } catch (err) {
      clearInterval(progressInterval);
      setScanProgress(0);
      logger.error("Scan", "Échec numérisation par lot", extractError(err));
      setStatusMessage(t("status.batchError", { error: extractError(err) }));
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
      setStatusMessage(t("status.error", { error: extractError(err) }));
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
      setStatusMessage(t("status.error", { error: extractError(err) }));
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
      logger.info("Intelligence", `Analyse du document: ${selectedDocument.name}`);
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
      logger.info("Intelligence", `Analyse terminée: ${docTypeLabel} (${Math.round(result.classification.confidence * 100)}%)`, `Tags: ${result.auto_tags.join(", ") || "aucun"}, Règles: ${result.rule_results.length}`);
      setStatusMessage(t("status.analysisDone", { type: docTypeLabel, confidence: Math.round(result.classification.confidence * 100) }));
      setStatusType("ready");
    } catch (err) {
      logger.error("Intelligence", "Échec analyse document", extractError(err));
      setStatusMessage(t("status.analysisError", { error: extractError(err) }));
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

  const applyRuleActions = async (ruleName: string, actions: RuleAction[]) => {
    if (!selectedDocument) return;
    await invoke("apply_rule_actions", { docId: selectedDocument.id, actions });
    for (const action of actions) {
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
    setStatusMessage(t("status.ruleApplied", { name: ruleName }));
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
      setStatusMessage(t("status.error", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  const deleteRule = async (ruleId: string) => {
    try {
      const updated = await invoke<AutomationRule[]>("delete_automation_rule", { ruleId });
      setAutomationRules(updated);
    } catch (err) {
      setStatusMessage(t("status.error", { error: extractError(err) }));
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
      logger.debug("Processing", `Rotation ${direction}° du document ${docId}`);
      const result = await invoke<ScanResultDto>("rotate_document", { docId, direction });
      const updated = dtoToDoc(result);
      if (selectedDocument?.id === updated.id) setSelectedDocument(updated);
      setDocuments((docs) => docs.map((d) => (d.id === updated.id ? updated : d)));
      setStatusMessage(t("status.rotateComplete", { deg: direction }));
      setStatusType("ready");
    } catch (err) {
      logger.error("Processing", `Échec rotation ${direction}°`, extractError(err));
      setStatusMessage(t("status.rotateError", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  const flipDocument = async (axis: string, targetDocId?: string) => {
    const docId = targetDocId || selectedDocument?.id;
    if (!docId) return;
    try {
      setStatusMessage(t("status.flipping"));
      setStatusType("scanning");
      logger.debug("Processing", `Retournement ${axis} du document ${docId}`);
      const result = await invoke<ScanResultDto>("flip_document", { docId, axis });
      const updated = dtoToDoc(result);
      if (selectedDocument?.id === updated.id) setSelectedDocument(updated);
      setDocuments((docs) => docs.map((d) => (d.id === updated.id ? updated : d)));
      setStatusMessage(t("status.flipComplete"));
      setStatusType("ready");
    } catch (err) {
      logger.error("Processing", `Échec retournement ${axis}`, extractError(err));
      setStatusMessage(t("status.flipError", { error: extractError(err) }));
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
      logger.info("Processing", `Application ajustements`, `Luminosité: ${adjustments.brightness}, Contraste: ${adjustments.contrast}, Saturation: ${adjustments.saturation}, Netteté: ${adjustments.sharpness}`);
      const result = await invoke<ScanResultDto>("commit_adjustments", { docId: selectedDocument.id, adjustments });
      const updated = dtoToDoc(result);
      setSelectedDocument(updated);
      setDocuments((docs) => docs.map((d) => (d.id === updated.id ? updated : d)));
      setAdjustments({ brightness: 0, contrast: 0, saturation: 0, sharpness: 0 });
      logger.info("Processing", "Ajustements appliqués");
      setStatusMessage(t("status.adjustmentsApplied"));
      setStatusType("ready");
    } catch (err) {
      logger.error("Processing", "Échec application ajustements", extractError(err));
      setStatusMessage(t("status.adjustmentsError", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  const revertAdjustments = async () => {
    if (!selectedDocument) return;
    try {
      logger.debug("Processing", "Annulation des ajustements");
      const result = await invoke<ScanResultDto>("revert_adjustments", { docId: selectedDocument.id });
      const updated = dtoToDoc(result);
      setSelectedDocument(updated);
      setDocuments((docs) => docs.map((d) => (d.id === updated.id ? updated : d)));
      setAdjustments({ brightness: 0, contrast: 0, saturation: 0, sharpness: 0 });
      setStatusMessage(t("status.adjustmentsCancelled"));
      setStatusType("ready");
    } catch (err) {
      logger.error("Processing", "Échec annulation ajustements", extractError(err));
      setStatusMessage(t("status.error", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  // ─── Processing operations ──────────────────────────────────────
  const denoiseDocument = async (strength: number) => {
    if (!selectedDocument) return;
    try {
      setStatusMessage(t("status.denoising"));
      setStatusType("scanning");
      logger.info("Processing", `Débruitage (force: ${strength})`, `Document: ${selectedDocument.name}`);
      const result = await invoke<ScanResultDto>("denoise_document", { docId: selectedDocument.id, strength });
      const updated = dtoToDoc(result);
      setSelectedDocument(updated);
      setDocuments((docs) => docs.map((d) => (d.id === updated.id ? updated : d)));
      logger.info("Processing", "Débruitage terminé");
      setStatusMessage(t("status.denoiseComplete"));
      setStatusType("ready");
    } catch (err) {
      logger.error("Processing", "Échec débruitage", extractError(err));
      setStatusMessage(t("status.denoiseError", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  const deskewDocument = async () => {
    if (!selectedDocument) return;
    try {
      setStatusMessage(t("status.deskewing"));
      setStatusType("scanning");
      logger.info("Processing", `Redressement: ${selectedDocument.name}`);
      const result = await invoke<ScanResultDto>("deskew_document", { docId: selectedDocument.id });
      const updated = dtoToDoc(result);
      setSelectedDocument(updated);
      setDocuments((docs) => docs.map((d) => (d.id === updated.id ? updated : d)));
      logger.info("Processing", "Redressement terminé");
      setStatusMessage(t("status.deskewComplete"));
      setStatusType("ready");
    } catch (err) {
      logger.error("Processing", "Échec redressement", extractError(err));
      setStatusMessage(t("status.deskewError", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  const whitenBackground = async () => {
    if (!selectedDocument) return;
    try {
      setStatusMessage(t("status.whitening"));
      setStatusType("scanning");
      logger.info("Processing", `Blanchiment fond: ${selectedDocument.name}`);
      const result = await invoke<ScanResultDto>("whiten_document_background", { docId: selectedDocument.id, threshold: 200 });
      const updated = dtoToDoc(result);
      setSelectedDocument(updated);
      setDocuments((docs) => docs.map((d) => (d.id === updated.id ? updated : d)));
      logger.info("Processing", "Blanchiment terminé");
      setStatusMessage(t("status.whiteningComplete"));
      setStatusType("ready");
    } catch (err) {
      logger.error("Processing", "Échec blanchiment fond", extractError(err));
      setStatusMessage(t("status.whiteningError", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  // ─── Multipage ──────────────────────────────────────────────────
  const addPageToMultipage = async (docId: string) => {
    if (!multipageDoc) return;
    try {
      const updated = await invoke<MultiPageDocDto>("add_page_to_document", { multipageId: multipageDoc.id, docId, position: null });
      setMultipageDoc(updated);
      setStatusMessage(t("status.pageAdded", { count: updated.page_count }));
      setStatusType("ready");
    } catch (err) {
      setStatusMessage(t("status.error", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  const removePageFromMultipage = async (pageIndex: number) => {
    if (!multipageDoc) return;
    try {
      const updated = await invoke<MultiPageDocDto>("remove_page_from_document", { multipageId: multipageDoc.id, pageIndex });
      setMultipageDoc(updated);
    } catch (err) {
      setStatusMessage(t("status.error", { error: extractError(err) }));
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
      setStatusMessage(t("status.reorderError", { error: extractError(err) }));
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
      setStatusMessage(t("status.error", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  // ─── Annotations ────────────────────────────────────────────────
  const getSvgCoords = (e: React.MouseEvent<SVGSVGElement>): { x: number; y: number } => {
    const svg = e.currentTarget;
    const rect = svg.getBoundingClientRect();
    return { x: (e.clientX - rect.left) / rect.width, y: (e.clientY - rect.top) / rect.height };
  };

  const handleAnnotationMouseDown = (e: React.MouseEvent<SVGSVGElement>) => {
    if (!annotationTool) return;
    const coords = getSvgCoords(e);
    setIsDrawing(true);
    setDrawStart(coords);
    setCurrentDrawPos(coords);
  };

  const handleAnnotationMouseMove = (e: React.MouseEvent<SVGSVGElement>) => {
    if (!isDrawing) return;
    setCurrentDrawPos(getSvgCoords(e));
  };

  const handleAnnotationMouseUp = (e: React.MouseEvent<SVGSVGElement>) => {
    if (!isDrawing || !drawStart || !annotationTool || !selectedDocument) {
      setIsDrawing(false);
      return;
    }
    const end = getSvgCoords(e);
    const x = Math.min(drawStart.x, end.x);
    const y = Math.min(drawStart.y, end.y);
    const w = Math.abs(end.x - drawStart.x);
    const h = Math.abs(end.y - drawStart.y);
    if (w < 0.005 && h < 0.005) {
      setIsDrawing(false);
      return;
    }
    const colorMap: Record<AnnotationTypeName, string> = { Highlight: "#FFFF00", Ellipse: "#FF0000", TextNote: "#3B82F6" };
    const annotation: AnnotationData = {
      annotation_type: annotationTool,
      x, y, width: w, height: h,
      color: colorMap[annotationTool],
      text: annotationTool === "TextNote" ? prompt(t("export.annotationTextPrompt") || "Note:") || "" : null,
    };
    setAnnotations((prev) => ({
      ...prev,
      [selectedDocument.id]: [...(prev[selectedDocument.id] || []), annotation],
    }));
    setIsDrawing(false);
    setDrawStart(null);
    setCurrentDrawPos(null);
  };

  const clearAnnotations = () => {
    if (!selectedDocument) return;
    setAnnotations((prev) => ({ ...prev, [selectedDocument.id]: [] }));
  };

  // ─── Signature ──────────────────────────────────────────────────
  const handleSignatureMouseDown = (e: React.MouseEvent<SVGSVGElement>) => {
    const coords = getSvgCoords(e);
    setSigDragStart(coords);
    setSigDragCurrent(coords);
  };

  const handleSignatureMouseMove = (e: React.MouseEvent<SVGSVGElement>) => {
    if (!sigDragStart) return;
    setSigDragCurrent(getSvgCoords(e));
  };

  const handleSignatureMouseUp = (_e: React.MouseEvent<SVGSVGElement>) => {
    if (!sigDragStart || !sigDragCurrent) return;
    const x = Math.min(sigDragStart.x, sigDragCurrent.x);
    const y = Math.min(sigDragStart.y, sigDragCurrent.y);
    const w = Math.abs(sigDragCurrent.x - sigDragStart.x);
    const h = Math.abs(sigDragCurrent.y - sigDragStart.y);
    if (w > 0.01 && h > 0.01) {
      setSignaturePlacement({ x, y, w, h });
    }
    setSigDragStart(null);
    setSigDragCurrent(null);
    setIsPlacingSignature(false);
  };

  // ─── Signature Canvas ──────────────────────────────────────────
  const handleSigCanvasMouseDown = (e: React.MouseEvent<HTMLCanvasElement>) => {
    setIsDrawingSig(true);
    const canvas = sigCanvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const rect = canvas.getBoundingClientRect();
    ctx.beginPath();
    ctx.moveTo(e.clientX - rect.left, e.clientY - rect.top);
  };

  const handleSigCanvasMouseMove = (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (!isDrawingSig) return;
    const canvas = sigCanvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const rect = canvas.getBoundingClientRect();
    ctx.lineWidth = 2;
    ctx.strokeStyle = "#000";
    ctx.lineTo(e.clientX - rect.left, e.clientY - rect.top);
    ctx.stroke();
  };

  const handleSigCanvasMouseUp = () => {
    setIsDrawingSig(false);
  };

  const clearSigCanvas = () => {
    const canvas = sigCanvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (ctx) ctx.clearRect(0, 0, canvas.width, canvas.height);
  };

  const importSignatureImage = async () => {
    // Use file input to import an image
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "image/*";
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) return;
      const reader = new FileReader();
      reader.onload = () => {
        const base64 = (reader.result as string).split(",")[1];
        setSignatureImage(base64);
        setShowSignatureDialog(false);
        setIsPlacingSignature(true);
      };
      reader.readAsDataURL(file);
    };
    input.click();
  };

  const saveSignatureFromCanvas = () => {
    const canvas = sigCanvasRef.current;
    if (!canvas) return;
    const dataUrl = canvas.toDataURL("image/png");
    const base64 = dataUrl.split(",")[1];
    setSignatureImage(base64);
    setShowSignatureDialog(false);
    setIsPlacingSignature(true);
  };

  // ─── Export with options ─────────────────────────────────────────
  const handleExportPdf = async () => {
    if (!selectedDocument && !(multipageDoc && multipageDoc.page_count > 0)) return;

    const docId = selectedDocument?.id;
    const defaultName = multipageDoc && multipageDoc.page_count > 0
      ? `${multipageDoc.name}.pdf`
      : selectedDocument?.name.replace(/\.\w+$/, ".pdf") ?? "document.pdf";

    const path = await save({ defaultPath: defaultName, filters: [{ name: "PDF", extensions: ["pdf"] }] });
    if (!path) return;

    setStatusMessage(t("status.savingPdf"));
    setStatusType("scanning");

    try {
      const pdfaMap: Record<string, string | null> = { none: null, a1b: "A1b", a2b: "A2b" };
      const watermark: WatermarkConfig | null = exportWatermarkEnabled ? {
        text: exportWatermarkText,
        opacity: exportWatermarkOpacity,
        rotation: exportWatermarkRotation,
        font_size: exportWatermarkFontSize,
        color: exportWatermarkColor,
        position: exportWatermarkPosition,
      } : null;

      const signature: SignatureConfig | null = signatureImage && signaturePlacement ? {
        image_base64: signatureImage,
        page_index: 0,
        x: signaturePlacement.x,
        y: signaturePlacement.y,
        width: signaturePlacement.w,
        height: signaturePlacement.h,
      } : null;

      const currentAnnotations = docId ? annotations[docId] ?? [] : [];
      const pageAnnotations: PageAnnotationsData[] = currentAnnotations.length > 0
        ? [{ page_index: 0, annotations: currentAnnotations }]
        : [];

      const options: PdfExportOptions = {
        pdfa: pdfaMap[exportPdfa] as PdfExportOptions["pdfa"],
        user_password: exportUserPassword || null,
        owner_password: exportOwnerPassword || null,
        watermark,
        signature,
      };

      logger.info("Export", `Export PDF avancé: ${path}`, `PDF/A: ${exportPdfa}, Chiffrement: ${exportUserPassword ? "oui" : "non"}, Filigrane: ${watermark ? "oui" : "non"}, Signature: ${signature ? "oui" : "non"}, Annotations: ${pageAnnotations.length > 0 ? pageAnnotations[0].annotations.length : 0}`);

      let result: PdfSaveResult;
      if (multipageDoc && multipageDoc.page_count > 0) {
        result = await invoke<PdfSaveResult>("export_multipage_pdf_advanced", {
          multipageId: multipageDoc.id,
          outputPath: path,
          options,
          annotations: pageAnnotations,
        });
      } else {
        result = await invoke<PdfSaveResult>("export_pdf_advanced", {
          docId,
          outputPath: path,
          options,
          annotations: pageAnnotations,
        });
      }

      setLastExportHash(result.sha256);
      setShowExportDialog(false);
      logger.info("Export", `PDF exporté: ${path.split(/[/\\]/).pop()}`, result.sha256 ? `SHA-256: ${result.sha256}` : undefined);
      setStatusMessage(t("status.pdfSaved", { filename: path.split(/[/\\]/).pop() ?? "" }));
      setStatusType("ready");
    } catch (err) {
      logger.error("Export", "Échec export PDF avancé", extractError(err));
      setStatusMessage(t("status.pdfError", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  // ─── Watermark confirm ──────────────────────────────────────────
  const handleWatermarkConfirm = () => {
    setExportWatermarkEnabled(true);
    setShowWatermarkDialog(false);
  };

  // ─── Email ──────────────────────────────────────────────────────
  const emailDocument = async (docId?: string) => {
    const id = docId || selectedDocument?.id;
    if (!id) return;
    try {
      setStatusMessage(t("status.emailing"));
      setStatusType("scanning");
      logger.info("Email", `Envoi par email: ${id}`);
      await invoke("send_document_by_email", { docId: id });
      logger.info("Email", "Email envoyé");
      setStatusMessage(t("status.emailSent"));
      setStatusType("ready");
    } catch (err) {
      logger.error("Email", "Échec envoi email", extractError(err));
      setStatusMessage(t("status.error", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  // ─── Vault operations ──────────────────────────────────────────
  const openVault = async () => {
    setShowVault(true);
    try {
      const setup = await invoke<boolean>("vault_is_setup");
      setVaultSetup(setup);
    } catch { setVaultSetup(false); }
  };

  const unlockVault = async () => {
    try {
      if (!vaultSetup) {
        logger.info("Vault", "Configuration du mot de passe du coffre-fort");
        await invoke("vault_set_password", { password: vaultPassword });
        setVaultSetup(true);
      }
      logger.info("Vault", "Déverrouillage du coffre-fort");
      await invoke("vault_unlock", { password: vaultPassword });
      setVaultUnlocked(true);
      const docs = await invoke<VaultDocument[]>("vault_list_documents");
      setVaultDocs(docs);
      setVaultPassword("");
      logger.info("Vault", `Coffre-fort déverrouillé: ${docs.length} document(s)`);
    } catch (err) {
      logger.error("Vault", "Échec déverrouillage coffre-fort", extractError(err));
      setStatusMessage(t("status.vaultError", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  const lockVault = async () => {
    try {
      await invoke("vault_lock");
      setVaultUnlocked(false);
      setVaultDocs([]);
    } catch { /* ignore */ }
  };

  const vaultSetupPassword = async () => {
    try {
      await invoke("vault_set_password", { password: vaultPassword });
      setVaultSetup(true);
      await unlockVault();
    } catch (err) {
      setStatusMessage(t("status.error", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  const vaultAddDocument = async (docId?: string) => {
    const id = docId || selectedDocument?.id;
    if (!id) return;
    try {
      await invoke("vault_add_document", { docId: id });
      if (vaultUnlocked) {
        const docs = await invoke<VaultDocument[]>("vault_list_documents");
        setVaultDocs(docs);
      }
      setStatusMessage(t("status.vaultAdded"));
      setStatusType("ready");
    } catch (err) {
      setStatusMessage(t("status.error", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  const vaultRemoveDocument = async (docId: string) => {
    try {
      await invoke("vault_remove_document", { docId });
      setVaultDocs((prev) => prev.filter((d) => d.id !== docId));
    } catch (err) {
      setStatusMessage(t("status.error", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  const vaultOpenDocument = async (docId: string) => {
    try {
      await invoke("vault_open_document", { docId });
    } catch (err) {
      setStatusMessage(t("status.error", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  // ─── Stats ──────────────────────────────────────────────────────
  const openStats = async () => {
    try {
      const stats = await invoke<AppStats>("get_statistics");
      setAppStats(stats);
      setShowStats(true);
    } catch (err) {
      setStatusMessage(t("status.error", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  // ─── Undo ──────────────────────────────────────────────────────
  const undoLastAction = async () => {
    if (!selectedDocument) return;
    try {
      const result = await invoke<ScanResultDto>("undo_last_action", { docId: selectedDocument.id });
      const updated = dtoToDoc(result);
      setSelectedDocument(updated);
      setDocuments((docs) => docs.map((d) => (d.id === updated.id ? updated : d)));
      setStatusMessage(t("status.undone"));
      setStatusType("ready");
    } catch (err) {
      setStatusMessage(t("status.error", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  // ─── AI features ──────────────────────────────────────────────
  const aiOcr = async () => {
    if (!selectedDocument) return;
    setIsAiLoading(true);
    logger.info("AI", `OCR IA lancé: ${selectedDocument.name}`);
    try {
      const text = await invoke<string>("groq_ocr", { docId: selectedDocument.id });
      setOcrText(text);
      setActiveView("ocr");
      logger.info("AI", `OCR IA terminé: ${text.length} caractères`);
    } catch (err) {
      logger.error("AI", "Échec OCR IA", extractError(err));
      setStatusMessage(t("status.error", { error: extractError(err) }));
      setStatusType("error");
    } finally {
      setIsAiLoading(false);
    }
  };

  const aiSummarize = async () => {
    if (!selectedDocument) return;
    setIsAiLoading(true);
    logger.info("AI", `Résumé IA lancé: ${selectedDocument.name}`);
    try {
      const summary = await invoke<string>("groq_summarize", { docId: selectedDocument.id });
      setAiSummary(summary);
      logger.info("AI", `Résumé généré: ${summary.length} caractères`);
    } catch (err) {
      logger.error("AI", "Échec résumé IA", extractError(err));
      setStatusMessage(t("status.error", { error: extractError(err) }));
      setStatusType("error");
    } finally {
      setIsAiLoading(false);
    }
  };

  const aiTranslate = async () => {
    if (!selectedDocument) return;
    setIsAiLoading(true);
    logger.info("AI", `Traduction IA lancée vers ${aiTargetLang}`);
    try {
      const translation = await invoke<string>("groq_translate", { docId: selectedDocument.id, targetLang: aiTargetLang });
      setAiTranslation(translation);
      logger.info("AI", `Traduction terminée: ${translation.length} caractères`);
    } catch (err) {
      logger.error("AI", "Échec traduction IA", extractError(err));
      setStatusMessage(t("status.error", { error: extractError(err) }));
      setStatusType("error");
    } finally {
      setIsAiLoading(false);
    }
  };

  const detectSensitive = async () => {
    if (!selectedDocument) return;
    logger.info("AI", `Détection données sensibles: ${selectedDocument.name}`);
    try {
      const items = await invoke<SensitiveInfo[]>("detect_sensitive_info", { docId: selectedDocument.id });
      setSensitiveItems(items);
      if (items.length > 0) setShowRedaction(true);
      logger.info("AI", `${items.length} élément(s) sensible(s) détecté(s)`);
    } catch (err) {
      logger.error("AI", "Échec détection données sensibles", extractError(err));
      setStatusMessage(t("status.error", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  const detectTables = async () => {
    if (!selectedDocument) return;
    logger.info("AI", `Détection tableaux: ${selectedDocument.name}`);
    try {
      const tables = await invoke<DetectedTable[]>("detect_tables", { docId: selectedDocument.id });
      setDetectedTables(tables);
      logger.info("AI", `${tables.length} tableau(x) détecté(s)`);
    } catch (err) {
      logger.error("AI", "Échec détection tableaux", extractError(err));
      setStatusMessage(t("status.error", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  const semanticSearch = async () => {
    if (!semanticQuery.trim()) return;
    logger.debug("Search", `Recherche sémantique: "${semanticQuery}"`);
    try {
      const results = await invoke<SemanticResult[]>("semantic_search", { query: semanticQuery });
      setSemanticResults(results);
      logger.debug("Search", `${results.length} résultat(s) trouvé(s)`);
    } catch (err) {
      logger.error("Search", "Échec recherche sémantique", extractError(err));
      setStatusMessage(t("status.error", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  const applyRedactions = async () => {
    if (!selectedDocument || sensitiveItems.length === 0) return;
    logger.info("AI", `Application caviardage: ${sensitiveItems.length} élément(s)`);
    try {
      const result = await invoke<ScanResultDto>("apply_redactions", { docId: selectedDocument.id, items: sensitiveItems });
      const updated = dtoToDoc(result);
      setSelectedDocument(updated);
      setDocuments((docs) => docs.map((d) => (d.id === updated.id ? updated : d)));
      setShowRedaction(false);
      setSensitiveItems([]);
      logger.info("AI", "Caviardage appliqué");
      setStatusMessage(t("status.redactionApplied"));
      setStatusType("ready");
    } catch (err) {
      logger.error("AI", "Échec caviardage", extractError(err));
      setStatusMessage(t("status.error", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  const exportTableCsv = async (table: DetectedTable) => {
    try {
      const csv = table.rows.map((row) => row.join(",")).join("\n");
      await navigator.clipboard.writeText(csv);
      setStatusMessage(t("status.tableSaved"));
      setStatusType("ready");
    } catch (err) {
      setStatusMessage(t("status.error", { error: extractError(err) }));
      setStatusType("error");
    }
  };

  // ─── Context menu handlers ──────────────────────────────────────
  const handleHistoryContextMenu = (e: React.MouseEvent, docId: string) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY, docId });
  };

  const handleMultipageContextMenu = (e: React.MouseEvent, docId: string, pageIndex: number) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY, docId, pageIndex });
  };

  const handlePreviewContextMenu = (e: React.MouseEvent) => {
    if (!selectedDocument) return;
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY, docId: selectedDocument.id, isPreview: true });
  };

  const handleStartRename = (docId: string) => {
    setRenamingDocId(docId);
    setRenameValue(documents.find((d) => d.id === docId)?.name ?? "");
    setActiveView("history");
  };

  const handleSelectHistoryDoc = (doc: ScannedDocument) => {
    setSelectedDocument(doc);
    setActiveView("preview");
  };

  // ── Derived values ──
  const hasDocument = selectedDocument !== null;
  const hasMultipage = multipageDoc !== null && multipageDoc.page_count > 0;
  const currentAnnotations = selectedDocument ? annotations[selectedDocument.id] ?? [] : [];

  return (
    <div className="app">
      {/* a11y: Skip link */}
      <a href="#main-content" className="skip-link">{t("a11y.skipToContent")}</a>

      {/* Drag-and-drop overlay */}
      <DropOverlay isDragOver={isDragOver} isImporting={isImporting} />

      {/* Background */}
      <div className="bg-mesh" aria-hidden="true">
        <div className="orb orb-1" /><div className="orb orb-2" /><div className="orb orb-3" />
      </div>

      {/* Sidebar */}
      <Sidebar
        scanners={scanners}
        selectedScanner={selectedScanner}
        onSelectScanner={setSelectedScanner}
        onRefresh={loadScanners}
        isRefreshing={isRefreshing}
        themeMode={themeMode}
        onThemeChange={setThemeMode}
      />

      {/* Main Content */}
      <main id="main-content" className="main-content">
        {/* Action Bar */}
        <ActionBar
          onScan={startScan}
          onExportPdf={() => setShowExportDialog(true)}
          onExportImage={saveAsImage}
          onPrint={printDoc}
          onCrop={autoCrop}
          onOcr={runOcr}
          onAnalyze={analyzeDocument}
          onEmail={() => emailDocument()}
          onVault={openVault}
          onStats={openStats}
          onUndo={undoLastAction}
          onRules={() => setShowRulesModal(true)}
          onSettings={() => setShowSettings(true)}
          onAiPanel={() => setRightPanelMode("intelligence")}
          canScan={!!selectedScanner}
          isScanning={isScanning}
          hasDocument={hasDocument}
          hasMultipage={hasMultipage}
          isOcrRunning={isOcrRunning}
          isAnalyzing={isAnalyzing}
          canUndo={hasDocument}
        />

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
                  <button className="btn btn-icon btn-sm btn-ghost" onClick={() => setZoomLevel((z) => Math.min(400, z + 25))} aria-label={t("a11y.zoomIn")}>{Icons.zoomIn}</button>
                  <button className="btn btn-icon btn-sm btn-ghost" onClick={() => setZoomLevel(100)} aria-label="Reset zoom" style={{ marginLeft: 4, fontSize: '0.7rem', opacity: zoomLevel !== 100 ? 1 : 0.4 }}>1:1</button>
                </div>
              )}
            </div>

            {activeView === "preview" ? (
              <div className="preview-content" role="tabpanel" onWheel={(e) => { if (e.ctrlKey || e.metaKey) { e.preventDefault(); setZoomLevel((z) => Math.max(25, Math.min(400, z + (e.deltaY < 0 ? 25 : -25)))); } }}>
                {/* Multipage panel */}
                {hasMultipage && !isScanning && (
                  <MultipagePanel
                    multipageDoc={multipageDoc}
                    documents={documents}
                    selectedDocument={selectedDocument}
                    onSelectDocument={setSelectedDocument}
                    onAddPage={addPageToMultipage}
                    onRemovePage={removePageFromMultipage}
                    onReorderPages={handleDragEnd}
                    onSaveMultipagePdf={saveMultipagePdf}
                    onClose={() => setMultipageDoc(null)}
                    onContextMenu={handleMultipageContextMenu}
                    zoomLevel={zoomLevel}
                  />
                )}

                {/* Preview / scanning / empty */}
                {(!hasMultipage || isScanning) && (
                  <Preview
                    selectedDocument={selectedDocument}
                    zoomLevel={zoomLevel}
                    onZoomChange={setZoomLevel}
                    annotations={annotations}
                    annotationTool={annotationTool}
                    onAnnotationToolChange={setAnnotationTool}
                    isDrawing={isDrawing}
                    drawStart={drawStart}
                    currentDrawPos={currentDrawPos}
                    onMouseDown={handleAnnotationMouseDown}
                    onMouseMove={handleAnnotationMouseMove}
                    onMouseUp={handleAnnotationMouseUp}
                    signatureImage={signatureImage}
                    signaturePlacement={signaturePlacement}
                    isPlacingSignature={isPlacingSignature}
                    onSignatureMouseDown={handleSignatureMouseDown}
                    onSignatureMouseMove={handleSignatureMouseMove}
                    onSignatureMouseUp={handleSignatureMouseUp}
                    sigDragStart={sigDragStart}
                    sigDragCurrent={sigDragCurrent}
                    onContextMenu={handlePreviewContextMenu}
                    onClearAnnotations={clearAnnotations}
                    isScanning={isScanning}
                    scanProgress={scanProgress}
                    hasMultipage={hasMultipage}
                    hasScanners={scanners.length > 0}
                    onScan={startScan}
                  />
                )}
              </div>
            ) : activeView === "ocr" ? (
              <OcrPanel
                ocrText={ocrText}
                isOcrRunning={isOcrRunning}
                onRunOcr={runOcr}
                onCopyText={copyOcrText}
                hasDocument={hasDocument}
              />
            ) : (
              <HistoryGrid
                documents={documents}
                selectedDocument={selectedDocument}
                onSelectDocument={handleSelectHistoryDoc}
                searchQuery={searchQuery}
                onSearchChange={setSearchQuery}
                onSearch={handleSearch}
                searchResults={searchResults}
                renamingDocId={renamingDocId}
                renameValue={renameValue}
                onStartRename={handleStartRename}
                onRenameChange={setRenameValue}
                onRenameSubmit={renameDoc}
                onRenameCancel={() => setRenamingDocId(null)}
                onContextMenu={handleHistoryContextMenu}
                onDeleteDocument={deleteDocument}
                zoomLevel={zoomLevel}
                documentTags={documentTags}
              />
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
              <ConfigPanel
                config={config}
                onConfigChange={setConfig}
                scanProfiles={scanProfiles}
                selectedProfileId={selectedProfileId}
                onSelectProfile={applyProfile}
                onSaveProfile={saveCurrentAsProfile}
                onDeleteProfile={deleteProfile}
                scanners={scanners}
                selectedScanner={selectedScanner}
                batchMode={batchMode}
                batchPageCount={batchPageCount}
                onBatchModeChange={setBatchMode}
                onBatchPageCountChange={setBatchPageCount}
                onBatchScan={startBatchScan}
                isScanning={isScanning}
              />
            ) : rightPanelMode === "edit" ? (
              <EditPanel
                adjustments={adjustments}
                onAdjustmentChange={handleAdjustmentChange}
                onApplyAdjustments={commitAdjustments}
                onRevertAdjustments={revertAdjustments}
                isAdjusting={isAdjusting}
                onRotate={(dir) => rotateDocument(dir)}
                onFlip={(axis) => flipDocument(axis)}
                onDeskew={deskewDocument}
                onWhiten={whitenBackground}
                onDenoise={denoiseDocument}
                hasDocument={hasDocument}
              />
            ) : (
              <IntelligencePanel
                documentTags={selectedDocument ? documentTags[selectedDocument.id] || [] : []}
                tagDefinitions={tagDefinitions}
                onAddTag={(tag) => selectedDocument && addTag(selectedDocument.id, tag)}
                onRemoveTag={(tag) => selectedDocument && removeTag(selectedDocument.id, tag)}
                analysisResult={analysisResult}
                isAnalyzing={isAnalyzing}
                onAnalyze={analyzeDocument}
                hasDocument={hasDocument}
                selectedDocId={selectedDocument?.id ?? null}
                onApplySuggestion={applySuggestion}
                onApplyRuleActions={applyRuleActions}
                aiSummary={aiSummary}
                aiTranslation={aiTranslation}
                aiTargetLang={aiTargetLang}
                onAiTargetLangChange={setAiTargetLang}
                isAiLoading={isAiLoading}
                onAiOcr={aiOcr}
                onAiSummarize={aiSummarize}
                onAiTranslate={aiTranslate}
                onDetectSensitive={detectSensitive}
                onDetectTables={detectTables}
                semanticQuery={semanticQuery}
                semanticResults={semanticResults}
                onSemanticQueryChange={setSemanticQuery}
                onSemanticSearch={semanticSearch}
                detectedTables={detectedTables}
                sensitiveItems={sensitiveItems}
                hasGroqKey={!!settings.groq_api_key}
                onShowRedaction={() => setShowRedaction(true)}
                onExportTableCsv={exportTableCsv}
              />
            )}
          </div>
        </div>

        {/* Status Bar */}
        <StatusBar
          statusMessage={statusMessage}
          statusType={statusType}
          scanProgress={scanProgress}
          isScanning={isScanning}
          isAdjusting={isAdjusting}
          onToggleDebug={() => setShowDebugPanel((v) => !v)}
          showDebugPanel={showDebugPanel}
        />
      </main>

      {/* Context Menu */}
      <ContextMenu
        contextMenu={contextMenu}
        onClose={() => setContextMenu(null)}
        onRename={handleStartRename}
        onDuplicate={duplicateDoc}
        onDelete={deleteDocument}
        onEmail={emailDocument}
        onVaultAdd={vaultAddDocument}
        onRotate={(direction, docId) => rotateDocument(direction, docId)}
        onFlip={(axis, docId) => flipDocument(axis, docId)}
        onWatermark={() => setShowWatermarkDialog(true)}
        onSignature={() => setShowSignatureDialog(true)}
        onRemovePage={removePageFromMultipage}
        onAddToPages={addPageToMultipage}
        exportWatermarkEnabled={exportWatermarkEnabled}
        onRemoveWatermark={() => setExportWatermarkEnabled(false)}
        signaturePlacement={signaturePlacement}
        signatureImage={signatureImage}
        onRemoveSignature={() => { setSignaturePlacement(null); setSignatureImage(null); }}
        hasMultipage={hasMultipage}
      />

      {/* Settings Modal */}
      <SettingsModal
        show={showSettings}
        onClose={() => setShowSettings(false)}
        settings={settings}
        onSettingsChange={setSettings}
        onSaveSettings={handleSaveSettings}
        onSelectOutputDir={selectOutputDir}
        onSelectWatchFolder={selectWatchFolder}
        language={language}
        onLanguageChange={setLanguage}
        themeMode={themeMode}
        onThemeChange={setThemeMode}
      />

      {/* Export Dialog */}
      <ExportDialog
        show={showExportDialog}
        onClose={() => setShowExportDialog(false)}
        onExport={handleExportPdf}
        exportPdfa={exportPdfa}
        onPdfaChange={setExportPdfa}
        exportUserPassword={exportUserPassword}
        onUserPasswordChange={setExportUserPassword}
        exportOwnerPassword={exportOwnerPassword}
        onOwnerPasswordChange={setExportOwnerPassword}
        exportWatermarkEnabled={exportWatermarkEnabled}
        exportWatermarkText={exportWatermarkText}
        hasAnnotations={currentAnnotations.length > 0}
        annotationCount={currentAnnotations.length}
        hasSignature={!!signaturePlacement && !!signatureImage}
        lastExportHash={lastExportHash}
      />

      {/* Rules Modal */}
      <RulesModal
        show={showRulesModal}
        onClose={() => setShowRulesModal(false)}
        rules={automationRules}
        editingRule={editingRule}
        onEditRule={setEditingRule}
        onSaveRule={saveRule}
        onDeleteRule={deleteRule}
        onNewRule={() => setEditingRule({
          id: crypto.randomUUID(), name: t("rules.newRuleDefault"), enabled: true, condition_logic: "And",
          conditions: [{ field: "DocumentType", operator: "Equals", value: "Facture" }],
          actions: [{ action_type: "AddTag", value: "Facture" }],
        })}
        onCancelEdit={() => setEditingRule(null)}
        scanProfiles={scanProfiles}
        tagDefinitions={tagDefinitions}
      />

      {/* Stats Modal */}
      <StatsModal
        show={showStats}
        onClose={() => setShowStats(false)}
        stats={appStats}
      />

      {/* Watermark Dialog */}
      <WatermarkDialog
        show={showWatermarkDialog}
        onClose={() => setShowWatermarkDialog(false)}
        text={exportWatermarkText}
        onTextChange={setExportWatermarkText}
        opacity={exportWatermarkOpacity}
        onOpacityChange={setExportWatermarkOpacity}
        rotation={exportWatermarkRotation}
        onRotationChange={setExportWatermarkRotation}
        fontSize={exportWatermarkFontSize}
        onFontSizeChange={setExportWatermarkFontSize}
        color={exportWatermarkColor}
        onColorChange={setExportWatermarkColor}
        position={exportWatermarkPosition}
        onPositionChange={setExportWatermarkPosition}
        onConfirm={handleWatermarkConfirm}
      />

      {/* Signature Dialog */}
      <SignatureDialog
        show={showSignatureDialog}
        onClose={() => setShowSignatureDialog(false)}
        canvasRef={sigCanvasRef}
        onClear={clearSigCanvas}
        onImport={importSignatureImage}
        onSave={saveSignatureFromCanvas}
        onCanvasMouseDown={handleSigCanvasMouseDown}
        onCanvasMouseMove={handleSigCanvasMouseMove}
        onCanvasMouseUp={handleSigCanvasMouseUp}
      />

      {/* Redaction Modal */}
      <RedactionModal
        show={showRedaction}
        onClose={() => setShowRedaction(false)}
        items={sensitiveItems}
        onApplyRedactions={applyRedactions}
      />

      {/* Vault Modal */}
      <VaultModal
        show={showVault}
        onClose={() => setShowVault(false)}
        vaultUnlocked={vaultUnlocked}
        vaultSetup={vaultSetup}
        vaultPassword={vaultPassword}
        onPasswordChange={setVaultPassword}
        onUnlock={unlockVault}
        onLock={lockVault}
        onSetupPassword={vaultSetupPassword}
        vaultDocs={vaultDocs}
        vaultViewMode={vaultViewMode}
        onViewModeChange={setVaultViewMode}
        vaultFilter={vaultFilter}
        onFilterChange={setVaultFilter}
        onAddDocument={() => vaultAddDocument()}
        onRemoveDocument={vaultRemoveDocument}
        onOpenDocument={vaultOpenDocument}
      />

      {/* Debug Log Panel */}
      <DebugLogPanel show={showDebugPanel} onClose={() => setShowDebugPanel(false)} />

      {/* Onboarding Wizard */}
      {showOnboarding && <OnboardingWizard onComplete={handleOnboardingComplete} />}

      {/* Guided Tour */}
      {tour.isActive && tour.currentStep && (
        <TourTooltip step={tour.currentStep} stepIndex={tour.stepIndex} totalSteps={tour.totalSteps} onNext={tour.next} onPrev={tour.prev} onSkip={tour.skip} />
      )}
    </div>
  );
}

export default App;
