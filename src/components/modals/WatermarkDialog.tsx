import { useTranslation } from "../../contexts/LanguageContext";
import Icons from "../Icons";
import type { WatermarkPosition } from "../../types";

interface WatermarkDialogProps {
  show: boolean;
  onClose: () => void;
  text: string;
  onTextChange: (text: string) => void;
  opacity: number;
  onOpacityChange: (opacity: number) => void;
  rotation: number;
  onRotationChange: (rotation: number) => void;
  fontSize: number;
  onFontSizeChange: (size: number) => void;
  color: string;
  onColorChange: (color: string) => void;
  position: WatermarkPosition;
  onPositionChange: (position: WatermarkPosition) => void;
  onConfirm: () => void;
}

export function WatermarkDialog({
  show,
  onClose,
  text,
  onTextChange,
  opacity,
  onOpacityChange,
  rotation,
  onRotationChange,
  fontSize,
  onFontSizeChange,
  color,
  onColorChange,
  position,
  onPositionChange,
  onConfirm,
}: WatermarkDialogProps) {
  const { t } = useTranslation();

  if (!show) return null;

  return (
    <div
      className="settings-overlay"
      onClick={(e) => e.target === e.currentTarget && onClose()}
    >
      <div
        className="settings-modal watermark-dialog"
        role="dialog"
        aria-modal="true"
      >
        <div className="settings-header">
          <span className="settings-title">{t("export.watermark")}</span>
          <button
            className="btn btn-icon btn-ghost"
            onClick={onClose}
            aria-label={t("a11y.close")}
          >
            {Icons.close}
          </button>
        </div>
        <div className="settings-body">
          <div className="watermark-controls">
            <div className="settings-row">
              <label className="settings-label">
                {t("export.watermarkText")}
              </label>
              <input
                type="text"
                className="settings-input"
                value={text}
                onChange={(e) => onTextChange(e.target.value)}
                placeholder={t("export.watermarkTextPlaceholder")}
              />
            </div>
            <div className="settings-row">
              <label className="settings-label">
                {t("export.watermarkOpacity")}
              </label>
              <input
                type="range"
                min="0.05"
                max="1"
                step="0.05"
                value={opacity}
                onChange={(e) => onOpacityChange(parseFloat(e.target.value))}
              />
              <span className="watermark-value">
                {Math.round(opacity * 100)}%
              </span>
            </div>
            <div className="settings-row">
              <label className="settings-label">
                {t("export.watermarkRotation")}
              </label>
              <input
                type="range"
                min="-90"
                max="90"
                step="5"
                value={rotation}
                onChange={(e) =>
                  onRotationChange(parseInt(e.target.value))
                }
              />
              <span className="watermark-value">{rotation}°</span>
            </div>
            <div className="settings-row">
              <label className="settings-label">
                {t("export.watermarkPosition")}
              </label>
              <select
                className="settings-select"
                value={position}
                onChange={(e) =>
                  onPositionChange(e.target.value as WatermarkPosition)
                }
              >
                <option value="Diagonal">
                  {t("export.positionDiagonal")}
                </option>
                <option value="Center">
                  {t("export.positionCenter")}
                </option>
                <option value="TopLeft">
                  {t("export.positionTopLeft")}
                </option>
                <option value="TopRight">
                  {t("export.positionTopRight")}
                </option>
                <option value="BottomLeft">
                  {t("export.positionBottomLeft")}
                </option>
                <option value="BottomRight">
                  {t("export.positionBottomRight")}
                </option>
              </select>
            </div>
            <div className="settings-row">
              <label className="settings-label">
                {t("export.watermarkFontSize")}
              </label>
              <input
                type="number"
                className="settings-input watermark-fontsize-input"
                min={8}
                max={200}
                value={fontSize}
                onChange={(e) =>
                  onFontSizeChange(parseInt(e.target.value) || 48)
                }
              />
            </div>
            <div className="settings-row">
              <label className="settings-label">
                {t("export.watermarkColor")}
              </label>
              <input
                type="color"
                value={color}
                onChange={(e) => onColorChange(e.target.value)}
              />
            </div>
          </div>
        </div>
        <div className="settings-footer">
          <button className="btn" onClick={onClose}>
            {t("settings.cancel")}
          </button>
          <button className="btn btn-accent" onClick={onConfirm}>
            {t("export.watermarkConfirm")}
          </button>
        </div>
      </div>
    </div>
  );
}
