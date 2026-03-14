import { useState, useCallback } from "react";
import { useTranslation, type Language } from "../../contexts/LanguageContext";
import { useFocusTrap } from "../../hooks/useFocusTrap";
import Icons from "../Icons";
import type { AppSettings, ThemeMode } from "../../types";

interface SettingsModalProps {
  show: boolean;
  onClose: () => void;
  settings: AppSettings;
  onSettingsChange: (settings: AppSettings) => void;
  onSaveSettings: () => void;
  onSelectOutputDir: () => void;
  onSelectWatchFolder: () => void;
  language: Language;
  onLanguageChange: (lang: Language) => void;
  themeMode: ThemeMode;
  onThemeChange: (mode: ThemeMode) => void;
}

export function SettingsModal({
  show,
  onClose,
  settings,
  onSettingsChange,
  onSaveSettings,
  onSelectOutputDir,
  onSelectWatchFolder,
  language,
  onLanguageChange,
}: SettingsModalProps) {
  const { t } = useTranslation();
  const [settingsTab, setSettingsTab] = useState<
    "general" | "scan" | "export" | "app" | "about"
  >("general");
  const closeHandler = useCallback(() => onClose(), [onClose]);
  const modalRef = useFocusTrap(show, closeHandler);

  if (!show) return null;

  return (
    <div
      className="settings-overlay"
      onClick={(e) => e.target === e.currentTarget && onClose()}
    >
      <div
        ref={modalRef}
        className="settings-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="settings-title"
      >
        <div className="settings-header">
          <div className="settings-header-left">
            <img src="/logo.svg" alt="" className="settings-logo" />
            <div>
              <span id="settings-title" className="settings-title">
                {t("settings.title")}
              </span>
              <div className="settings-version">Photon v1.0.1</div>
            </div>
          </div>
          <button
            className="btn btn-icon btn-ghost"
            onClick={onClose}
            aria-label={t("a11y.close")}
          >
            {Icons.close}
          </button>
        </div>

        <div className="settings-tabs" role="tablist">
          {(["general", "scan", "export", "app", "about"] as const).map((tab) => (
            <button
              key={tab}
              role="tab"
              aria-selected={settingsTab === tab}
              className={`settings-tab ${settingsTab === tab ? "active" : ""}`}
              onClick={() => setSettingsTab(tab)}
            >
              {t(`settings.tab.${tab}`)}
            </button>
          ))}
        </div>

        <div className="settings-body">
          {settingsTab === "general" && (
            <>
              <div className="settings-group">
                <div className="settings-row-label settings-row-label-mb">
                  {t("settings.outputDir")}
                </div>
                <div className="settings-dir-row">
                  <input
                    type="text"
                    className="glass-input"
                    value={settings.output_dir}
                    onChange={(e) =>
                      onSettingsChange({
                        ...settings,
                        output_dir: e.target.value,
                      })
                    }
                    placeholder={t("settings.outputDirPlaceholder")}
                  />
                  <button
                    className="btn btn-icon"
                    onClick={onSelectOutputDir}
                    aria-label={t("settings.browse")}
                  >
                    {Icons.folder}
                  </button>
                </div>
              </div>
              <div className="settings-group">
                <div className="settings-row">
                  <div>
                    <div className="settings-row-label">
                      {t("settings.defaultFormat")}
                    </div>
                    <div className="settings-row-desc">
                      {t("settings.defaultFormatDesc")}
                    </div>
                  </div>
                  <select
                    className="glass-select settings-select-sm"
                    value={settings.default_format}
                    onChange={(e) =>
                      onSettingsChange({
                        ...settings,
                        default_format: e.target.value,
                      })
                    }
                  >
                    <option value="PDF">PDF</option>
                    <option value="PNG">PNG</option>
                    <option value="JPEG">JPEG</option>
                    <option value="TIFF">TIFF</option>
                  </select>
                </div>
              </div>
              <div className="settings-group">
                <div className="settings-row">
                  <div>
                    <div className="settings-row-label">
                      {t("settings.autoCrop")}
                    </div>
                    <div className="settings-row-desc">
                      {t("settings.autoCropDesc")}
                    </div>
                  </div>
                  <input
                    type="checkbox"
                    className="toggle"
                    checked={settings.auto_crop}
                    onChange={(e) =>
                      onSettingsChange({
                        ...settings,
                        auto_crop: e.target.checked,
                      })
                    }
                  />
                </div>
              </div>
            </>
          )}

          {settingsTab === "scan" && (
            <>
              <div className="settings-group">
                <div className="settings-row">
                  <div className="settings-row-label">
                    {t("settings.resolution")}
                  </div>
                  <select
                    className="glass-select settings-select-sm"
                    value={settings.default_dpi}
                    onChange={(e) =>
                      onSettingsChange({
                        ...settings,
                        default_dpi: Number(e.target.value),
                      })
                    }
                  >
                    <option value={150}>150 DPI</option>
                    <option value={300}>300 DPI</option>
                    <option value={600}>600 DPI</option>
                    <option value={1200}>1200 DPI</option>
                  </select>
                </div>
                <div className="settings-row">
                  <div className="settings-row-label">
                    {t("settings.colorMode")}
                  </div>
                  <select
                    className="glass-select settings-select-md"
                    value={settings.default_color_mode}
                    onChange={(e) =>
                      onSettingsChange({
                        ...settings,
                        default_color_mode: e.target.value,
                      })
                    }
                  >
                    <option value="Couleur">
                      {t("colorModes.Couleur")}
                    </option>
                    <option value="Niveaux de gris">
                      {t("colorModes.Niveaux de gris")}
                    </option>
                    <option value="Noir et blanc">
                      {t("colorModes.Noir et blanc")}
                    </option>
                  </select>
                </div>
                <div className="settings-row">
                  <div className="settings-row-label">
                    {t("settings.paperFormat")}
                  </div>
                  <select
                    className="glass-select settings-select-sm"
                    value={settings.default_paper_format}
                    onChange={(e) =>
                      onSettingsChange({
                        ...settings,
                        default_paper_format: e.target.value,
                      })
                    }
                  >
                    <option value="A4">A4</option>
                    <option value="A3">A3</option>
                    <option value="Letter">Letter</option>
                    <option value="Legal">Legal</option>
                  </select>
                </div>
              </div>
              <div className="settings-group">
                <div className="settings-row settings-row-mb">
                  <div className="settings-row-label">
                    {t("settings.quality")}
                  </div>
                  <span className="range-value">{settings.quality}%</span>
                </div>
                <div className="range-wrapper">
                  <input
                    type="range"
                    className="glass-range"
                    min={10}
                    max={100}
                    step={5}
                    value={settings.quality}
                    onChange={(e) =>
                      onSettingsChange({
                        ...settings,
                        quality: Number(e.target.value),
                      })
                    }
                  />
                </div>
              </div>
              <div className="settings-group">
                <div className="settings-row">
                  <div>
                    <div className="settings-row-label">
                      {t("settings.autoOcr")}
                    </div>
                    <div className="settings-row-desc">
                      {t("settings.autoOcrDesc")}
                    </div>
                  </div>
                  <input
                    type="checkbox"
                    className="toggle"
                    checked={settings.auto_ocr}
                    onChange={(e) =>
                      onSettingsChange({
                        ...settings,
                        auto_ocr: e.target.checked,
                      })
                    }
                  />
                </div>
                <div className="settings-row">
                  <div className="settings-row-label">
                    {t("settings.ocrLanguage")}
                  </div>
                  <select
                    className="glass-select settings-select-md"
                    value={settings.default_ocr_lang}
                    onChange={(e) =>
                      onSettingsChange({
                        ...settings,
                        default_ocr_lang: e.target.value,
                      })
                    }
                  >
                    <option value="fra">Français</option>
                    <option value="eng">English</option>
                    <option value="deu">Deutsch</option>
                    <option value="spa">Español</option>
                    <option value="ita">Italiano</option>
                    <option value="por">Português</option>
                    <option value="nld">Nederlands</option>
                    <option value="fra+eng">Français + English</option>
                  </select>
                </div>
              </div>
            </>
          )}

          {settingsTab === "export" && (
            <>
              <div className="settings-group">
                <div className="settings-row-label settings-row-label-mb-sm">
                  {t("settings.namingTemplate")}
                </div>
                <div className="settings-row-desc settings-row-desc-mb">
                  {t("settings.namingTemplateDesc")}
                </div>
                <input
                  type="text"
                  className="glass-input"
                  value={settings.naming_template}
                  onChange={(e) =>
                    onSettingsChange({
                      ...settings,
                      naming_template: e.target.value,
                    })
                  }
                  placeholder="Scan_{date}_{time}"
                />
              </div>
              <div className="settings-group">
                <div className="settings-row-label settings-row-label-mb-sm">
                  {t("settings.watchFolder")}
                </div>
                <div className="settings-row-desc settings-row-desc-mb">
                  {t("settings.watchFolderDesc")}
                </div>
                <div className="settings-dir-row">
                  <input
                    type="text"
                    className="glass-input"
                    value={settings.watch_folder ?? ""}
                    onChange={(e) =>
                      onSettingsChange({
                        ...settings,
                        watch_folder: e.target.value || null,
                      })
                    }
                    placeholder={t("settings.watchFolderPlaceholder")}
                  />
                  <button
                    className="btn btn-icon"
                    onClick={onSelectWatchFolder}
                    aria-label={t("settings.browse")}
                  >
                    {Icons.folder}
                  </button>
                </div>
              </div>
            </>
          )}

          {settingsTab === "app" && (
            <>
              <div className="settings-group">
                <div className="settings-row">
                  <div className="settings-row-label">
                    {t("settings.language")}
                  </div>
                  <select
                    className="glass-select settings-select-md"
                    value={language}
                    onChange={(e) =>
                      onLanguageChange(e.target.value as Language)
                    }
                  >
                    <option value="fr">Français</option>
                    <option value="en">English</option>
                  </select>
                </div>
              </div>
              <div className="settings-group">
                <div className="settings-row-label settings-row-label-mb-sm">
                  {t("settings.groqApiKey")}
                </div>
                <div className="settings-row-desc settings-row-desc-mb">
                  {t("settings.groqApiKeyDesc")}
                </div>
                <input
                  type="password"
                  className="glass-input"
                  value={settings.groq_api_key ?? ""}
                  onChange={(e) =>
                    onSettingsChange({
                      ...settings,
                      groq_api_key: e.target.value || null,
                    })
                  }
                  placeholder="gsk_..."
                />
              </div>
            </>
          )}

          {settingsTab === "about" && (
            <div className="settings-about">
              <div className="settings-about-logo">
                <img src="/logo.svg" alt="Photon" style={{ width: 64, height: 64 }} />
              </div>
              <h2 className="settings-about-name">Photon</h2>
              <div className="settings-about-version">v1.0.1</div>
              <p className="settings-about-desc">{t("about.description")}</p>
              <div className="settings-about-changelog">
                <div className="settings-about-changelog-title">{t("about.changelog")}</div>
                <div className="settings-about-entry">
                  <span className="settings-about-badge">v1.0.1</span>
                  <ul>
                    <li>{t("about.v101.wiaFix")}</li>
                    <li>{t("about.v101.errorMessages")}</li>
                    <li>{t("about.v101.debugLogs")}</li>
                    <li>{t("about.v101.aboutPage")}</li>
                  </ul>
                </div>
                <div className="settings-about-entry">
                  <span className="settings-about-badge">v1.0.0</span>
                  <ul>
                    <li>{t("about.v100.initial")}</li>
                  </ul>
                </div>
              </div>
              <div className="settings-about-footer">
                <span>{t("about.madeBy")}</span>
                <a href="https://github.com/Alchim-IA/photon" target="_blank" rel="noopener noreferrer" className="settings-about-link">GitHub</a>
              </div>
            </div>
          )}
        </div>

        <div className="settings-footer">
          <button className="btn" onClick={onClose}>
            {t("settings.cancel")}
          </button>
          <button className="btn btn-accent" onClick={onSaveSettings}>
            {t("settings.save")}
          </button>
        </div>
      </div>
    </div>
  );
}
