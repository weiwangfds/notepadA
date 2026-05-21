import { useState, useRef, useEffect } from "react";

interface Props {
  totalLines: number;
  onGoto: (line: number) => void;
  onClose: () => void;
}

export default function GotoLineDialog({ totalLines, onGoto, onClose }: Props) {
  const [value, setValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const handleSubmit = (e: React.SyntheticEvent) => {
    e.preventDefault();
    const line = parseInt(value, 10);
    if (!isNaN(line) && line >= 1 && line <= totalLines) {
      onGoto(line - 1); // Convert 1-based to 0-based
    }
    onClose();
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      onClose();
    }
  };

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="dialog" onClick={(e) => e.stopPropagation()} onKeyDown={handleKeyDown}>
        <form onSubmit={handleSubmit}>
          <div className="dialog-title">Go to Line</div>
          <div className="dialog-body">
            <input
              ref={inputRef}
              type="text"
              className="dialog-input"
              placeholder={`Line number (1 - ${totalLines})`}
              value={value}
              onChange={(e) => setValue(e.target.value)}
            />
          </div>
          <div className="dialog-footer">
            <button type="button" className="dialog-btn" onClick={onClose}>Cancel</button>
            <button type="submit" className="dialog-btn dialog-btn-primary">Go</button>
          </div>
        </form>
      </div>
    </div>
  );
}
