import { useTranslation } from "../contexts/LanguageContext";

interface DropOverlayProps {
  isDragOver: boolean;
  isImporting: boolean;
}

export function DropOverlay({ isDragOver, isImporting }: DropOverlayProps) {
  const { t } = useTranslation();

  if (isImporting) {
    return (
      <div className="drop-overlay importing" aria-hidden="true">
        <div className="drop-overlay-content">
          <div className="import-spinner" />
          <h2>{t("status.importing")}</h2>
        </div>
      </div>
    );
  }

  if (isDragOver) {
    return (
      <div className="drop-overlay" aria-hidden="true">
        <div className="drop-overlay-content">
          <svg
            width="64"
            height="64"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
            <polyline points="17 8 12 3 7 8" />
            <line x1="12" y1="3" x2="12" y2="15" />
          </svg>
          <h2>{t("dropzone.title")}</h2>
          <p>{t("dropzone.subtitle")}</p>
        </div>
      </div>
    );
  }

  return null;
}
