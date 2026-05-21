interface Props {
  onOpenFile: () => void;
  onSave: () => void;
  onSaveAs: () => void;
  onUndo: () => void;
  onRedo: () => void;
  darkMode: boolean;
  onToggleTheme: () => void;
}

export default function MenuBar({ onOpenFile, onSave, onSaveAs, onUndo, onRedo, darkMode, onToggleTheme }: Props) {
  return (
    <div className="menubar">
      <div className="menubar-item" onClick={onOpenFile}>
        File
      </div>
      <div className="menubar-item" onClick={onSave}>
        Save
      </div>
      <div className="menubar-item" onClick={onSaveAs}>
        Save As
      </div>
      <div className="menubar-separator" />
      <div className="menubar-item" onClick={onUndo}>
        Undo
      </div>
      <div className="menubar-item" onClick={onRedo}>
        Redo
      </div>
      <div style={{ flex: 1 }} />
      <div className="menubar-item menubar-theme-btn" onClick={onToggleTheme} title="Toggle Dark/Light Mode">
        {darkMode ? "☀" : "🌙"}
      </div>
    </div>
  );
}
