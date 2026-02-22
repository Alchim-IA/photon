import React from "react";
import {
  DndContext,
  rectIntersection,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  rectSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { useTranslation } from "../contexts/LanguageContext";
import Icons from "./Icons";
import type { ScannedDocument, MultiPageDocDto } from "../types";

// ─── SortablePreviewPage sub-component ─────────────────────────
function SortablePreviewPage({
  uniqueId,
  index,
  doc,
  isSelected,
  onSelect,
  onRemove,
  onContextMenu,
}: {
  uniqueId: string;
  index: number;
  doc: ScannedDocument | undefined;
  isSelected: boolean;
  onSelect: () => void;
  onRemove: () => void;
  onContextMenu: (e: React.MouseEvent) => void;
}) {
  const { attributes, listeners, setNodeRef, transform, transition } =
    useSortable({ id: uniqueId });
  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
  };

  return (
    <div
      ref={setNodeRef}
      style={style}
      {...attributes}
      {...listeners}
      className={`multipage-preview-item${isSelected ? " selected" : ""}`}
      onClick={onSelect}
      onContextMenu={onContextMenu}
    >
      <div className="multipage-preview-item-number">{index + 1}</div>
      {doc ? (
        <img
          src={doc.dataUrl}
          alt={`Page ${index + 1}`}
          className="multipage-preview-item-thumb"
        />
      ) : (
        <div className="multipage-preview-item-placeholder">?</div>
      )}
      <button
        className="multipage-preview-item-remove"
        onClick={(e) => {
          e.stopPropagation();
          onRemove();
        }}
        aria-label="Remove page"
      >
        {Icons.close}
      </button>
    </div>
  );
}

// ─── MultipagePanel ─────────────────────────────────────────────
interface MultipagePanelProps {
  multipageDoc: MultiPageDocDto | null;
  documents: ScannedDocument[];
  selectedDocument: ScannedDocument | null;
  onSelectDocument: (doc: ScannedDocument) => void;
  onAddPage: (docId: string) => void;
  onRemovePage: (pageIndex: number) => void;
  onReorderPages: (event: DragEndEvent) => void;
  onSaveMultipagePdf: () => void;
  onClose: () => void;
  onContextMenu: (e: React.MouseEvent, docId: string, pageIndex: number) => void;
  zoomLevel: number;
}

export function MultipagePanel({
  multipageDoc,
  documents,
  selectedDocument,
  onSelectDocument,
  onRemovePage,
  onReorderPages,
  onSaveMultipagePdf,
  onClose,
  onContextMenu,
  zoomLevel,
}: MultipagePanelProps) {
  const { t } = useTranslation();
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 8 } }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    })
  );

  if (!multipageDoc || multipageDoc.page_count === 0) {
    return null;
  }

  return (
    <div className="multipage-preview">
      <div className="multipage-preview-header">
        <span className="multipage-preview-title">{multipageDoc.name}</span>
        <span className="multipage-preview-count">
          {t("multipage.pageCount", { count: multipageDoc.page_count })}
        </span>
        <div className="multipage-preview-actions">
          <button
            className="btn btn-sm btn-accent"
            onClick={onSaveMultipagePdf}
            disabled={multipageDoc.page_count === 0}
          >
            {Icons.pdf} {t("multipage.savePdf")}
          </button>
          <button className="btn btn-sm" onClick={onClose}>
            {Icons.close} {t("multipage.close")}
          </button>
        </div>
      </div>
      <DndContext
        sensors={sensors}
        collisionDetection={rectIntersection}
        onDragEnd={onReorderPages}
      >
        <SortableContext
          items={multipageDoc.page_ids.map((_, i) => `page-${i}`)}
          strategy={rectSortingStrategy}
        >
          <div
            className="multipage-preview-grid"
            style={
              { "--page-scale": zoomLevel / 100 } as React.CSSProperties
            }
          >
            {multipageDoc.page_ids.map((pid, i) => {
              const pageDoc = documents.find((d) => d.id === pid);
              return (
                <SortablePreviewPage
                  key={`page-${i}`}
                  uniqueId={`page-${i}`}
                  index={i}
                  doc={pageDoc}
                  isSelected={selectedDocument?.id === pid}
                  onSelect={() => {
                    if (pageDoc) onSelectDocument(pageDoc);
                  }}
                  onRemove={() => onRemovePage(i)}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    if (pageDoc) onContextMenu(e, pid, i);
                  }}
                />
              );
            })}
          </div>
        </SortableContext>
      </DndContext>
    </div>
  );
}
