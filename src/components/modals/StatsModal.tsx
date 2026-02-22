import { useTranslation } from "../../contexts/LanguageContext";
import Icons from "../Icons";
import type { AppStats } from "../../types";

interface StatsModalProps {
  show: boolean;
  onClose: () => void;
  stats: AppStats | null;
}

export function StatsModal({ show, onClose, stats }: StatsModalProps) {
  const { t } = useTranslation();

  if (!show || !stats) return null;

  return (
    <div
      className="settings-overlay"
      onClick={(e) => e.target === e.currentTarget && onClose()}
    >
      <div
        className="settings-modal stats-modal"
        role="dialog"
        aria-modal="true"
      >
        <div className="settings-header">
          <span className="settings-title">
            {Icons.chart} {t("stats.title")}
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
          <div className="settings-group">
            <div className="settings-row">
              <div className="settings-row-label">
                {t("stats.totalScans")}
              </div>
              <span>{stats.total_scans}</span>
            </div>
            <div className="settings-row">
              <div className="settings-row-label">
                {t("stats.totalExports")}
              </div>
              <span>{stats.total_exports}</span>
            </div>
            <div className="settings-row">
              <div className="settings-row-label">
                {t("stats.totalOcr")}
              </div>
              <span>{stats.total_ocr_runs}</span>
            </div>
            <div className="settings-row">
              <div className="settings-row-label">
                {t("stats.totalPages")}
              </div>
              <span>{stats.total_pages_scanned}</span>
            </div>
          </div>
          {Object.keys(stats.formats_used || {}).length > 0 && (
            <div className="settings-group">
              <div className="settings-row-label stats-formats-label">
                {t("stats.formatsUsed")}
              </div>
              {Object.entries(stats.formats_used).map(([fmt, count]) => (
                <div key={fmt} className="settings-row">
                  <span>{fmt}</span>
                  <span>{count}</span>
                </div>
              ))}
            </div>
          )}
        </div>
        <div className="settings-footer">
          <button className="btn" onClick={onClose}>
            {t("settings.cancel")}
          </button>
        </div>
      </div>
    </div>
  );
}
