import type { ViewportData, CursorPosition } from "../../types/editor";
import { formatFileSize } from "../../types/editor";

interface Props {
  viewport: ViewportData | null;
  cursor: CursorPosition;
  loading: boolean;
}

export default function StatusBar({ viewport, cursor, loading }: Props) {
  if (!viewport) {
    return (
      <div className="statusbar">
        <span className="statusbar-item">Ready</span>
      </div>
    );
  }

  const indexPct = Math.round(viewport.index_progress * 100);
  const indexLabel = viewport.index_complete
    ? "Indexed"
    : `Indexing ${indexPct}%`;

  return (
    <div className="statusbar">
      <span className="statusbar-item">
        Ln {cursor.line + 1}, Col {cursor.column + 1}
      </span>
      <span className="statusbar-item">
        {viewport.total_lines.toLocaleString()} lines
      </span>
      <span className="statusbar-item">
        {formatFileSize(viewport.file_size)}
      </span>
      <span className="statusbar-item">
        {viewport.encoding}
      </span>
      <span className="statusbar-item">
        {viewport.line_ending}
      </span>
      <span className="statusbar-item">
        {indexLabel}
      </span>
      {loading && (
        <span className="statusbar-item statusbar-loading">Loading...</span>
      )}
    </div>
  );
}
