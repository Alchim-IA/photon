import React from "react";
import { useTranslation } from "../contexts/LanguageContext";
import Icons from "./Icons";
import type { ScannedDocument, HistoryEntryDto } from "../types";

interface HistoryGridProps {
  documents: ScannedDocument[];
  selectedDocument: ScannedDocument | null;
  onSelectDocument: (doc: ScannedDocument) => void;
  searchQuery: string;
  onSearchChange: (query: string) => void;
  onSearch: (query: string) => void;
  searchResults: HistoryEntryDto[] | null;
  renamingDocId: string | null;
  renameValue: string;
  onStartRename: (docId: string, currentName: string) => void;
  onRenameChange: (value: string) => void;
  onRenameSubmit: (docId: string, newName: string) => void;
  onRenameCancel: () => void;
  onContextMenu: (e: React.MouseEvent, docId: string) => void;
  onDeleteDocument: (docId: string) => void;
  zoomLevel: number;
  documentTags: Record<string, string[]>;
}

export function HistoryGrid({
  documents,
  selectedDocument,
  onSelectDocument,
  searchQuery,
  onSearch,
  searchResults,
  renamingDocId,
  renameValue,
  onRenameChange,
  onRenameSubmit,
  onRenameCancel,
  onContextMenu,
  onDeleteDocument,
  zoomLevel,
}: HistoryGridProps) {
  const { t } = useTranslation();

  const displayDocs = searchQuery.trim()
    ? documents.filter(
        (doc) => searchResults?.some((r) => r.id === doc.id) ?? false
      )
    : documents;

  return (
    <div className="history-panel" role="tabpanel">
      <div className="history-search">
        <div className="history-search-icon">{Icons.search}</div>
        <input
          type="text"
          className="glass-input history-search-input"
          placeholder={t("history.searchPlaceholder")}
          value={searchQuery}
          onChange={(e) => onSearch(e.target.value)}
        />
      </div>
      <div
        className="history-grid"
        style={{
          gridTemplateColumns: `repeat(auto-fill, minmax(${Math.round(
            (130 * zoomLevel) / 100
          )}px, 1fr))`,
        }}
      >
        {displayDocs.length === 0 ? (
          <div className="history-empty">
            <div className="history-empty-icon">{Icons.folder}</div>
            <p>
              {searchQuery.trim()
                ? t("history.noResults")
                : t("history.noDocuments")}
            </p>
            <p className="history-empty-hint">
              {searchQuery.trim()
                ? t("history.noResultsHint")
                : t("history.noDocumentsHint")}
            </p>
          </div>
        ) : (
          displayDocs.map((doc) => (
            <button
              key={doc.id}
              className={`history-item ${selectedDocument?.id === doc.id ? "active" : ""}`}
              onClick={() => onSelectDocument(doc)}
              onContextMenu={(e) => {
                e.preventDefault();
                onContextMenu(e, doc.id);
              }}
              aria-pressed={selectedDocument?.id === doc.id}
              aria-label={`${doc.name}, ${doc.date}`}
            >
              <div className="history-thumb">
                <img src={doc.dataUrl} alt={doc.name} />
                {searchResults?.find((r) => r.id === doc.id)?.has_ocr && (
                  <div className="ocr-badge" title={t("history.ocrBadge")}>
                    T
                  </div>
                )}
              </div>
              <div className="history-meta">
                {renamingDocId === doc.id ? (
                  <input
                    className="glass-input history-rename-input"
                    value={renameValue}
                    onChange={(e) => onRenameChange(e.target.value)}
                    onBlur={() => {
                      if (renameValue.trim())
                        onRenameSubmit(doc.id, renameValue.trim());
                      else onRenameCancel();
                    }}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" && renameValue.trim())
                        onRenameSubmit(doc.id, renameValue.trim());
                      if (e.key === "Escape") onRenameCancel();
                    }}
                    autoFocus
                    onClick={(e) => e.stopPropagation()}
                  />
                ) : (
                  <div className="history-name">{doc.name}</div>
                )}
                <div className="history-date">{doc.date}</div>
                <button
                  className="history-delete"
                  onClick={(e) => {
                    e.stopPropagation();
                    onDeleteDocument(doc.id);
                  }}
                  aria-label={t("history.deleteTooltip")}
                >
                  {Icons.delete}
                </button>
              </div>
            </button>
          ))
        )}
      </div>
    </div>
  );
}
