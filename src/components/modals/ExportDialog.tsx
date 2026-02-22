import { useTranslation } from "../../contexts/LanguageContext";
import Icons from "../Icons";

interface ExportDialogProps {
  show: boolean;
  onClose: () => void;
  onExport: () => void;
  exportPdfa: "none" | "a1b" | "a2b";
  onPdfaChange: (value: "none" | "a1b" | "a2b") => void;
  exportUserPassword: string;
  onUserPasswordChange: (value: string) => void;
  exportOwnerPassword: string;
  onOwnerPasswordChange: (value: string) => void;
  exportWatermarkEnabled: boolean;
  exportWatermarkText: string;
  hasAnnotations: boolean;
  annotationCount: number;
  hasSignature: boolean;
  lastExportHash: string | null;
}

export function ExportDialog({
  show,
  onClose,
  onExport,
  exportPdfa,
  onPdfaChange,
  exportUserPassword,
  onUserPasswordChange,
  exportOwnerPassword,
  onOwnerPasswordChange,
  exportWatermarkEnabled,
  exportWatermarkText,
  hasAnnotations,
  annotationCount,
  hasSignature,
}: ExportDialogProps) {
  const { t } = useTranslation();

  if (!show) return null;

  return (
    <div
      className="settings-overlay"
      onClick={(e) => e.target === e.currentTarget && onClose()}
    >
      <div
        className="settings-modal export-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="export-title"
      >
        <div className="settings-header">
          <span id="export-title" className="settings-title">
            {t("export.title")}
          </span>
          <button
            className="btn btn-icon btn-ghost"
            onClick={onClose}
            aria-label={t("a11y.close")}
          >
            {Icons.close}
          </button>
        </div>
        <div className="settings-body">
          {/* PDF/A Section */}
          <div className="settings-section">
            <div className="settings-section-title">{t("export.pdfa")}</div>
            <div className="settings-row">
              <select
                className="settings-select"
                value={exportPdfa}
                onChange={(e) =>
                  onPdfaChange(
                    e.target.value as "none" | "a1b" | "a2b"
                  )
                }
              >
                <option value="none">{t("export.pdfaNone")}</option>
                <option value="a1b">PDF/A-1b</option>
                <option value="a2b">PDF/A-2b</option>
              </select>
            </div>
          </div>

          {/* Encryption Section */}
          <div className="settings-section">
            <div className="settings-section-title">
              {t("export.encryption")}
            </div>
            <div className="settings-row">
              <label className="settings-label">
                {t("export.userPassword")}
              </label>
              <input
                type="password"
                className="settings-input"
                value={exportUserPassword}
                onChange={(e) => onUserPasswordChange(e.target.value)}
                placeholder={t("export.passwordPlaceholder")}
              />
            </div>
            <div className="settings-row">
              <label className="settings-label">
                {t("export.ownerPassword")}
              </label>
              <input
                type="password"
                className="settings-input"
                value={exportOwnerPassword}
                onChange={(e) => onOwnerPasswordChange(e.target.value)}
                placeholder={t("export.ownerPasswordPlaceholder")}
              />
            </div>
          </div>

          {/* Annotations info */}
          {hasAnnotations && (
            <div className="settings-section">
              <div className="settings-section-title">
                {t("export.annotations")}
              </div>
              <p className="export-info-text">
                {t("export.annotationCount", {
                  count: annotationCount,
                })}
              </p>
            </div>
          )}

          {/* Watermark/Signature status */}
          {(exportWatermarkEnabled || hasSignature) && (
            <div className="settings-section">
              <div className="settings-section-title">
                {t("export.overlays")}
              </div>
              {exportWatermarkEnabled && (
                <p className="export-info-text">
                  {t("export.watermarkActive", {
                    text: exportWatermarkText,
                  })}
                </p>
              )}
              {hasSignature && (
                <p className="export-info-text">
                  {t("export.signaturePlaced")}
                </p>
              )}
            </div>
          )}

          {/* Integrity Section */}
          <div className="settings-section">
            <div className="settings-section-title">
              {t("export.integrity")}
            </div>
            <p className="export-info-text">{t("export.integrityInfo")}</p>
          </div>
        </div>
        <div className="settings-footer">
          <button className="btn" onClick={onClose}>
            {t("settings.cancel")}
          </button>
          <button className="btn btn-accent" onClick={onExport}>
            {t("export.exportBtn")}
          </button>
        </div>
      </div>
    </div>
  );
}
