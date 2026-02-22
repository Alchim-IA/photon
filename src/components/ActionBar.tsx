import { useTranslation } from "../contexts/LanguageContext";
import Icons from "./Icons";

interface ActionBarProps {
  onScan: () => void;
  onExportPdf: () => void;
  onExportImage: () => void;
  onPrint: () => void;
  onCrop: () => void;
  onOcr: () => void;
  onAnalyze: () => void;
  onEmail: () => void;
  onVault: () => void;
  onStats: () => void;
  onUndo: () => void;
  onRules: () => void;
  onSettings: () => void;
  onAiPanel: () => void;
  canScan: boolean;
  isScanning: boolean;
  hasDocument: boolean;
  hasMultipage: boolean;
  isOcrRunning: boolean;
  isAnalyzing: boolean;
  canUndo: boolean;
}

export function ActionBar({
  onScan,
  onExportPdf,
  onExportImage,
  onPrint,
  onCrop,
  onOcr,
  onAnalyze,
  onEmail,
  onVault,
  onStats,
  onUndo,
  onRules,
  onSettings,
  onAiPanel,
  canScan,
  isScanning,
  hasDocument,
  hasMultipage,
  isOcrRunning,
  isAnalyzing,
  canUndo,
}: ActionBarProps) {
  const { t } = useTranslation();
  const canExport = hasDocument || hasMultipage;

  return (
    <div className="action-bar">
      <button
        id="btn-scan"
        className={`btn btn-accent btn-scan ${isScanning ? "scanning" : ""}`}
        onClick={onScan}
        disabled={isScanning || !canScan}
      >
        {Icons.scan}
        {isScanning ? t("actions.scanning") : t("actions.scan")}
      </button>

      <div className="action-bar-divider" />

      <button
        id="btn-save-pdf"
        className="btn"
        onClick={onExportPdf}
        disabled={!canExport}
        title={t("actions.savePdf")}
      >
        {Icons.pdf}
        <span>{t("actions.pdf")}</span>
      </button>
      <button
        className="btn"
        onClick={onExportImage}
        disabled={!canExport}
        title={t("actions.saveImage")}
      >
        {Icons.image}
        <span>{t("actions.image")}</span>
      </button>
      <button
        className="btn"
        onClick={onPrint}
        disabled={!canExport}
        title={t("actions.print")}
      >
        {Icons.print}
        <span>{t("actions.print")}</span>
      </button>
      <button
        className="btn"
        onClick={onEmail}
        disabled={!hasDocument}
        title={t("actions.email")}
      >
        {Icons.email}
        <span>{t("actions.email")}</span>
      </button>

      <div className="action-bar-divider" />

      <button
        className="btn"
        onClick={onCrop}
        disabled={!hasDocument}
        title={t("actions.autoCrop")}
      >
        {Icons.crop}
        <span>{t("actions.crop")}</span>
      </button>
      <button
        id="btn-ocr"
        className="btn btn-ocr"
        onClick={onOcr}
        disabled={(!hasDocument && !hasMultipage) || isOcrRunning}
        title={t("actions.ocrTooltip")}
      >
        {Icons.ocr}
        <span>{isOcrRunning ? t("actions.ocrRunning") : t("actions.ocr")}</span>
      </button>
      <button
        id="btn-analyze"
        className="btn"
        onClick={onAnalyze}
        disabled={!hasDocument || isAnalyzing}
        title={t("actions.analyzeTooltip")}
      >
        {Icons.brain}
        <span>{isAnalyzing ? t("actions.analyzing") : t("actions.analyze")}</span>
      </button>

      <div className="action-bar-divider" />

      <button
        className="btn btn-icon btn-ghost"
        onClick={onRules}
        aria-label={t("actions.rules")}
      >
        {Icons.rules}
      </button>
      <button
        className="btn btn-icon btn-ghost"
        onClick={onVault}
        title={t("actions.vault")}
      >
        {Icons.vault}
      </button>
      <button
        className="btn btn-icon btn-ghost"
        onClick={onAiPanel}
        title={t("actions.ai")}
      >
        {Icons.groq}
      </button>
      <button
        className="btn btn-icon btn-ghost"
        onClick={onStats}
        title={t("actions.stats")}
      >
        {Icons.chart}
      </button>
      <button
        className="btn btn-icon btn-ghost"
        onClick={onUndo}
        disabled={!canUndo}
        title={t("actions.undo")}
      >
        {Icons.history}
      </button>

      <div className="action-bar-spacer" />

      <button
        id="btn-settings"
        className="btn btn-icon btn-ghost"
        onClick={onSettings}
        aria-label={t("actions.settings")}
      >
        {Icons.settings}
      </button>
    </div>
  );
}
