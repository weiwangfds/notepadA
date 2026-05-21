import { useRef, useEffect } from "react";
import { useEditor } from "./hooks/useEditor";
import MenuBar from "./components/MenuBar/MenuBar";
import TabBar from "./components/TabBar/TabBar";
import EditorView from "./components/Editor/EditorView";
import StatusBar from "./components/StatusBar/StatusBar";
import GotoLineDialog from "./components/Dialogs/GotoLineDialog";
import SearchBar from "./components/Search/SearchBar";

function App() {
  const {
    activeTabId,
    tabs,
    viewport,
    cursor,
    loading,
    error,
    showGotoDialog,
    showSearchBar,
    searchResults,
    currentMatchIndex,
    openFile,
    closeTab,
    switchTab,
    requestViewport,
    setCursor,
    handleGotoLine,
    handleInsertText,
    handleDeleteRange,
    handleSave,
    handleSaveAs,
    handleUndo,
    handleRedo,
    handleSearch,
    handleSearchNext,
    handleSearchPrev,
    handleReplace,
    handleReplaceAll,
    closeGotoDialog,
    closeSearchBar,
    darkMode,
    toggleDarkMode,
  } = useEditor();

  // Auto-open file if specified in URL hash
  const autoOpenDone = useRef(false);
  useEffect(() => {
    if (autoOpenDone.current) return;
    const hash = window.location.hash;
    const match = hash.match(/#file=(.+)/);
    if (match) {
      autoOpenDone.current = true;
      const filePath = decodeURIComponent(match[1]);
      openFile(filePath);
    }
  }, [openFile]);

  const handleScroll = (startLine: number) => {
    if (activeTabId) {
      requestViewport(activeTabId, startLine);
    }
  };

  // Drag-drop: handle file drops on the editor
  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const files = e.dataTransfer.files;
    if (files.length > 0) {
      const file = files[0] as File & { path?: string };
      const filePath = file.path;
      if (filePath) {
        openFile(filePath);
      }
    }
  };

  return (
    <div className="app-root">
      <MenuBar
        onOpenFile={openFile}
        onSave={handleSave}
        onSaveAs={handleSaveAs}
        onUndo={handleUndo}
        onRedo={handleRedo}
        darkMode={darkMode}
        onToggleTheme={toggleDarkMode}
      />
      <TabBar
        tabs={tabs}
        activeTabId={activeTabId}
        onSwitchTab={switchTab}
        onCloseTab={closeTab}
      />

      {error && (
        <div className="error-bar">{error}</div>
      )}

      <div className="editor-wrapper" onDragOver={handleDragOver} onDrop={handleDrop}>
        {viewport ? (
          <>
            <EditorView
              viewport={viewport}
              cursor={cursor}
              onScroll={handleScroll}
              onCursorChange={setCursor}
              onInsertText={handleInsertText}
              onDeleteRange={handleDeleteRange}
            />
            {showSearchBar && (
              <SearchBar
                onSearch={handleSearch}
                onNext={handleSearchNext}
                onPrev={handleSearchPrev}
                onReplace={handleReplace}
                onReplaceAll={handleReplaceAll}
                onClose={closeSearchBar}
                matchCount={searchResults.length}
                currentMatch={currentMatchIndex}
              />
            )}
          </>
        ) : (
          <div className="editor-container">
            <div className="editor-empty">
              <div className="editor-empty-icon">&#128196;</div>
              <div className="editor-empty-text">NotePadA</div>
              <div className="editor-empty-hint">
                Press <kbd>Ctrl</kbd>+<kbd>O</kbd> to open a file
              </div>
              <div className="editor-empty-hint">
                Supports files up to 100 GB
              </div>
            </div>
          </div>
        )}
      </div>

      <StatusBar viewport={viewport} cursor={cursor} loading={loading} />

      {showGotoDialog && viewport && (
        <GotoLineDialog
          totalLines={viewport.total_lines}
          onGoto={handleGotoLine}
          onClose={closeGotoDialog}
        />
      )}
    </div>
  );
}

export default App;
