import { useTranslation } from "../contexts/LanguageContext";
import Icons from "./Icons";

interface OcrPanelProps {
  ocrText: string | null;
  isOcrRunning: boolean;
  onRunOcr: () => void;
  onCopyText: () => void;
  hasDocument: boolean;
}

export function OcrPanel({
  ocrText,
  isOcrRunning,
  onRunOcr,
  onCopyText,
  hasDocument,
}: OcrPanelProps) {
  const { t } = useTranslation();

  if (ocrText) {
    return (
      <div className="ocr-content" role="tabpanel">
        <div className="ocr-toolbar">
          <button
            className="btn btn-sm"
            onClick={onCopyText}
            title={t("ocr.copyTooltip")}
          >
            {Icons.copy}
            <span>{t("ocr.copy")}</span>
          </button>
          <button
            className="btn btn-sm"
            onClick={onRunOcr}
            disabled={isOcrRunning || !hasDocument}
          >
            {Icons.refresh}
            <span>{t("ocr.rerun")}</span>
          </button>
        </div>
        <div className="ocr-text-container">
          <pre className="ocr-text">{ocrText}</pre>
        </div>
      </div>
    );
  }

  return (
    <div className="ocr-content" role="tabpanel">
      <div className="preview-empty">
        <div className="preview-empty-icon">{Icons.ocr}</div>
        <div className="preview-empty-title">{t("ocr.emptyTitle")}</div>
        <div className="preview-empty-desc">{t("ocr.emptyDesc")}</div>
        {hasDocument && (
          <button
            className="btn btn-accent"
            onClick={onRunOcr}
            disabled={isOcrRunning}
          >
            {Icons.ocr} {t("ocr.launch")}
          </button>
        )}
      </div>
    </div>
  );
}
