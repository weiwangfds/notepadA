import { useRef, useEffect } from "react";
import { useEditor } from "./hooks/useEditor";
import MenuBar from "./components/MenuBar/MenuBar";
import TabBar from "./components/TabBar/TabBar";
import EditorView from "./components/Editor/EditorView";
import StatusBar from "./components/StatusBar/StatusBar";
import GotoLineDialog from "./components/Dialogs/GotoLineDialog";

function App() {
  const {
    activeTabId,
    tabs,
    viewport,
    cursor,
    loading,
    error,
    showGotoDialog,
    closeGotoDialog,
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
  } = useEditor();

  // Auto-open file if specified in URL hash (e.g., #file=/tmp/test.txt)
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

  return (
    <div className="app-root">
      <MenuBar
        onOpenFile={openFile}
        onSave={handleSave}
        onSaveAs={handleSaveAs}
        onUndo={handleUndo}
        onRedo={handleRedo}
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

      <div className="editor-wrapper">
        {viewport ? (
          <EditorView
            viewport={viewport}
            cursor={cursor}
            onScroll={handleScroll}
            onCursorChange={setCursor}
            onInsertText={handleInsertText}
            onDeleteRange={handleDeleteRange}
          />
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
