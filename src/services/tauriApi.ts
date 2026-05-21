import { invoke } from "@tauri-apps/api/core";
import { open as dialogOpen } from "@tauri-apps/plugin-dialog";
import type { OpenFileResult, ViewportData, TabInfo, LineCountInfo } from "../types/editor";

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

export async function showOpenFileDialog(): Promise<string | null> {
  const selected = await dialogOpen({
    multiple: false,
    directory: false,
    title: "Open File",
  });
  return selected as string | null;
}
