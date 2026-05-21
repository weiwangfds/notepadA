import { invoke } from "@tauri-apps/api/core";
import { open as dialogOpen, save as dialogSave } from "@tauri-apps/plugin-dialog";
import type { OpenFileResult, ViewportData, TabInfo, LineCountInfo, EditResult } from "../types/editor";

export async function openFile(path: string): Promise<OpenFileResult> {
  return invoke<OpenFileResult>("open_file", { path });
}

export async function closeFile(tabId: string): Promise<void> {
  return invoke("close_file", { tabId });
}

export async function getTabs(): Promise<TabInfo[]> {
  return invoke<TabInfo[]>("get_tabs");
}

export async function getViewport(tabId: string, startLine: number, lineCount: number): Promise<ViewportData> {
  return invoke<ViewportData>("get_viewport", { tabId, startLine, lineCount });
}

export async function gotoLine(tabId: string, line: number): Promise<ViewportData> {
  return invoke<ViewportData>("goto_line", { tabId, line });
}

export async function getLineCount(tabId: string): Promise<LineCountInfo> {
  return invoke<LineCountInfo>("get_line_count", { tabId });
}

// ─── Edit commands ──────────────────────────────────────────

export async function insertText(
  tabId: string,
  line: number,
  col: number,
  text: string,
): Promise<EditResult> {
  return invoke<EditResult>("insert_text", { tabId, line, col, text });
}

export async function deleteRange(
  tabId: string,
  startLine: number,
  startCol: number,
  endLine: number,
  endCol: number,
): Promise<EditResult> {
  return invoke<EditResult>("delete_range", { tabId, startLine, startCol, endLine, endCol });
}

export async function replaceRange(
  tabId: string,
  startLine: number,
  startCol: number,
  endLine: number,
  endCol: number,
  text: string,
): Promise<EditResult> {
  return invoke<EditResult>("replace_range", { tabId, startLine, startCol, endLine, endCol, text });
}

export async function undoEdit(tabId: string, currentLine: number): Promise<EditResult> {
  return invoke<EditResult>("undo", { tabId, currentLine });
}

export async function redoEdit(tabId: string, currentLine: number): Promise<EditResult> {
  return invoke<EditResult>("redo", { tabId, currentLine });
}

export async function saveFile(tabId: string): Promise<void> {
  return invoke("save_file", { tabId });
}

export async function saveFileAs(tabId: string, path: string): Promise<void> {
  return invoke("save_file_as", { tabId, path });
}

// ─── Dialogs ────────────────────────────────────────────────

export async function showOpenFileDialog(): Promise<string | null> {
  const selected = await dialogOpen({
    multiple: false,
    directory: false,
    title: "Open File",
  });
  return selected as string | null;
}

export async function showSaveFileDialog(): Promise<string | null> {
  const selected = await dialogSave({
    title: "Save File As",
  });
  return selected as string | null;
}
