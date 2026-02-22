import { useTranslation } from "../../contexts/LanguageContext";
import Icons from "../Icons";
import type { SensitiveInfo } from "../../types";

interface RedactionModalProps {
  show: boolean;
  onClose: () => void;
  items: SensitiveInfo[];
  onApplyRedactions: () => void;
}

export function RedactionModal({
  show,
  onClose,
  items,
  onApplyRedactions,
}: RedactionModalProps) {
  const { t } = useTranslation();

  if (!show || items.length === 0) return null;

  return (
    <div
      className="settings-overlay"
      onClick={(e) => e.target === e.currentTarget && onClose()}
    >
      <div
        className="settings-modal redaction-modal"
        role="dialog"
        aria-modal="true"
      >
        <div className="settings-header">
          <span className="settings-title">
            {Icons.redact} {t("ai.redactionTitle")}
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
          <p className="redaction-desc">
            {t("ai.redactionDesc", { count: items.length })}
          </p>
          <div className="redaction-list">
            {items.map((item, i) => (
              <div key={i} className="rule-card redaction-item">
                <span className="rule-cond-chip">{item.category}</span>{" "}
                {item.text}
              </div>
            ))}
          </div>
        </div>
        <div className="settings-footer">
          <button className="btn" onClick={onClose}>
            {t("edit.cancel")}
          </button>
          <button className="btn btn-accent" onClick={onApplyRedactions}>
            {Icons.redact} {t("ai.applyRedactions")}
          </button>
        </div>
      </div>
    </div>
  );
}
