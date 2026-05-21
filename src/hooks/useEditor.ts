import { useState, useCallback, useRef, useEffect } from "react";
import * as api from "../services/tauriApi";
import type { ViewportData, TabInfo, CursorPosition } from "../types/editor";

/** Number of visible lines in the viewport */
const VIEWPORT_LINES = 80;
/** Extra lines to preload above/below */
const PRELOAD_LINES = 40;

export function useEditor() {
  const [activeTabId, setActiveTabId] = useState<string | null>(null);
  const [tabs, setTabs] = useState<TabInfo[]>([]);
  const [viewport, setViewport] = useState<ViewportData | null>(null);
  const [cursor, setCursor] = useState<CursorPosition>({ line: 0, column: 0 });
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Throttle viewport requests
  const pendingRequest = useRef<number | null>(null);
  const lastStartLine = useRef<number>(0);

  const refreshTabs = useCallback(async () => {
    try {
      const t = await api.getTabs();
      setTabs(t);
    } catch {
      // ignore
    }
  }, []);

  const loadViewport = useCallback(async (tabId: string, startLine: number) => {
    // Clamp start line
    const clampedStart = Math.max(0, startLine);
    const effectiveStart = Math.max(0, clampedStart - PRELOAD_LINES);
    const lineCount = VIEWPORT_LINES + PRELOAD_LINES * 2;

    try {
      const vp = await api.getViewport(tabId, effectiveStart, lineCount);
      setViewport(vp);
      lastStartLine.current = clampedStart;
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const requestViewport = useCallback((tabId: string, startLine: number) => {
    // Throttle: cancel pending request
    if (pendingRequest.current !== null) {
      cancelAnimationFrame(pendingRequest.current);
    }
    pendingRequest.current = requestAnimationFrame(() => {
      loadViewport(tabId, startLine);
      pendingRequest.current = null;
    });
  }, [loadViewport]);

  const openFile = useCallback(async (path?: string) => {
    setLoading(true);
    setError(null);
    try {
      const filePath = path ?? (await api.showOpenFileDialog());
      if (!filePath) {
        setLoading(false);
        return;
      }

      const result = await api.openFile(filePath);
      setActiveTabId(result.tab_id);
      await refreshTabs();
      await loadViewport(result.tab_id, 0);
      setCursor({ line: 0, column: 0 });
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [loadViewport, refreshTabs]);

  const closeTab = useCallback(async (tabId: string) => {
    try {
      await api.closeFile(tabId);
      await refreshTabs();

      if (activeTabId === tabId) {
        const remaining = tabs.filter((t) => t.id !== tabId);
        if (remaining.length > 0) {
          const nextTab = remaining[remaining.length - 1];
          setActiveTabId(nextTab.id);
          await loadViewport(nextTab.id, 0);
        } else {
          setActiveTabId(null);
          setViewport(null);
        }
      }
    } catch (e) {
      setError(String(e));
    }
  }, [activeTabId, tabs, loadViewport, refreshTabs]);

  const switchTab = useCallback(async (tabId: string) => {
    setActiveTabId(tabId);
    await loadViewport(tabId, 0);
  }, [loadViewport]);

  const handleGotoLine = useCallback(async (line: number) => {
    if (!activeTabId) return;
    const clamped = Math.max(0, line);
    await loadViewport(activeTabId, clamped);
    setCursor({ line: clamped, column: 0 });
  }, [activeTabId, loadViewport]);

  // Keyboard shortcuts
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // Cmd/Ctrl + O: Open file
      if ((e.metaKey || e.ctrlKey) && e.key === "o") {
        e.preventDefault();
        openFile();
      }
      // Cmd/Ctrl + G: Go to line
      if ((e.metaKey || e.ctrlKey) && e.key === "g") {
        e.preventDefault();
        const lineStr = prompt("Go to line:");
        if (lineStr) {
          const line = parseInt(lineStr, 10);
          if (!isNaN(line)) {
            handleGotoLine(line - 1); // Convert 1-based to 0-based
          }
        }
      }
      // Cmd/Ctrl + W: Close tab
      if ((e.metaKey || e.ctrlKey) && e.key === "w") {
        e.preventDefault();
        if (activeTabId) {
          closeTab(activeTabId);
        }
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [openFile, handleGotoLine, closeTab, activeTabId]);

  return {
    activeTabId,
    tabs,
    viewport,
    cursor,
    loading,
    error,
    openFile,
    closeTab,
    switchTab,
    requestViewport,
    handleGotoLine,
    setCursor,
  };
}
