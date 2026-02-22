import { useTranslation } from "../contexts/LanguageContext";
import Icons from "./Icons";
import type { ImageAdjustments } from "../types";

interface EditPanelProps {
  adjustments: ImageAdjustments;
  onAdjustmentChange: (key: keyof ImageAdjustments, value: number) => void;
  onApplyAdjustments: () => void;
  onRevertAdjustments: () => void;
  isAdjusting: boolean;
  onRotate: (direction: string) => void;
  onFlip: (axis: string) => void;
  onDeskew: () => void;
  onWhiten: () => void;
  onDenoise: (strength: number) => void;
  hasDocument: boolean;
}

export function EditPanel({
  adjustments,
  onAdjustmentChange,
  onApplyAdjustments,
  onRevertAdjustments,
  onRotate,
  onFlip,
  onDeskew,
  onWhiten,
  onDenoise,
  hasDocument,
}: EditPanelProps) {
  const { t } = useTranslation();
  const hasAdjustments =
    adjustments.brightness !== 0 ||
    adjustments.contrast !== 0 ||
    adjustments.saturation !== 0 ||
    adjustments.sharpness !== 0;

  return (
    <>
      <div className="config-header">{t("edit.header")}</div>

      <div className="config-section">
        <div className="config-label">{t("edit.rotationFlip")}</div>
        <div className="edit-btn-row">
          <button
            className="btn btn-sm"
            onClick={() => onRotate("270")}
            disabled={!hasDocument}
            title={t("edit.rotateLeft")}
          >
            {Icons.rotateLeft}
          </button>
          <button
            className="btn btn-sm"
            onClick={() => onRotate("90")}
            disabled={!hasDocument}
            title={t("edit.rotateRight")}
          >
            {Icons.rotateRight}
          </button>
          <button
            className="btn btn-sm"
            onClick={() => onRotate("180")}
            disabled={!hasDocument}
            title={t("edit.rotate180")}
          >
            180°
          </button>
          <button
            className="btn btn-sm"
            onClick={() => onFlip("horizontal")}
            disabled={!hasDocument}
            title={t("edit.flipH")}
          >
            {Icons.flipH}
          </button>
          <button
            className="btn btn-sm"
            onClick={() => onFlip("vertical")}
            disabled={!hasDocument}
            title={t("edit.flipV")}
          >
            {Icons.flipV}
          </button>
        </div>
      </div>

      <div className="config-section">
        <div className="config-label">{t("edit.adjustments")}</div>
        {(["brightness", "contrast", "saturation", "sharpness"] as const).map(
          (key) => (
            <div className="adjustment-slider" key={key}>
              <div className="adjustment-slider-header">
                <label htmlFor={`slider-${key}`}>{t(`edit.${key}`)}</label>
                <span className="adjustment-value" aria-hidden="true">
                  {key === "sharpness"
                    ? adjustments[key]
                    : `${adjustments[key] > 0 ? "+" : ""}${adjustments[key]}`}
                </span>
              </div>
              <input
                id={`slider-${key}`}
                type="range"
                className="glass-range"
                min={key === "sharpness" ? 0 : -100}
                max={100}
                step={1}
                value={adjustments[key]}
                onChange={(e) =>
                  onAdjustmentChange(key, Number(e.target.value))
                }
                disabled={!hasDocument}
                aria-label={t(`edit.${key}`)}
              />
            </div>
          )
        )}
        {hasAdjustments && (
          <div className="edit-btn-row edit-btn-row-actions">
            <button className="btn btn-sm" onClick={onRevertAdjustments}>
              {Icons.undo} {t("edit.cancel")}
            </button>
            <button
              className="btn btn-sm btn-accent"
              onClick={onApplyAdjustments}
            >
              {Icons.check} {t("edit.apply")}
            </button>
          </div>
        )}
      </div>

      <div className="config-section">
        <div className="config-label">{t("edit.processing")}</div>
        <div className="edit-action-list">
          <button
            className="btn btn-sm edit-action-btn"
            onClick={onDeskew}
            disabled={!hasDocument}
          >
            {Icons.deskew}
            <span>{t("edit.deskew")}</span>
          </button>
          <button
            className="btn btn-sm edit-action-btn"
            onClick={onWhiten}
            disabled={!hasDocument}
          >
            {Icons.whiten}
            <span>{t("edit.whitenBg")}</span>
          </button>
          <button
            className="btn btn-sm edit-action-btn"
            onClick={() => onDenoise(1)}
            disabled={!hasDocument}
          >
            {Icons.noise}
            <span>{t("edit.denoiseLight")}</span>
          </button>
          <button
            className="btn btn-sm edit-action-btn"
            onClick={() => onDenoise(2)}
            disabled={!hasDocument}
          >
            {Icons.noise}
            <span>{t("edit.denoiseStrong")}</span>
          </button>
        </div>
      </div>
    </>
  );
}
