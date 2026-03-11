import { useState, useEffect, useRef, useCallback } from "react";
import { useTranslation } from "../contexts/LanguageContext";
import { logger, type LogEntry, type LogLevel } from "../utils/debugLogger";
import Icons from "./Icons";

interface DebugLogPanelProps {
  show: boolean;
  onClose: () => void;
}

const LEVEL_COLORS: Record<LogLevel, string> = {
  debug: "var(--text-muted, #888)",
  info: "var(--accent, #3b82f6)",
  warn: "#f59e0b",
  error: "#ef4444",
};

const LEVEL_LABELS: Record<LogLevel, string> = {
  debug: "DBG",
  info: "INF",
  warn: "WRN",
  error: "ERR",
};

export function DebugLogPanel({ show, onClose }: DebugLogPanelProps) {
  const { t } = useTranslation();
  const [entries, setEntries] = useState<LogEntry[]>([...logger.getEntries()]);
  const [filterLevel, setFilterLevel] = useState<LogLevel | "all">("all");
  const [filterSource, setFilterSource] = useState("");
  const [autoScroll, setAutoScroll] = useState(true);
  const [minimized, setMinimized] = useState(false);
  const listRef = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState({ x: 20, y: 60 });
  const [size, setSize] = useState({ w: 600, h: 400 });
  const dragRef = useRef<{ startX: number; startY: number; origX: number; origY: number } | null>(null);
  const resizeRef = useRef<{ startX: number; startY: number; origW: number; origH: number } | null>(null);

  useEffect(() => {
    const unsub = logger.subscribe((entry) => {
      setEntries((prev) => [...prev, entry]);
    });
    return unsub;
  }, []);

  useEffect(() => {
    if (autoScroll && listRef.current) {
      listRef.current.scrollTop = listRef.current.scrollHeight;
    }
  }, [entries, autoScroll]);

  const filtered = entries.filter((e) => {
    if (filterLevel !== "all" && e.level !== filterLevel) return false;
    if (filterSource && !e.source.toLowerCase().includes(filterSource.toLowerCase())) return false;
    return true;
  });

  const errorCount = entries.filter((e) => e.level === "error").length;
  const warnCount = entries.filter((e) => e.level === "warn").length;

  const handleClear = useCallback(() => {
    logger.clear();
    setEntries([]);
  }, []);

  const handleExport = useCallback(() => {
    const text = entries
      .map((e) => `[${e.timestamp.toISOString()}] [${e.level.toUpperCase()}] [${e.source}] ${e.message}${e.details ? "\n  " + e.details : ""}`)
      .join("\n");
    const blob = new Blob([text], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `photon-debug-${new Date().toISOString().slice(0, 19).replace(/:/g, "-")}.log`;
    a.click();
    URL.revokeObjectURL(url);
  }, [entries]);

  // Drag to move
  const handleDragStart = (e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest(".debug-panel-controls")) return;
    e.preventDefault();
    dragRef.current = { startX: e.clientX, startY: e.clientY, origX: position.x, origY: position.y };
    const handleMove = (ev: MouseEvent) => {
      if (!dragRef.current) return;
      setPosition({
        x: dragRef.current.origX + (ev.clientX - dragRef.current.startX),
        y: dragRef.current.origY + (ev.clientY - dragRef.current.startY),
      });
    };
    const handleUp = () => {
      dragRef.current = null;
      window.removeEventListener("mousemove", handleMove);
      window.removeEventListener("mouseup", handleUp);
    };
    window.addEventListener("mousemove", handleMove);
    window.addEventListener("mouseup", handleUp);
  };

  // Resize
  const handleResizeStart = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    resizeRef.current = { startX: e.clientX, startY: e.clientY, origW: size.w, origH: size.h };
    const handleMove = (ev: MouseEvent) => {
      if (!resizeRef.current) return;
      setSize({
        w: Math.max(350, resizeRef.current.origW + (ev.clientX - resizeRef.current.startX)),
        h: Math.max(200, resizeRef.current.origH + (ev.clientY - resizeRef.current.startY)),
      });
    };
    const handleUp = () => {
      resizeRef.current = null;
      window.removeEventListener("mousemove", handleMove);
      window.removeEventListener("mouseup", handleUp);
    };
    window.addEventListener("mousemove", handleMove);
    window.addEventListener("mouseup", handleUp);
  };

  if (!show) return null;

  const formatTime = (d: Date) =>
    `${d.getHours().toString().padStart(2, "0")}:${d.getMinutes().toString().padStart(2, "0")}:${d.getSeconds().toString().padStart(2, "0")}.${d.getMilliseconds().toString().padStart(3, "0")}`;

  return (
    <div
      className="debug-log-panel"
      style={{
        left: position.x,
        top: position.y,
        width: minimized ? 280 : size.w,
        height: minimized ? "auto" : size.h,
      }}
    >
      {/* Header - draggable */}
      <div className="debug-panel-header" onMouseDown={handleDragStart}>
        <div className="debug-panel-title">
          {Icons.terminal} {t("debug.title")}
          {errorCount > 0 && <span className="debug-badge debug-badge-error">{errorCount}</span>}
          {warnCount > 0 && <span className="debug-badge debug-badge-warn">{warnCount}</span>}
        </div>
        <div className="debug-panel-controls">
          <button className="btn btn-icon btn-xs btn-ghost" onClick={() => setMinimized(!minimized)} title={minimized ? t("debug.expand") : t("debug.minimize")}>
            {minimized ? "▢" : "—"}
          </button>
          <button className="btn btn-icon btn-xs btn-ghost" onClick={onClose} title={t("debug.close")}>
            ✕
          </button>
        </div>
      </div>

      {!minimized && (
        <>
          {/* Toolbar */}
          <div className="debug-panel-toolbar">
            <select
              className="debug-filter-select"
              value={filterLevel}
              onChange={(e) => setFilterLevel(e.target.value as LogLevel | "all")}
            >
              <option value="all">{t("debug.allLevels")}</option>
              <option value="error">Error</option>
              <option value="warn">Warning</option>
              <option value="info">Info</option>
              <option value="debug">Debug</option>
            </select>
            <input
              className="debug-filter-input"
              type="text"
              placeholder={t("debug.filterSource")}
              value={filterSource}
              onChange={(e) => setFilterSource(e.target.value)}
            />
            <div className="debug-toolbar-spacer" />
            <label className="debug-autoscroll">
              <input type="checkbox" checked={autoScroll} onChange={(e) => setAutoScroll(e.target.checked)} />
              {t("debug.autoScroll")}
            </label>
            <button className="btn btn-xs btn-ghost" onClick={handleExport} title={t("debug.export")}>
              {Icons.save}
            </button>
            <button className="btn btn-xs btn-ghost" onClick={handleClear} title={t("debug.clear")}>
              {Icons.trash}
            </button>
          </div>

          {/* Log entries */}
          <div className="debug-panel-logs" ref={listRef}>
            {filtered.length === 0 ? (
              <div className="debug-empty">{t("debug.noLogs")}</div>
            ) : (
              filtered.map((entry) => (
                <div key={entry.id} className={`debug-log-entry debug-log-${entry.level}`}>
                  <span className="debug-log-time">{formatTime(entry.timestamp)}</span>
                  <span className="debug-log-level" style={{ color: LEVEL_COLORS[entry.level] }}>
                    {LEVEL_LABELS[entry.level]}
                  </span>
                  <span className="debug-log-source">[{entry.source}]</span>
                  <span className="debug-log-message">{entry.message}</span>
                  {entry.details && <div className="debug-log-details">{entry.details}</div>}
                </div>
              ))
            )}
          </div>

          {/* Status bar */}
          <div className="debug-panel-status">
            <span>{t("debug.entries", { count: filtered.length, total: entries.length })}</span>
          </div>

          {/* Resize handle */}
          <div className="debug-resize-handle" onMouseDown={handleResizeStart} />
        </>
      )}
    </div>
  );
}
