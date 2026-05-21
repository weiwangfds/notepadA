/** Result from opening a file */
export interface OpenFileResult {
  tab_id: string;
  file_name: string;
  path: string;
  file_size: number;
  total_lines: number;
  encoding: string;
  has_bom: boolean;
  line_ending: string;
}

/** Viewport data returned from the backend */
export interface ViewportData {
  lines: string[];
  start_line: number;
  total_lines: number;
  file_size: number;
  encoding: string;
  has_bom: boolean;
  line_ending: string;
  index_progress: number;
  index_complete: boolean;
  file_name: string;
}

/** Tab info for the tab bar */
export interface TabInfo {
  id: string;
  file_name: string;
  path: string;
  dirty: boolean;
}

/** Line count info */
export interface LineCountInfo {
  total_lines: number;
  file_size: number;
  index_progress: number;
  index_complete: boolean;
}

/** Editor cursor position */
export interface CursorPosition {
  line: number;
  column: number;
}

/** Result from an edit operation */
export interface EditResult {
  viewport: ViewportData;
  cursor_line: number;
  cursor_col: number;
  dirty: boolean;
  can_undo: boolean;
  can_redo: boolean;
}

/** Format byte size to human-readable string */
export function formatFileSize(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  const val = bytes / Math.pow(1024, i);
  return `${val.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}
