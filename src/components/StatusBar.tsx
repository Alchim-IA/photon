import { useState, useEffect } from "react";
import { useTranslation } from "../contexts/LanguageContext";
import { logger, type LogEntry } from "../utils/debugLogger";
import Icons from "./Icons";

interface StatusBarProps {
  statusMessage: string;
  statusType: "ready" | "scanning" | "error";
  scanProgress: number;
  isScanning: boolean;
  isAdjusting?: boolean;
  onToggleDebug: () => void;
  showDebugPanel: boolean;
}

export function StatusBar({
  statusMessage,
  statusType,
  scanProgress,
  isScanning,
  isAdjusting,
  onToggleDebug,
  showDebugPanel,
}: StatusBarProps) {
  const { t } = useTranslation();
  const clampedProgress = Math.round(Math.min(scanProgress, 100));
  const [errorCount, setErrorCount] = useState(0);

  useEffect(() => {
    setErrorCount(logger.getEntries().filter((e) => e.level === "error").length);
    const unsub = logger.subscribe((entry: LogEntry) => {
      if (entry.level === "error") {
        setErrorCount((c) => c + 1);
      }
    });
    return unsub;
  }, []);

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
      <button
        className={`debug-toggle-btn ${showDebugPanel ? "active" : ""}`}
        onClick={onToggleDebug}
        title={`${t("debug.openDebug")} (F12)`}
      >
        {Icons.bug}
        {errorCount > 0 && <span className="debug-badge debug-badge-error">{errorCount}</span>}
      </button>
    </div>
  );
}
