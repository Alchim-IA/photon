import React from "react";
import { useTranslation } from "../contexts/LanguageContext";
import Icons from "./Icons";
import type { ScannedDocument, AnnotationData, AnnotationTypeName } from "../types";

interface PreviewProps {
  selectedDocument: ScannedDocument | null;
  zoomLevel: number;
  onZoomChange: (zoom: number) => void;
  annotations: Record<string, AnnotationData[]>;
  annotationTool: AnnotationTypeName | null;
  onAnnotationToolChange: (tool: AnnotationTypeName | null) => void;
  isDrawing: boolean;
  drawStart: { x: number; y: number } | null;
  currentDrawPos: { x: number; y: number } | null;
  onMouseDown: (e: React.MouseEvent<SVGSVGElement>) => void;
  onMouseMove: (e: React.MouseEvent<SVGSVGElement>) => void;
  onMouseUp: (e: React.MouseEvent<SVGSVGElement>) => void;
  signatureImage: string | null;
  signaturePlacement: { x: number; y: number; w: number; h: number } | null;
  isPlacingSignature: boolean;
  onSignatureMouseDown: (e: React.MouseEvent<SVGSVGElement>) => void;
  onSignatureMouseMove: (e: React.MouseEvent<SVGSVGElement>) => void;
  onSignatureMouseUp: (e: React.MouseEvent<SVGSVGElement>) => void;
  sigDragStart: { x: number; y: number } | null;
  sigDragCurrent: { x: number; y: number } | null;
  onContextMenu: (e: React.MouseEvent) => void;
  onClearAnnotations: () => void;
  isScanning: boolean;
  scanProgress: number;
  hasMultipage: boolean;
  hasScanners: boolean;
  onScan: () => void;
}

export function Preview({
  selectedDocument,
  zoomLevel,
  annotations,
  annotationTool,
  onAnnotationToolChange,
  isDrawing,
  drawStart,
  currentDrawPos,
  onMouseDown,
  onMouseMove,
  onMouseUp,
  signatureImage,
  signaturePlacement,
  isPlacingSignature,
  onSignatureMouseDown,
  onSignatureMouseMove,
  onSignatureMouseUp,
  sigDragStart,
  sigDragCurrent,
  onContextMenu,
  onClearAnnotations,
  isScanning,
  scanProgress,
  hasMultipage,
  hasScanners,
  onScan,
}: PreviewProps) {
  const { t } = useTranslation();
  const currentAnnotations = selectedDocument
    ? annotations[selectedDocument.id] ?? []
    : [];

  if (isScanning) {
    const clampedProgress = Math.round(Math.min(scanProgress, 100));
    return (
      <div className="scan-in-progress">
        <div className="scan-animation">
          <div className="scan-page">
            <div className="scan-line-sweep" />
          </div>
        </div>
        <div className="scan-status-text">{t("preview.scanningStatus")}</div>
        <div className="scan-progress-inline">
          <div
            className="scan-progress-bar"
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={clampedProgress}
            aria-label={t("status.scanning")}
          >
            <div
              className="scan-progress-fill"
              style={{ width: `${clampedProgress}%` }}
            />
          </div>
          <span className="scan-progress-pct">{clampedProgress}%</span>
        </div>
      </div>
    );
  }

  if (hasMultipage) {
    // Multipage rendering is handled by MultipagePanel; return null here
    return null;
  }

  if (selectedDocument) {
    return (
      <>
        {/* Annotation toolbar */}
        <div className="annotation-toolbar">
          {(["Highlight", "Ellipse", "TextNote"] as AnnotationTypeName[]).map(
            (tool) => (
              <button
                key={tool}
                className={`btn btn-sm${annotationTool === tool ? " btn-accent" : ""}`}
                onClick={() =>
                  onAnnotationToolChange(annotationTool === tool ? null : tool)
                }
              >
                {t(`export.annotation${tool}`)}
              </button>
            )
          )}
          {currentAnnotations.length > 0 && (
            <button className="btn btn-sm" onClick={onClearAnnotations}>
              {t("export.annotationClear")}
            </button>
          )}
        </div>
        <div
          className="preview-image-container"
          style={{ width: `${zoomLevel * 6}px` }}
          onContextMenu={onContextMenu}
        >
          <img
            src={selectedDocument.dataUrl}
            alt={selectedDocument.name}
            className="preview-image"
          />
          {/* Annotation & Signature SVG overlay */}
          <svg
            className="annotation-overlay"
            viewBox="0 0 1 1"
            preserveAspectRatio="none"
            style={{
              pointerEvents:
                annotationTool || isPlacingSignature ? "all" : "none",
              cursor: isPlacingSignature ? "crosshair" : undefined,
            }}
            onMouseDown={
              isPlacingSignature ? onSignatureMouseDown : onMouseDown
            }
            onMouseMove={
              isPlacingSignature ? onSignatureMouseMove : onMouseMove
            }
            onMouseUp={isPlacingSignature ? onSignatureMouseUp : onMouseUp}
          >
            {currentAnnotations.map((ann, i) => {
              if (ann.annotation_type === "Highlight") {
                return (
                  <rect
                    key={i}
                    x={ann.x}
                    y={ann.y}
                    width={ann.width}
                    height={ann.height}
                    fill={ann.color}
                    opacity={0.3}
                  />
                );
              }
              if (ann.annotation_type === "Ellipse") {
                return (
                  <ellipse
                    key={i}
                    cx={ann.x + ann.width / 2}
                    cy={ann.y + ann.height / 2}
                    rx={ann.width / 2}
                    ry={ann.height / 2}
                    fill="none"
                    stroke={ann.color}
                    strokeWidth={0.003}
                  />
                );
              }
              if (ann.annotation_type === "TextNote") {
                return (
                  <g key={i}>
                    <rect
                      x={ann.x}
                      y={ann.y}
                      width={0.015}
                      height={0.015}
                      fill={ann.color}
                    />
                    <text
                      x={ann.x + 0.02}
                      y={ann.y + 0.012}
                      fontSize={0.012}
                      fill="#000"
                    >
                      {ann.text}
                    </text>
                  </g>
                );
              }
              return null;
            })}
            {/* Signature placed preview */}
            {signaturePlacement && signatureImage && (
              <rect
                x={signaturePlacement.x}
                y={signaturePlacement.y}
                width={signaturePlacement.w}
                height={signaturePlacement.h}
                fill="rgba(37,99,235,0.08)"
                stroke="#2563eb"
                strokeWidth={0.003}
                strokeDasharray="0.008 0.004"
              />
            )}
            {/* Signature drag-in-progress preview */}
            {isPlacingSignature &&
              sigDragStart &&
              sigDragCurrent &&
              (() => {
                const x = Math.min(sigDragStart.x, sigDragCurrent.x);
                const y = Math.min(sigDragStart.y, sigDragCurrent.y);
                const w = Math.abs(sigDragCurrent.x - sigDragStart.x);
                const h = Math.abs(sigDragCurrent.y - sigDragStart.y);
                return (
                  <rect
                    x={x}
                    y={y}
                    width={w}
                    height={h}
                    fill="rgba(37,99,235,0.15)"
                    stroke="#2563eb"
                    strokeWidth={0.003}
                    strokeDasharray="0.006 0.003"
                  />
                );
              })()}
            {/* Drawing preview */}
            {isDrawing &&
              drawStart &&
              currentDrawPos &&
              annotationTool &&
              (() => {
                const x = Math.min(drawStart.x, currentDrawPos.x);
                const y = Math.min(drawStart.y, currentDrawPos.y);
                const w = Math.abs(currentDrawPos.x - drawStart.x);
                const h = Math.abs(currentDrawPos.y - drawStart.y);
                if (annotationTool === "Highlight") {
                  return (
                    <rect
                      x={x}
                      y={y}
                      width={w}
                      height={h}
                      fill="#FFFF00"
                      opacity={0.3}
                    />
                  );
                }
                if (annotationTool === "Ellipse") {
                  return (
                    <ellipse
                      cx={x + w / 2}
                      cy={y + h / 2}
                      rx={w / 2}
                      ry={h / 2}
                      fill="none"
                      stroke="#FF0000"
                      strokeWidth={0.003}
                    />
                  );
                }
                return (
                  <rect
                    x={x}
                    y={y}
                    width={w}
                    height={h}
                    fill="#3B82F6"
                    opacity={0.2}
                  />
                );
              })()}
          </svg>
        </div>
      </>
    );
  }

  // Empty state
  return (
    <div className="preview-empty">
      <div className="preview-empty-icon">{Icons.empty}</div>
      <div className="preview-empty-title">{t("preview.emptyTitle")}</div>
      <div className="preview-empty-desc">{t("preview.emptyDesc")}</div>
      {hasScanners && (
        <button className="btn btn-accent" onClick={onScan}>
          {Icons.scan} {t("actions.scan")}
        </button>
      )}
    </div>
  );
}
