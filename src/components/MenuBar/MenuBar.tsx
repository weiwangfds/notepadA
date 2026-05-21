interface Props {
  onOpenFile: () => void;
}

export default function MenuBar({ onOpenFile }: Props) {
  return (
    <div className="menubar">
      <div className="menubar-item" onClick={onOpenFile}>
        File
      </div>
      <div className="menubar-item menubar-item-disabled">Edit</div>
      <div className="menubar-item menubar-item-disabled">View</div>
      <div className="menubar-item menubar-item-disabled">Help</div>
    </div>
  );
}
