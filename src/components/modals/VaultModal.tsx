import { useTranslation } from "../../contexts/LanguageContext";
import Icons from "../Icons";
import type { VaultDocument } from "../../types";

interface VaultModalProps {
  show: boolean;
  onClose: () => void;
  vaultUnlocked: boolean;
  vaultSetup: boolean;
  vaultPassword: string;
  onPasswordChange: (password: string) => void;
  onUnlock: () => void;
  onLock: () => void;
  onSetupPassword: () => void;
  vaultDocs: VaultDocument[];
  vaultViewMode: "grid" | "list";
  onViewModeChange: (mode: "grid" | "list") => void;
  vaultFilter: string;
  onFilterChange: (filter: string) => void;
  onAddDocument: () => void;
  onRemoveDocument: (docId: string) => void;
  onOpenDocument: (docId: string) => void;
}

export function VaultModal({
  show,
  onClose,
  vaultUnlocked,
  vaultSetup,
  vaultPassword,
  onPasswordChange,
  onUnlock,
  onLock,
  vaultDocs,
  vaultViewMode,
  onViewModeChange,
  vaultFilter,
  onFilterChange,
  onRemoveDocument,
  onOpenDocument,
}: VaultModalProps) {
  const { t } = useTranslation();

  if (!show) return null;

  const filteredDocs = vaultDocs.filter((d) => {
    if (!vaultFilter) return true;
    const q = vaultFilter.toLowerCase();
    return (
      d.name.toLowerCase().includes(q) ||
      d.format.toLowerCase().includes(q) ||
      d.date.includes(q)
    );
  });

  return (
    <div className="vault-fullpage">
      <div className="vault-header">
        <div className="vault-header-left">
          {Icons.vault}
          <span className="vault-title">{t("vault.title")}</span>
          {vaultUnlocked && (
            <span className="vault-count">
              {filteredDocs.length} {t("vault.documents")}
            </span>
          )}
        </div>
        <div className="vault-header-right">
          {vaultUnlocked && (
            <>
              <input
                className="glass-input vault-search"
                value={vaultFilter}
                onChange={(e) => onFilterChange(e.target.value)}
                placeholder={t("vault.filterPlaceholder")}
              />
              <div className="vault-view-toggle">
                <button
                  className={`btn btn-icon btn-sm ${vaultViewMode === "grid" ? "btn-accent" : "btn-ghost"}`}
                  onClick={() => onViewModeChange("grid")}
                  aria-label="Grid"
                >
                  {Icons.grid}
                </button>
                <button
                  className={`btn btn-icon btn-sm ${vaultViewMode === "list" ? "btn-accent" : "btn-ghost"}`}
                  onClick={() => onViewModeChange("list")}
                  aria-label="List"
                >
                  {Icons.list}
                </button>
              </div>
              <button className="btn btn-sm" onClick={onLock}>
                {Icons.vault} {t("vault.lock")}
              </button>
            </>
          )}
          <button className="btn btn-icon btn-ghost" onClick={onClose}>
            {Icons.close}
          </button>
        </div>
      </div>
      <div className="vault-body">
        {!vaultUnlocked ? (
          <div className="vault-unlock-screen">
            <div className="vault-lock-icon">{Icons.vault}</div>
            <p className="vault-unlock-text">
              {vaultSetup
                ? t("vault.enterPassword")
                : t("vault.createPassword")}
            </p>
            <input
              type="password"
              className="glass-input vault-password-input"
              value={vaultPassword}
              onChange={(e) => onPasswordChange(e.target.value)}
              placeholder={t("vault.passwordPlaceholder")}
              onKeyDown={(e) => e.key === "Enter" && onUnlock()}
              autoFocus
            />
            <button className="btn btn-accent" onClick={onUnlock}>
              {t("vault.unlock")}
            </button>
          </div>
        ) : filteredDocs.length === 0 ? (
          <div className="vault-empty">
            {vaultFilter ? t("vault.noResults") : t("vault.empty")}
          </div>
        ) : vaultViewMode === "grid" ? (
          <div className="vault-grid">
            {filteredDocs.map((doc) => (
              <div
                key={doc.id}
                className="vault-grid-item"
                onClick={() => onOpenDocument(doc.id)}
              >
                <div className="vault-grid-icon">{Icons.pdf}</div>
                <div className="vault-grid-name">{doc.name}</div>
                <div className="vault-grid-meta">{doc.date}</div>
                <div className="vault-grid-size">
                  {(doc.size_bytes / 1024).toFixed(0)} KB
                </div>
                <button
                  className="vault-grid-remove"
                  onClick={(e) => {
                    e.stopPropagation();
                    onRemoveDocument(doc.id);
                  }}
                  aria-label="Remove"
                >
                  {Icons.close}
                </button>
              </div>
            ))}
          </div>
        ) : (
          <div className="vault-list">
            {filteredDocs.map((doc) => (
              <div
                key={doc.id}
                className="vault-list-item"
                onClick={() => onOpenDocument(doc.id)}
              >
                <div className="vault-list-icon">{Icons.pdf}</div>
                <div className="vault-list-info">
                  <div className="vault-list-name">{doc.name}</div>
                  <div className="vault-list-meta">
                    {doc.date} &middot; {doc.format} &middot;{" "}
                    {(doc.size_bytes / 1024).toFixed(0)} KB
                  </div>
                </div>
                <button
                  className="vault-list-remove"
                  onClick={(e) => {
                    e.stopPropagation();
                    onRemoveDocument(doc.id);
                  }}
                  aria-label="Remove"
                >
                  {Icons.close}
                </button>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
