import { useTranslation } from "../contexts/LanguageContext";
import Icons from "./Icons";

interface ContextMenuState {
  x: number;
  y: number;
  docId: string;
  pageIndex?: number;
  isPreview?: boolean;
}

interface ContextMenuProps {
  contextMenu: ContextMenuState | null;
  onClose: () => void;
  onRename: (docId: string) => void;
  onDuplicate: (docId: string) => void;
  onDelete: (docId: string) => void;
  onEmail: (docId: string) => void;
  onVaultAdd: (docId: string) => void;
  onRotate: (direction: string, docId: string) => void;
  onFlip: (axis: string, docId: string) => void;
  onWatermark: () => void;
  onSignature: () => void;
  onRemovePage: (pageIndex: number) => void;
  onAddToPages: (docId: string) => void;
  exportWatermarkEnabled: boolean;
  onRemoveWatermark: () => void;
  signaturePlacement: { x: number; y: number; w: number; h: number } | null;
  signatureImage: string | null;
  onRemoveSignature: () => void;
  hasMultipage: boolean;
}

export function ContextMenu({
  contextMenu,
  onClose,
  onRename,
  onDuplicate,
  onDelete,
  onEmail,
  onVaultAdd,
  onRotate,
  onFlip,
  onWatermark,
  onSignature,
  onRemovePage,
  onAddToPages,
  exportWatermarkEnabled,
  onRemoveWatermark,
  signaturePlacement,
  signatureImage,
  onRemoveSignature,
  hasMultipage,
}: ContextMenuProps) {
  const { t } = useTranslation();

  if (!contextMenu) return null;

  return (
    <div
      className="context-menu-overlay"
      role="presentation"
      onClick={onClose}
      onContextMenu={(e) => {
        e.preventDefault();
        onClose();
      }}
    >
      <div
        className="context-menu"
        role="menu"
        style={{ top: contextMenu.y, left: contextMenu.x }}
      >
        {contextMenu.isPreview ? (
          <>
            <button
              role="menuitem"
              className="context-menu-item"
              onClick={() => {
                onClose();
                onWatermark();
              }}
            >
              {exportWatermarkEnabled
                ? t("contextMenu.editWatermark")
                : t("contextMenu.addWatermark")}
            </button>
            {exportWatermarkEnabled && (
              <button
                role="menuitem"
                className="context-menu-item"
                onClick={() => {
                  onRemoveWatermark();
                  onClose();
                }}
              >
                {t("contextMenu.removeWatermark")}
              </button>
            )}
            <div className="context-menu-divider" />
            <button
              role="menuitem"
              className="context-menu-item"
              onClick={() => {
                onClose();
                onSignature();
              }}
            >
              {signaturePlacement
                ? t("contextMenu.editSignature")
                : t("contextMenu.addSignature")}
            </button>
            {signaturePlacement && signatureImage && (
              <button
                role="menuitem"
                className="context-menu-item"
                onClick={() => {
                  onRemoveSignature();
                  onClose();
                }}
              >
                {t("contextMenu.removeSignature")}
              </button>
            )}
          </>
        ) : contextMenu.pageIndex !== undefined ? (
          <>
            <button
              role="menuitem"
              className="context-menu-item"
              onClick={() => {
                onRotate("270", contextMenu.docId);
                onClose();
              }}
            >
              {Icons.rotateLeft} {t("contextMenu.rotateLeft")}
            </button>
            <button
              role="menuitem"
              className="context-menu-item"
              onClick={() => {
                onRotate("90", contextMenu.docId);
                onClose();
              }}
            >
              {Icons.rotateRight} {t("contextMenu.rotateRight")}
            </button>
            <button
              role="menuitem"
              className="context-menu-item"
              onClick={() => {
                onRotate("180", contextMenu.docId);
                onClose();
              }}
            >
              ↻ {t("contextMenu.rotate180")}
            </button>
            <div className="context-menu-divider" />
            <button
              role="menuitem"
              className="context-menu-item"
              onClick={() => {
                onFlip("horizontal", contextMenu.docId);
                onClose();
              }}
            >
              {Icons.flipH} {t("contextMenu.flipH")}
            </button>
            <button
              role="menuitem"
              className="context-menu-item"
              onClick={() => {
                onFlip("vertical", contextMenu.docId);
                onClose();
              }}
            >
              {Icons.flipV} {t("contextMenu.flipV")}
            </button>
            <div className="context-menu-divider" />
            <button
              role="menuitem"
              className="context-menu-item"
              onClick={() => {
                onRemovePage(contextMenu.pageIndex!);
                onClose();
              }}
            >
              {Icons.close} {t("contextMenu.removeFromPages")}
            </button>
            <button
              role="menuitem"
              className="context-menu-item context-menu-danger"
              onClick={() => {
                onDelete(contextMenu.docId);
                onClose();
              }}
            >
              {Icons.delete} {t("contextMenu.delete")}
            </button>
          </>
        ) : (
          <>
            <button
              role="menuitem"
              className="context-menu-item"
              onClick={() => {
                onRename(contextMenu.docId);
                onClose();
              }}
            >
              {Icons.rename} {t("contextMenu.rename")}
            </button>
            <button
              role="menuitem"
              className="context-menu-item"
              onClick={() => {
                onDuplicate(contextMenu.docId);
                onClose();
              }}
            >
              {Icons.duplicate} {t("contextMenu.duplicate")}
            </button>
            <button
              role="menuitem"
              className="context-menu-item"
              onClick={() => {
                onAddToPages(contextMenu.docId);
                onClose();
              }}
              disabled={!hasMultipage}
            >
              {Icons.pages} {t("contextMenu.addToPages")}
            </button>
            <button
              role="menuitem"
              className="context-menu-item"
              onClick={() => {
                onEmail(contextMenu.docId);
                onClose();
              }}
            >
              {Icons.email} {t("contextMenu.email")}
            </button>
            <button
              role="menuitem"
              className="context-menu-item"
              onClick={() => {
                onVaultAdd(contextMenu.docId);
                onClose();
              }}
            >
              {Icons.vault} {t("contextMenu.addToVault")}
            </button>
            <div className="context-menu-divider" />
            <button
              role="menuitem"
              className="context-menu-item context-menu-danger"
              onClick={() => {
                onDelete(contextMenu.docId);
                onClose();
              }}
            >
              {Icons.delete} {t("contextMenu.delete")}
            </button>
          </>
        )}
      </div>
    </div>
  );
}
