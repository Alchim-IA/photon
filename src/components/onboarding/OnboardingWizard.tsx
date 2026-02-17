import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation, type Language } from "../../contexts/LanguageContext";
import { useFocusTrap } from "../../hooks/useFocusTrap";
import { selectDirectory } from "../../utils/selectDirectory";

type WizardStep = "language" | "scanner" | "outputFolder" | "testScan" | "complete";
const STEPS: WizardStep[] = ["language", "scanner", "outputFolder", "testScan", "complete"];

interface ScannerDeviceBasic {
  id: string;
  name: string;
  vendor: string;
}

interface OnboardingWizardProps {
  onComplete: (partial: { language: Language; output_dir: string }) => void;
}

const StepIcon = ({ step, active }: { step: WizardStep; active: boolean }) => {
  const color = active ? "var(--accent)" : "var(--text-3)";
  const size = 48;
  switch (step) {
    case "language":
      return (
        <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="12" cy="12" r="10" /><path d="M2 12h20" /><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
        </svg>
      );
    case "scanner":
      return (
        <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
          <rect x="2" y="6" width="20" height="12" rx="2" /><path d="M6 12h12" /><path d="M12 6v12" />
        </svg>
      );
    case "outputFolder":
      return (
        <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
        </svg>
      );
    case "testScan":
      return (
        <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
          <polyline points="9 11 12 14 22 4" /><path d="M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11" />
        </svg>
      );
    case "complete":
      return (
        <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
          <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
        </svg>
      );
  }
};

export function OnboardingWizard({ onComplete }: OnboardingWizardProps) {
  const { t, language, setLanguage } = useTranslation();
  const [currentStep, setCurrentStep] = useState<WizardStep>("language");
  const [scanners, setScanners] = useState<ScannerDeviceBasic[]>([]);
  const [selectedScanner, setSelectedScanner] = useState("");
  const [outputDir, setOutputDir] = useState("");
  const [isScanning, setIsScanning] = useState(false);
  const [testScanDone, setTestScanDone] = useState(false);
  const [testScanImage, setTestScanImage] = useState<string | null>(null);
  const [testScanError, setTestScanError] = useState<string | null>(null);
  const [isLoadingScanners, setIsLoadingScanners] = useState(false);

  const dialogRef = useFocusTrap(true, () => {});
  const stepIndex = STEPS.indexOf(currentStep);

  useEffect(() => {
    if (currentStep === "scanner") {
      setIsLoadingScanners(true);
      invoke<ScannerDeviceBasic[]>("list_scanners")
        .then((list) => {
          setScanners(list);
          if (list.length > 0) setSelectedScanner(list[0].id);
        })
        .catch(() => setScanners([]))
        .finally(() => setIsLoadingScanners(false));
    }
    if (currentStep === "outputFolder") {
      invoke<{ output_dir: string }>("load_settings")
        .then((s) => setOutputDir(s.output_dir))
        .catch(() => {});
    }
  }, [currentStep]);

  const goNext = () => {
    const next = STEPS[stepIndex + 1];
    if (next) setCurrentStep(next);
  };

  const goPrev = () => {
    const prev = STEPS[stepIndex - 1];
    if (prev) setCurrentStep(prev);
  };

  const handleComplete = () => {
    onComplete({ language, output_dir: outputDir });
  };

  const runTestScan = async () => {
    if (!selectedScanner) return;
    setIsScanning(true);
    setTestScanError(null);
    try {
      const result = await invoke<{ image_base64: string }>("scan_document", {
        options: { device_id: selectedScanner, dpi: 300, color_mode: "Couleur", duplex: false, paper_format: "A4" },
      });
      setTestScanImage(`data:image/png;base64,${result.image_base64}`);
      setTestScanDone(true);
    } catch (err) {
      setTestScanError(String(err));
      setTestScanDone(true);
    } finally {
      setIsScanning(false);
    }
  };

  const handleSelectOutputDir = async () => {
    try {
      const dir = await selectDirectory();
      if (dir) setOutputDir(dir);
    } catch { /* Dialog unavailable */ }
  };

  return (
    <div className="onboarding-overlay" role="presentation">
      <div ref={dialogRef} className="onboarding-modal" role="dialog" aria-modal="true" aria-labelledby="onboarding-title">
        {/* Left rail — branding + step indicators */}
        <div className="onboarding-rail">
          <div className="onboarding-rail-brand">
            <img src="/logo.svg" alt="Photon" className="onboarding-rail-logo" />
            <span className="onboarding-rail-name">Photon</span>
          </div>
          <div className="onboarding-rail-steps">
            {STEPS.filter((s) => s !== "complete").map((step, i) => (
              <div key={step} className={`onboarding-rail-dot ${i < stepIndex ? "done" : i === stepIndex ? "active" : ""}`}>
                <div className="onboarding-rail-dot-inner" />
                {i < STEPS.length - 2 && <div className={`onboarding-rail-line ${i < stepIndex ? "done" : ""}`} />}
              </div>
            ))}
          </div>
        </div>

        {/* Right content */}
        <div className="onboarding-content">
          <div className="onboarding-body">
            {currentStep === "language" && (
              <div className="onboarding-step-anim">
                <div className="onboarding-step-icon"><StepIcon step="language" active /></div>
                <h2 id="onboarding-title" className="onboarding-step-title">{t("onboarding.welcome")}</h2>
                <p className="onboarding-step-desc">{t("onboarding.welcomeDesc")}</p>
                <div className="onboarding-field-label">{t("onboarding.languageStep")}</div>
                <div className="onboarding-lang-cards">
                  <button className={`onboarding-lang-card ${language === "fr" ? "active" : ""}`} onClick={() => setLanguage("fr")}>
                    <span className="onboarding-lang-flag">FR</span>
                    <span className="onboarding-lang-label">Francais</span>
                  </button>
                  <button className={`onboarding-lang-card ${language === "en" ? "active" : ""}`} onClick={() => setLanguage("en")}>
                    <span className="onboarding-lang-flag">EN</span>
                    <span className="onboarding-lang-label">English</span>
                  </button>
                </div>
              </div>
            )}

            {currentStep === "scanner" && (
              <div className="onboarding-step-anim">
                <div className="onboarding-step-icon"><StepIcon step="scanner" active /></div>
                <h2 className="onboarding-step-title">{t("onboarding.scannerStep")}</h2>
                {isLoadingScanners ? (
                  <div className="onboarding-loading">
                    <div className="onboarding-spinner" />
                    <span>{t("status.searchingScanners")}</span>
                  </div>
                ) : scanners.length === 0 ? (
                  <div className="onboarding-empty-state">
                    <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="var(--text-3)" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                      <circle cx="12" cy="12" r="10" /><path d="M12 8v4" /><path d="M12 16h.01" />
                    </svg>
                    <p>{t("onboarding.scannerNone")}</p>
                    <p className="onboarding-empty-hint">{t("onboarding.scannerNoneHint")}</p>
                    <button className="btn btn-sm" onClick={() => {
                      setIsLoadingScanners(true);
                      invoke<ScannerDeviceBasic[]>("list_scanners")
                        .then((list) => { setScanners(list); if (list.length > 0) setSelectedScanner(list[0].id); })
                        .catch(() => setScanners([]))
                        .finally(() => setIsLoadingScanners(false));
                    }}>{t("onboarding.scannerRefresh")}</button>
                  </div>
                ) : (
                  <div className="onboarding-scanner-list">
                    {scanners.map((s) => (
                      <button
                        key={s.id}
                        className={`onboarding-scanner-item ${selectedScanner === s.id ? "active" : ""}`}
                        onClick={() => setSelectedScanner(s.id)}
                      >
                        <div className="onboarding-scanner-dot" />
                        <div className="onboarding-scanner-info">
                          <span className="onboarding-scanner-name">{s.name}</span>
                          <span className="onboarding-scanner-vendor">{s.vendor}</span>
                        </div>
                      </button>
                    ))}
                  </div>
                )}
              </div>
            )}

            {currentStep === "outputFolder" && (
              <div className="onboarding-step-anim">
                <div className="onboarding-step-icon"><StepIcon step="outputFolder" active /></div>
                <h2 className="onboarding-step-title">{t("onboarding.outputStep")}</h2>
                <p className="onboarding-step-desc">{t("onboarding.outputDesc")}</p>
                <div className="onboarding-output-picker">
                  <input type="text" className="glass-input" value={outputDir} onChange={(e) => setOutputDir(e.target.value)} placeholder={t("settings.outputDirPlaceholder")} />
                  <button className="btn btn-accent" onClick={handleSelectOutputDir}>{t("settings.browse")}</button>
                </div>
              </div>
            )}

            {currentStep === "testScan" && (
              <div className="onboarding-step-anim">
                <div className="onboarding-step-icon"><StepIcon step="testScan" active /></div>
                <h2 className="onboarding-step-title">{t("onboarding.testScanStep")}</h2>
                <p className="onboarding-step-desc">{t("onboarding.testScanDesc")}</p>
                {!selectedScanner ? (
                  <p className="onboarding-muted">{t("onboarding.testScanNoScanner")}</p>
                ) : testScanImage ? (
                  <div className="onboarding-test-result">
                    <img src={testScanImage} alt="Test scan" className="onboarding-test-image" />
                    <div className="onboarding-test-success">
                      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                        <polyline points="20 6 9 17 4 12" />
                      </svg>
                      <span>{t("onboarding.testScanSuccess")}</span>
                    </div>
                  </div>
                ) : testScanError ? (
                  <div className="onboarding-test-error">
                    <p>{t("onboarding.testScanError")}</p>
                    <button className="btn btn-accent" onClick={runTestScan} disabled={isScanning}>
                      {t("onboarding.testScanRetry")}
                    </button>
                  </div>
                ) : (
                  <button className="btn btn-accent onboarding-test-btn" onClick={runTestScan} disabled={isScanning}>
                    {isScanning ? (
                      <><div className="onboarding-spinner" /> {t("status.scanning")}</>
                    ) : t("onboarding.testScanBtn")}
                  </button>
                )}
              </div>
            )}

            {currentStep === "complete" && (
              <div className="onboarding-step-anim onboarding-complete">
                <div className="onboarding-complete-check">
                  <svg width="56" height="56" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <polyline points="20 6 9 17 4 12" />
                  </svg>
                </div>
                <h2 className="onboarding-step-title">{t("onboarding.completeTitle")}</h2>
                <p className="onboarding-step-desc">{t("onboarding.completeDesc")}</p>
              </div>
            )}
          </div>

          {/* Footer */}
          <div className="onboarding-footer">
            {stepIndex > 0 && currentStep !== "complete" && (
              <button className="btn onboarding-back-btn" onClick={goPrev}>
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="15 18 9 12 15 6" /></svg>
                {t("onboarding.back")}
              </button>
            )}
            <div style={{ flex: 1 }} />
            {currentStep === "complete" ? (
              <button className="btn btn-accent onboarding-start-btn" onClick={handleComplete}>
                {t("onboarding.startUsing")}
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="9 18 15 12 9 6" /></svg>
              </button>
            ) : (
              <button className="btn btn-accent" onClick={goNext}>
                {currentStep === "testScan" && !testScanDone && selectedScanner ? t("onboarding.skip") : t("onboarding.next")}
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="9 18 15 12 9 6" /></svg>
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
