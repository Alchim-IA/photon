import { useTranslation } from "../contexts/LanguageContext";
import Icons from "./Icons";
import type { ScannerDevice, ThemeMode } from "../types";

interface SidebarProps {
  scanners: ScannerDevice[];
  selectedScanner: string;
  onSelectScanner: (id: string) => void;
  onRefresh: () => void;
  isRefreshing: boolean;
  themeMode: ThemeMode;
  onThemeChange: (mode: ThemeMode) => void;
  activeNav?: string;
  onNavChange?: (nav: string) => void;
}

export function Sidebar({
  scanners,
  selectedScanner,
  onSelectScanner,
  onRefresh,
  isRefreshing,
  themeMode,
  onThemeChange,
}: SidebarProps) {
  const { t } = useTranslation();

  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        <div className="sidebar-logo">
          <img src="/logo.svg" alt="Photon" className="sidebar-logo-img" />
          <div>
            <div className="sidebar-title">{t("app.title")}</div>
            <div className="sidebar-subtitle">{t("app.subtitle")}</div>
          </div>
        </div>
      </div>

      <div className="sidebar-section">
        <div className="sidebar-section-header">
          <span className="sidebar-section-title">{t("sidebar.devices")}</span>
          <button
            className={`btn btn-icon btn-ghost ${isRefreshing ? "refreshing" : ""}`}
            onClick={onRefresh}
            aria-label={t("sidebar.refresh")}
            disabled={isRefreshing}
          >
            {Icons.refresh}
          </button>
        </div>

        <div
          id="sidebar-scanners"
          className="scanner-list"
          role="radiogroup"
          aria-label={t("a11y.scannerList")}
        >
          {scanners.length === 0 ? (
            <div className="scanner-empty">
              <div className="scanner-empty-icon">{Icons.scanner}</div>
              <p>{t("sidebar.noScannerTitle")}</p>
              <p className="scanner-empty-hint">{t("sidebar.noScannerHint")}</p>
            </div>
          ) : (
            scanners.map((scanner) => (
              <button
                key={scanner.id}
                role="radio"
                aria-checked={selectedScanner === scanner.id}
                className={`scanner-item ${selectedScanner === scanner.id ? "active" : ""}`}
                onClick={() => onSelectScanner(scanner.id)}
              >
                <div className="scanner-item-header">
                  <div className="scanner-status-dot online" />
                  <span className="scanner-name">{scanner.name}</span>
                </div>
                <div className="scanner-vendor">{scanner.vendor}</div>
              </button>
            ))
          )}
        </div>
      </div>

      <div className="theme-switcher">
        <button
          className={`theme-btn ${themeMode === "light" ? "active" : ""}`}
          onClick={() => onThemeChange("light")}
          title={t("sidebar.themeLight")}
        >
          {Icons.sun}
        </button>
        <button
          className={`theme-btn ${themeMode === "dark" ? "active" : ""}`}
          onClick={() => onThemeChange("dark")}
          title={t("sidebar.themeDark")}
        >
          {Icons.moon}
        </button>
        <button
          className={`theme-btn ${themeMode === "auto" ? "active" : ""}`}
          onClick={() => onThemeChange("auto")}
          title={t("sidebar.themeAuto")}
        >
          {Icons.auto} Auto
        </button>
      </div>
    </aside>
  );
}
