import { useState, useCallback, useRef, useEffect } from "react";
import * as api from "../services/tauriApi";
import type { ViewportData, TabInfo, CursorPosition, EditResult, SearchMatch, SearchOptions } from "../types/editor";

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
  const [canUndo, setCanUndo] = useState(false);
  const [canRedo, setCanRedo] = useState(false);
  const [showGotoDialog, setShowGotoDialog] = useState(false);
  const [showSearchBar, setShowSearchBar] = useState(false);
  const [darkMode, setDarkMode] = useState(() => {
    // Check system preference or saved setting
    return window.matchMedia("(prefers-color-scheme: dark)").matches;
  });
  const [searchResults, setSearchResults] = useState<SearchMatch[]>([]);
  const [currentMatchIndex, setCurrentMatchIndex] = useState(0);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchOptions, setSearchOptions] = useState<SearchOptions>({ case_sensitive: false, whole_word: false, regex: false });

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
      setCanUndo(false);
      setCanRedo(false);
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

  // ─── Edit operations ──────────────────────────────────────

  const applyEditResult = useCallback((result: EditResult) => {
    setViewport(result.viewport);
    setCursor({ line: result.cursor_line, column: result.cursor_col });
    setCanUndo(result.can_undo);
    setCanRedo(result.can_redo);
    // Update tab dirty state
    setTabs((prev) =>
      prev.map((t) =>
        t.id === activeTabId ? { ...t, dirty: result.dirty } : t
      )
    );
  }, [activeTabId]);

  const handleInsertText = useCallback(async (line: number, col: number, text: string) => {
    if (!activeTabId) return;
    try {
      const result = await api.insertText(activeTabId, line, col, text);
      applyEditResult(result);
    } catch (e) {
      setError(String(e));
    }
  }, [activeTabId, applyEditResult]);

  const handleDeleteRange = useCallback(async (startLine: number, startCol: number, endLine: number, endCol: number) => {
    if (!activeTabId) return;
    try {
      const result = await api.deleteRange(activeTabId, startLine, startCol, endLine, endCol);
      applyEditResult(result);
    } catch (e) {
      setError(String(e));
    }
  }, [activeTabId, applyEditResult]);

  const handleReplaceRange = useCallback(async (startLine: number, startCol: number, endLine: number, endCol: number, text: string) => {
    if (!activeTabId) return;
    try {
      const result = await api.replaceRange(activeTabId, startLine, startCol, endLine, endCol, text);
      applyEditResult(result);
    } catch (e) {
      setError(String(e));
    }
  }, [activeTabId, applyEditResult]);

  const handleUndo = useCallback(async () => {
    if (!activeTabId) return;
    try {
      const result = await api.undoEdit(activeTabId, cursor.line);
      applyEditResult(result);
    } catch (e) {
      setError(String(e));
    }
  }, [activeTabId, cursor.line, applyEditResult]);

  const handleRedo = useCallback(async () => {
    if (!activeTabId) return;
    try {
      const result = await api.redoEdit(activeTabId, cursor.line);
      applyEditResult(result);
    } catch (e) {
      setError(String(e));
    }
  }, [activeTabId, cursor.line, applyEditResult]);

  const handleSave = useCallback(async () => {
    if (!activeTabId) return;
    try {
      await api.saveFile(activeTabId);
      setTabs((prev) =>
        prev.map((t) =>
          t.id === activeTabId ? { ...t, dirty: false } : t
        )
      );
    } catch (e) {
      setError(String(e));
    }
  }, [activeTabId]);

  const handleSaveAs = useCallback(async () => {
    if (!activeTabId) return;
    try {
      const path = await api.showSaveFileDialog();
      if (!path) return;
      await api.saveFileAs(activeTabId, path);
      setTabs((prev) =>
        prev.map((t) =>
          t.id === activeTabId ? { ...t, dirty: false } : t
        )
      );
    } catch (e) {
      setError(String(e));
    }
  }, [activeTabId]);

  // ─── Theme toggle ──────────────────────────────────────────

  const toggleDarkMode = useCallback(() => {
    setDarkMode((prev) => {
      const next = !prev;
      if (next) {
        document.documentElement.classList.add("dark");
      } else {
        document.documentElement.classList.remove("dark");
      }
      return next;
    });
  }, []);

  // Apply initial theme
  useEffect(() => {
    if (darkMode) {
      document.documentElement.classList.add("dark");
    }
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // ─── Search operations ─────────────────────────────────────

  const handleSearch = useCallback(async (query: string, options: SearchOptions) => {
    if (!activeTabId || !query) {
      setSearchResults([]);
      setCurrentMatchIndex(0);
      return;
    }
    try {
      setSearchQuery(query);
      setSearchOptions(options);
      const results = await api.searchQuery(activeTabId, query, options);
      setSearchResults(results);
      setCurrentMatchIndex(results.length > 0 ? 0 : -1);
      // Navigate to first match
      if (results.length > 0) {
        const m = results[0];
        await loadViewport(activeTabId, m.line);
        setCursor({ line: m.line, column: m.col });
      }
    } catch (e) {
      setError(String(e));
    }
  }, [activeTabId, loadViewport, setCursor]);

  const handleSearchNext = useCallback(async () => {
    if (!activeTabId || searchResults.length === 0) return;
    const nextIdx = (currentMatchIndex + 1) % searchResults.length;
    setCurrentMatchIndex(nextIdx);
    const m = searchResults[nextIdx];
    await loadViewport(activeTabId, m.line);
    setCursor({ line: m.line, column: m.col });
  }, [activeTabId, searchResults, currentMatchIndex, loadViewport, setCursor]);

  const handleSearchPrev = useCallback(async () => {
    if (!activeTabId || searchResults.length === 0) return;
    const prevIdx = (currentMatchIndex - 1 + searchResults.length) % searchResults.length;
    setCurrentMatchIndex(prevIdx);
    const m = searchResults[prevIdx];
    await loadViewport(activeTabId, m.line);
    setCursor({ line: m.line, column: m.col });
  }, [activeTabId, searchResults, currentMatchIndex, loadViewport, setCursor]);

  const handleReplace = useCallback(async (replacement: string) => {
    if (!activeTabId || searchResults.length === 0) return;
    try {
      // Replace the current match
      const m = searchResults[currentMatchIndex];
      await api.replaceRange(activeTabId, m.line, m.col, m.line, m.col + m.length, replacement);
      // Re-search
      const results = await api.searchQuery(activeTabId, searchQuery, searchOptions);
      setSearchResults(results);
      if (results.length > 0) {
        const nextIdx = Math.min(currentMatchIndex, results.length - 1);
        setCurrentMatchIndex(nextIdx);
        const nm = results[nextIdx];
        await loadViewport(activeTabId, nm.line);
        setCursor({ line: nm.line, column: nm.col });
      }
    } catch (e) {
      setError(String(e));
    }
  }, [activeTabId, searchResults, currentMatchIndex, searchQuery, searchOptions, loadViewport, setCursor]);

  const handleReplaceAll = useCallback(async (replacement: string) => {
    if (!activeTabId || !searchQuery) return;
    try {
      const count = await api.replaceAll(activeTabId, searchQuery, replacement, searchOptions);
      setSearchResults([]);
      setCurrentMatchIndex(0);
      // Refresh viewport
      await loadViewport(activeTabId, cursor.line);
      alert(`Replaced ${count} occurrences`);
    } catch (e) {
      setError(String(e));
    }
  }, [activeTabId, searchQuery, searchOptions, cursor.line, loadViewport]);

  // ─── Keyboard shortcuts ───────────────────────────────────

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
        setShowGotoDialog(true);
      }
      // Cmd/Ctrl + F: Search
      if ((e.metaKey || e.ctrlKey) && e.key === "f") {
        e.preventDefault();
        setShowSearchBar(true);
      }
      // Cmd/Ctrl + H: Search & Replace
      if ((e.metaKey || e.ctrlKey) && e.key === "h") {
        e.preventDefault();
        setShowSearchBar(true);
      }
      // Escape: Close search/goto
      if (e.key === "Escape") {
        setShowSearchBar(false);
        setShowGotoDialog(false);
      }
      // Cmd/Ctrl + W: Close tab
      if ((e.metaKey || e.ctrlKey) && e.key === "w") {
        e.preventDefault();
        if (activeTabId) {
          closeTab(activeTabId);
        }
      }
      // Cmd/Ctrl + S: Save
      if ((e.metaKey || e.ctrlKey) && e.key === "s") {
        e.preventDefault();
        handleSave();
      }
      // Cmd/Ctrl + Shift + S: Save As
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === "S") {
        e.preventDefault();
        handleSaveAs();
      }
      // Cmd/Ctrl + Z: Undo
      if ((e.metaKey || e.ctrlKey) && !e.shiftKey && e.key === "z") {
        e.preventDefault();
        handleUndo();
      }
      // Cmd/Ctrl + Shift + Z or Cmd/Ctrl + Y: Redo
      if (((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === "Z") ||
          ((e.metaKey || e.ctrlKey) && e.key === "y")) {
        e.preventDefault();
        handleRedo();
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [openFile, handleGotoLine, closeTab, handleSave, handleSaveAs, handleUndo, handleRedo, activeTabId]);

  return {
    activeTabId,
    tabs,
    viewport,
    cursor,
    loading,
    error,
    canUndo,
    canRedo,
    showGotoDialog,
    closeGotoDialog: () => setShowGotoDialog(false),
    showSearchBar,
    searchResults,
    currentMatchIndex,
    darkMode,
    toggleDarkMode,
    openFile,
    closeTab,
    switchTab,
    requestViewport,
    handleGotoLine,
    setCursor,
    handleInsertText,
    handleDeleteRange,
    handleReplaceRange,
    handleUndo,
    handleRedo,
    handleSave,
    handleSaveAs,
    handleSearch,
    handleSearchNext,
    handleSearchPrev,
    handleReplace,
    handleReplaceAll,
    closeSearchBar: () => setShowSearchBar(false),
  };
}
