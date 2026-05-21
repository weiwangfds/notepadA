interface Props {
  onOpenFile: () => void;
  onSave: () => void;
  onSaveAs: () => void;
  onUndo: () => void;
  onRedo: () => void;
}

export default function MenuBar({ onOpenFile, onSave, onSaveAs, onUndo, onRedo }: Props) {
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
      <div className="menubar-item menubar-separator" />
      <div className="menubar-item" onClick={onUndo}>
        Undo
      </div>
      <div className="menubar-item" onClick={onRedo}>
        Redo
      </div>
    </div>
  );
}
