// ─── Debug Logger ─────────────────────────────────────────────────
// Centralized logging system for Photon.
// Logs are stored in-memory and can be displayed in the DebugLogPanel.

export type LogLevel = "debug" | "info" | "warn" | "error";

export interface LogEntry {
  id: number;
  timestamp: Date;
  level: LogLevel;
  source: string;
  message: string;
  details?: string;
}

type LogListener = (entry: LogEntry) => void;

let nextId = 1;
const MAX_ENTRIES = 500;
const entries: LogEntry[] = [];
const listeners: Set<LogListener> = new Set();

function addEntry(level: LogLevel, source: string, message: string, details?: string) {
  const entry: LogEntry = {
    id: nextId++,
    timestamp: new Date(),
    level,
    source,
    message,
    details,
  };
  entries.push(entry);
  if (entries.length > MAX_ENTRIES) {
    entries.splice(0, entries.length - MAX_ENTRIES);
  }
  for (const listener of listeners) {
    listener(entry);
  }
}

export const logger = {
  debug(source: string, message: string, details?: string) {
    addEntry("debug", source, message, details);
  },
  info(source: string, message: string, details?: string) {
    addEntry("info", source, message, details);
  },
  warn(source: string, message: string, details?: string) {
    addEntry("warn", source, message, details);
    console.warn(`[${source}] ${message}`, details ?? "");
  },
  error(source: string, message: string, details?: string) {
    addEntry("error", source, message, details);
    console.error(`[${source}] ${message}`, details ?? "");
  },
  getEntries(): readonly LogEntry[] {
    return entries;
  },
  clear() {
    entries.length = 0;
  },
  subscribe(listener: LogListener): () => void {
    listeners.add(listener);
    return () => listeners.delete(listener);
  },
};
