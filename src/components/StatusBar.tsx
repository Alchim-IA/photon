import { useTranslation } from "../contexts/LanguageContext";

interface StatusBarProps {
  statusMessage: string;
  statusType: "ready" | "scanning" | "error";
  scanProgress: number;
  isScanning: boolean;
  isAdjusting?: boolean;
}

export function StatusBar({
  statusMessage,
  statusType,
  scanProgress,
  isScanning,
  isAdjusting,
}: StatusBarProps) {
  const { t } = useTranslation();
  const clampedProgress = Math.round(Math.min(scanProgress, 100));

  return (
    <div className="status-bar" role="status">
      <div className={`status-dot ${statusType}`} aria-hidden="true" />
      <span className="status-text" aria-live="polite" aria-atomic="true">
        {statusMessage}
      </span>
      {isAdjusting && (
        <span className="status-text status-text-preview">
          {t("status.previewing")}
        </span>
      )}
      <div className="status-spacer" />
      {(isScanning || scanProgress > 0) && (
        <>
          <div
            className="progress-bar-track"
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={clampedProgress}
          >
            <div
              className="progress-bar-fill"
              style={{ width: `${clampedProgress}%` }}
            />
          </div>
          <span className="progress-text">{clampedProgress}%</span>
        </>
      )}
    </div>
  );
}
