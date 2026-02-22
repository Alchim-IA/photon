import React from "react";
import { useTranslation } from "../../contexts/LanguageContext";
import Icons from "../Icons";

interface SignatureDialogProps {
  show: boolean;
  onClose: () => void;
  canvasRef: React.RefObject<HTMLCanvasElement | null>;
  onClear: () => void;
  onImport: () => void;
  onSave: () => void;
  onCanvasMouseDown: (e: React.MouseEvent<HTMLCanvasElement>) => void;
  onCanvasMouseMove: (e: React.MouseEvent<HTMLCanvasElement>) => void;
  onCanvasMouseUp: () => void;
}

export function SignatureDialog({
  show,
  onClose,
  canvasRef,
  onClear,
  onImport,
  onSave,
  onCanvasMouseDown,
  onCanvasMouseMove,
  onCanvasMouseUp,
}: SignatureDialogProps) {
  const { t } = useTranslation();

  if (!show) return null;

  return (
    <div
      className="settings-overlay"
      onClick={(e) => e.target === e.currentTarget && onClose()}
    >
      <div
        className="settings-modal signature-dialog"
        role="dialog"
        aria-modal="true"
      >
        <div className="settings-header">
          <span className="settings-title">{t("export.signatureTitle")}</span>
          <button
            className="btn btn-icon btn-ghost"
            onClick={onClose}
            aria-label={t("a11y.close")}
          >
            {Icons.close}
          </button>
        </div>
        <div className="settings-body signature-dialog-body">
          <p className="signature-hint">{t("export.signatureDrawHint")}</p>
          <canvas
            ref={canvasRef}
            className="signature-canvas"
            width={400}
            height={180}
            onMouseDown={onCanvasMouseDown}
            onMouseMove={onCanvasMouseMove}
            onMouseUp={onCanvasMouseUp}
            onMouseLeave={onCanvasMouseUp}
          />
          <div className="signature-actions">
            <button className="btn" onClick={onClear}>
              {t("export.signatureClear")}
            </button>
            <button className="btn" onClick={onImport}>
              {t("export.signatureImport")}
            </button>
          </div>
        </div>
        <div className="settings-footer">
          <button className="btn" onClick={onClose}>
            {t("settings.cancel")}
          </button>
          <button className="btn btn-accent" onClick={onSave}>
            {t("export.signatureConfirm")}
          </button>
        </div>
      </div>
    </div>
  );
}
