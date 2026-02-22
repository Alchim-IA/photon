import { useTranslation } from "../contexts/LanguageContext";
import Icons from "./Icons";
import type { ScanConfig, ScanProfile, ScannerDevice } from "../types";

interface ConfigPanelProps {
  config: ScanConfig;
  onConfigChange: (config: ScanConfig) => void;
  scanProfiles: ScanProfile[];
  selectedProfileId: string | null;
  onSelectProfile: (profile: ScanProfile) => void;
  onSaveProfile: (name: string) => void;
  onDeleteProfile: (profileId: string) => void;
  scanners: ScannerDevice[];
  selectedScanner: string;
  batchMode: boolean;
  batchPageCount: number;
  onBatchModeChange: (enabled: boolean) => void;
  onBatchPageCountChange: (count: number) => void;
  onBatchScan: () => void;
  isScanning: boolean;
}

export function ConfigPanel({
  config,
  onConfigChange,
  scanProfiles,
  selectedProfileId,
  onSelectProfile,
  onSaveProfile,
  onDeleteProfile,
  scanners,
  selectedScanner,
  batchMode,
  batchPageCount,
  onBatchModeChange,
  onBatchPageCountChange,
  onBatchScan,
  isScanning,
}: ConfigPanelProps) {
  const { t } = useTranslation();
  const currentScanner = scanners.find((s) => s.id === selectedScanner);
  const dpiOptions = currentScanner?.capabilities.resolutions ?? [150, 300, 600, 1200];
  const colorOptions = currentScanner?.capabilities.color_modes ?? [
    "Couleur",
    "Niveaux de gris",
    "Noir et blanc",
  ];

  return (
    <>
      <div className="config-header">{t("config.header")}</div>

      {scanProfiles.length > 0 && (
        <div className="config-section">
          <div className="config-label">{t("config.profiles")}</div>
          <div className="chip-group">
            {scanProfiles.map((p) => (
              <button
                key={p.id}
                className={`chip ${selectedProfileId === p.id ? "active" : ""}`}
                onClick={() => onSelectProfile(p)}
                onContextMenu={(e) => {
                  e.preventDefault();
                  onDeleteProfile(p.id);
                }}
                title={t("config.profileTooltip", {
                  dpi: p.dpi,
                  mode: p.color_mode,
                })}
              >
                {Icons.profile} {p.name}
              </button>
            ))}
          </div>
        </div>
      )}

      <div className="config-section">
        <button
          className="btn btn-sm"
          onClick={() => {
            const name = prompt(t("config.profileNamePrompt"));
            if (name?.trim()) onSaveProfile(name.trim());
          }}
        >
          {Icons.profile} {t("config.saveAsProfile")}
        </button>
      </div>

      <div className="config-section">
        <div className="config-label">{t("config.resolution")}</div>
        <div className="chip-group">
          {dpiOptions.map((dpi) => (
            <button
              key={dpi}
              className={`chip ${config.dpi === dpi ? "active" : ""}`}
              onClick={() => onConfigChange({ ...config, dpi })}
            >
              {dpi}
            </button>
          ))}
        </div>
      </div>

      <div className="config-section">
        <div className="config-label">{t("config.colorMode")}</div>
        <div className="chip-group">
          {colorOptions.map((mode) => (
            <button
              key={mode}
              className={`chip ${config.colorMode === mode ? "active" : ""}`}
              onClick={() => onConfigChange({ ...config, colorMode: mode })}
            >
              {t(`colorModes.${mode}`) || mode}
            </button>
          ))}
        </div>
      </div>

      <div className="config-section">
        <div className="config-label">{t("config.paperFormat")}</div>
        <div className="select-wrapper">
          <select
            className="glass-select"
            value={config.paperFormat}
            onChange={(e) =>
              onConfigChange({ ...config, paperFormat: e.target.value })
            }
          >
            <option value="A4">A4 (210 x 297 mm)</option>
            <option value="A3">A3 (297 x 420 mm)</option>
            <option value="Letter">Letter (216 x 279 mm)</option>
            <option value="Legal">Legal (216 x 356 mm)</option>
          </select>
          <div className="select-arrow">{Icons.chevronDown}</div>
        </div>
      </div>

      <div className="config-section">
        <div className="config-label">{t("config.options")}</div>
        <div className="toggle-row">
          <span className="toggle-label">{t("config.duplex")}</span>
          <input
            type="checkbox"
            className="toggle"
            checked={config.duplex}
            onChange={(e) =>
              onConfigChange({ ...config, duplex: e.target.checked })
            }
            disabled={!currentScanner?.capabilities.supports_duplex}
          />
        </div>
        <div className="toggle-row">
          <span className="toggle-label">{t("config.adf")}</span>
          <input
            type="checkbox"
            className="toggle"
            checked={config.adf}
            onChange={(e) =>
              onConfigChange({ ...config, adf: e.target.checked })
            }
            disabled={!currentScanner?.capabilities.supports_adf}
          />
        </div>
      </div>

      <div className="config-section">
        <div className="config-label">{t("config.batchScan")}</div>
        <div className="toggle-row">
          <span className="toggle-label">{t("config.batchMode")}</span>
          <input
            type="checkbox"
            className="toggle"
            checked={batchMode}
            onChange={(e) => onBatchModeChange(e.target.checked)}
          />
        </div>
        {batchMode && (
          <div className="batch-controls">
            <div className="adjustment-slider-header">
              <span>{t("config.pageCount")}</span>
              <span className="adjustment-value">{batchPageCount}</span>
            </div>
            <input
              type="range"
              className="glass-range"
              min={2}
              max={50}
              step={1}
              value={batchPageCount}
              onChange={(e) => onBatchPageCountChange(Number(e.target.value))}
            />
            <button
              className="btn btn-sm btn-accent batch-scan-btn"
              onClick={onBatchScan}
              disabled={isScanning || !selectedScanner}
            >
              {Icons.batch} {t("config.scanPages", { count: batchPageCount })}
            </button>
          </div>
        )}
      </div>
    </>
  );
}
