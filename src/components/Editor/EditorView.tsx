import React, { useRef, useCallback, useEffect, useState, useMemo } from "react";
import type { ViewportData, CursorPosition } from "../../types/editor";

const LINE_HEIGHT = 22; // px per line
const OVERSCAN = 10; // extra lines rendered above/below visible area
const CHAR_WIDTH = 8.4; // approximate px per character

interface Props {
  viewport: ViewportData;
  cursor: CursorPosition;
  onScroll: (startLine: number) => void;
  onCursorChange: (pos: CursorPosition) => void;
  onInsertText: (line: number, col: number, text: string) => void;
  onDeleteRange: (startLine: number, startCol: number, endLine: number, endCol: number) => void;
}

export default function EditorView({
  viewport,
  cursor,
  onScroll,
  onCursorChange,
  onInsertText,
  onDeleteRange,
}: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const [containerHeight, setContainerHeight] = useState(600);
  const [scrollTop, setScrollTop] = useState(0);
  const [selection, setSelection] = useState<{ anchor: CursorPosition; focus: CursorPosition } | null>(null);

  // Observe container height
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        setContainerHeight(entry.contentRect.height);
      }
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  // Calculate visible range
  const totalLines = viewport.total_lines;
  const totalHeight = totalLines * LINE_HEIGHT;

  const firstVisibleLine = Math.floor(scrollTop / LINE_HEIGHT);
  const visibleCount = Math.ceil(containerHeight / LINE_HEIGHT) + 1;

  const renderStart = Math.max(0, firstVisibleLine - OVERSCAN);
  const renderEnd = Math.min(totalLines, firstVisibleLine + visibleCount + OVERSCAN);

  // Get lines to render from viewport data
  const renderLines = useMemo(() => {
    const lines: { lineNum: number; text: string }[] = [];
    for (let i = renderStart; i < renderEnd; i++) {
      const vpIndex = i - viewport.start_line;
      const text = vpIndex >= 0 && vpIndex < viewport.lines.length
        ? viewport.lines[vpIndex]
        : "";
      lines.push({ lineNum: i, text });
    }
    return lines;
  }, [renderStart, renderEnd, viewport.start_line, viewport.lines]);

  // Scroll handler - notify parent of the new visible start line
  const handleScroll = useCallback(() => {
    const el = containerRef.current;
    if (!el) return;
    const newScrollTop = el.scrollTop;
    setScrollTop(newScrollTop);

    const newStartLine = Math.floor(newScrollTop / LINE_HEIGHT);
    onScroll(newStartLine);
  }, [onScroll]);

  // Handle clicking a line to position cursor
  const handleLineClick = useCallback((lineNum: number, e: React.MouseEvent) => {
    const column = Math.floor(e.nativeEvent.offsetX / CHAR_WIDTH);
    onCursorChange({ line: lineNum, column });
    setSelection(null);
    // Focus the hidden input to capture keyboard events
    inputRef.current?.focus();
  }, [onCursorChange]);

  // Handle shift+click for selection
  const handleLineShiftClick = useCallback((lineNum: number, e: React.MouseEvent) => {
    const column = Math.floor(e.nativeEvent.offsetX / CHAR_WIDTH);
    const focus = { line: lineNum, column };
    setSelection({ anchor: cursor, focus });
    onCursorChange(focus);
  }, [cursor, onCursorChange]);

  // Get the effective selection range (always anchor before focus)
  const selectionRange = useMemo(() => {
    if (!selection) return null;
    const { anchor, focus } = selection;
    if (anchor.line < focus.line || (anchor.line === focus.line && anchor.column <= focus.column)) {
      return { start: anchor, end: focus };
    }
    return { start: focus, end: anchor };
  }, [selection]);

  // Max line number width for gutter
  const gutterWidth = useMemo(() => {
    const digits = Math.max(1, String(totalLines).length);
    return Math.max(50, digits * 10 + 20);
  }, [totalLines]);

  // Scrollbar ratio
  const thumbHeight = Math.max(30, (containerHeight / totalHeight) * containerHeight);
  const thumbTop = totalHeight > 0
    ? (scrollTop / totalHeight) * containerHeight
    : 0;

  // Handle custom scrollbar drag
  const dragging = useRef(false);
  const dragStartY = useRef(0);
  const dragStartScrollTop = useRef(0);

  const handleThumbMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    dragging.current = true;
    dragStartY.current = e.clientY;
    dragStartScrollTop.current = scrollTop;
  }, [scrollTop]);

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (!dragging.current) return;
      const delta = e.clientY - dragStartY.current;
      const scrollDelta = (delta / containerHeight) * totalHeight;
      const newScrollTop = Math.max(0, Math.min(totalHeight - containerHeight, dragStartScrollTop.current + scrollDelta));
      if (containerRef.current) {
        containerRef.current.scrollTop = newScrollTop;
      }
    };
    const handleMouseUp = () => {
      dragging.current = false;
    };
    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
    return () => {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
    };
  }, [containerHeight, totalHeight]);

  // ─── Keyboard input handling ──────────────────────────────

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    const totalLines = viewport.total_lines;

    switch (e.key) {
      case "ArrowLeft": {
        e.preventDefault();
        if (cursor.column > 0) {
          onCursorChange({ line: cursor.line, column: cursor.column - 1 });
        } else if (cursor.line > 0) {
          // Move to end of previous line
          const prevLineIdx = cursor.line - 1 - viewport.start_line;
          const prevLineText = prevLineIdx >= 0 && prevLineIdx < viewport.lines.length
            ? viewport.lines[prevLineIdx]
            : "";
          onCursorChange({ line: cursor.line - 1, column: prevLineText.length });
        }
        setSelection(null);
        break;
      }
      case "ArrowRight": {
        e.preventDefault();
        const curLineIdx = cursor.line - viewport.start_line;
        const curLineText = curLineIdx >= 0 && curLineIdx < viewport.lines.length
          ? viewport.lines[curLineIdx]
          : "";
        if (cursor.column < curLineText.length) {
          onCursorChange({ line: cursor.line, column: cursor.column + 1 });
        } else if (cursor.line < totalLines - 1) {
          onCursorChange({ line: cursor.line + 1, column: 0 });
        }
        setSelection(null);
        break;
      }
      case "ArrowUp": {
        e.preventDefault();
        if (cursor.line > 0) {
          onCursorChange({ line: cursor.line - 1, column: cursor.column });
        }
        setSelection(null);
        break;
      }
      case "ArrowDown": {
        e.preventDefault();
        if (cursor.line < totalLines - 1) {
          onCursorChange({ line: cursor.line + 1, column: cursor.column });
        }
        setSelection(null);
        break;
      }
      case "Home": {
        e.preventDefault();
        if (e.ctrlKey) {
          onCursorChange({ line: 0, column: 0 });
        } else {
          onCursorChange({ line: cursor.line, column: 0 });
        }
        setSelection(null);
        break;
      }
      case "End": {
        e.preventDefault();
        if (e.ctrlKey) {
          const lastLine = totalLines - 1;
          const lastLineIdx = lastLine - viewport.start_line;
          const lastLineText = lastLineIdx >= 0 && lastLineIdx < viewport.lines.length
            ? viewport.lines[lastLineIdx]
            : "";
          onCursorChange({ line: lastLine, column: lastLineText.length });
        } else {
          const curLineIdx = cursor.line - viewport.start_line;
          const curLineText = curLineIdx >= 0 && curLineIdx < viewport.lines.length
            ? viewport.lines[curLineIdx]
            : "";
          onCursorChange({ line: cursor.line, column: curLineText.length });
        }
        setSelection(null);
        break;
      }
      case "PageUp": {
        e.preventDefault();
        const pageSize = Math.floor(containerHeight / LINE_HEIGHT);
        const newLine = Math.max(0, cursor.line - pageSize);
        onCursorChange({ line: newLine, column: cursor.column });
        setSelection(null);
        break;
      }
      case "PageDown": {
        e.preventDefault();
        const pageSize = Math.floor(containerHeight / LINE_HEIGHT);
        const newLine = Math.min(totalLines - 1, cursor.line + pageSize);
        onCursorChange({ line: newLine, column: cursor.column });
        setSelection(null);
        break;
      }
      case "Backspace": {
        e.preventDefault();
        if (selectionRange) {
          onDeleteRange(selectionRange.start.line, selectionRange.start.column, selectionRange.end.line, selectionRange.end.column);
          onCursorChange(selectionRange.start);
          setSelection(null);
        } else if (cursor.column > 0) {
          onDeleteRange(cursor.line, cursor.column - 1, cursor.line, cursor.column);
          onCursorChange({ line: cursor.line, column: cursor.column - 1 });
        } else if (cursor.line > 0) {
          const prevLineIdx = cursor.line - 1 - viewport.start_line;
          const prevLineText = prevLineIdx >= 0 && prevLineIdx < viewport.lines.length
            ? viewport.lines[prevLineIdx]
            : "";
          onDeleteRange(cursor.line - 1, prevLineText.length, cursor.line, 0);
          onCursorChange({ line: cursor.line - 1, column: prevLineText.length });
        }
        break;
      }
      case "Delete": {
        e.preventDefault();
        if (selectionRange) {
          onDeleteRange(selectionRange.start.line, selectionRange.start.column, selectionRange.end.line, selectionRange.end.column);
          onCursorChange(selectionRange.start);
          setSelection(null);
        } else {
          const curLineIdx = cursor.line - viewport.start_line;
          const curLineText = curLineIdx >= 0 && curLineIdx < viewport.lines.length
            ? viewport.lines[curLineIdx]
            : "";
          if (cursor.column < curLineText.length) {
            onDeleteRange(cursor.line, cursor.column, cursor.line, cursor.column + 1);
          } else if (cursor.line < totalLines - 1) {
            onDeleteRange(cursor.line, curLineText.length, cursor.line + 1, 0);
          }
        }
        break;
      }
      case "Enter": {
        e.preventDefault();
        if (selectionRange) {
          onDeleteRange(selectionRange.start.line, selectionRange.start.column, selectionRange.end.line, selectionRange.end.column);
          onInsertText(selectionRange.start.line, selectionRange.start.column, "\n");
          onCursorChange({ line: selectionRange.start.line + 1, column: 0 });
          setSelection(null);
        } else {
          onInsertText(cursor.line, cursor.column, "\n");
          onCursorChange({ line: cursor.line + 1, column: 0 });
        }
        break;
      }
      case "Tab": {
        e.preventDefault();
        if (selectionRange) {
          onDeleteRange(selectionRange.start.line, selectionRange.start.column, selectionRange.end.line, selectionRange.end.column);
          onInsertText(selectionRange.start.line, selectionRange.start.column, "  ");
          onCursorChange({ line: selectionRange.start.line, column: selectionRange.start.column + 2 });
          setSelection(null);
        } else {
          onInsertText(cursor.line, cursor.column, "  ");
          onCursorChange({ line: cursor.line, column: cursor.column + 2 });
        }
        break;
      }
      default: {
        // Handle regular character input — delegated to the hidden input's onInput
        if (e.key.length === 1 && !e.metaKey && !e.ctrlKey) {
          // Let the hidden input handle it
        }
        break;
      }
    }
  }, [cursor, viewport, selectionRange, containerHeight, onCursorChange, onInsertText, onDeleteRange]);

  // Handle text input from the hidden input element
  const handleInput = useCallback((e: React.SyntheticEvent<HTMLInputElement>) => {
    const input = e.currentTarget;
    const text = input.value;
    if (text) {
      if (selectionRange) {
        onDeleteRange(selectionRange.start.line, selectionRange.start.column, selectionRange.end.line, selectionRange.end.column);
        onInsertText(selectionRange.start.line, selectionRange.start.column, text);
        onCursorChange({ line: selectionRange.start.line, column: selectionRange.start.column + text.length });
        setSelection(null);
      } else {
        onInsertText(cursor.line, cursor.column, text);
        onCursorChange({ line: cursor.line, column: cursor.column + text.length });
      }
      input.value = "";
    }
  }, [cursor, selectionRange, onInsertText, onDeleteRange, onCursorChange]);

  const isEmpty = viewport.lines.length === 0;

  return (
    <div
      className="editor-container"
      ref={containerRef}
      onScroll={handleScroll}
      onClick={() => inputRef.current?.focus()}
    >
      {/* Hidden input for capturing keyboard/IME input */}
      <input
        ref={inputRef}
        type="text"
        style={{
          position: "absolute",
          opacity: 0,
          width: 0,
          height: 0,
          overflow: "hidden",
          pointerEvents: "none",
        }}
        onKeyDown={handleKeyDown}
        onInput={handleInput}
        autoFocus
      />

      {isEmpty ? (
        <div className="editor-empty">
          <div className="editor-empty-icon">&#128196;</div>
          <div className="editor-empty-text">No file open</div>
          <div className="editor-empty-hint">Press Ctrl+O to open a file</div>
        </div>
      ) : (
        <div className="editor-content" style={{ height: totalHeight, position: "relative" }}>
          {/* Line number gutter */}
          <div
            className="editor-gutter"
            style={{
              position: "sticky",
              left: 0,
              top: 0,
              width: gutterWidth,
              height: totalHeight,
              zIndex: 2,
            }}
          >
            <div style={{ position: "relative", height: "100%" }}>
              {renderLines.map(({ lineNum }) => (
                <div
                  key={lineNum}
                  className="editor-line-number"
                  style={{
                    position: "absolute",
                    top: lineNum * LINE_HEIGHT,
                    height: LINE_HEIGHT,
                    lineHeight: `${LINE_HEIGHT}px`,
                    width: gutterWidth,
                  }}
                >
                  {lineNum + 1}
                </div>
              ))}
            </div>
          </div>

          {/* Text area */}
          <div
            className="editor-text"
            style={{
              position: "absolute",
              left: gutterWidth,
              top: 0,
              right: 0,
              height: totalHeight,
            }}
          >
            {renderLines.map(({ lineNum, text }) => {
              // Check if this line has selection
              const hasSelection = selectionRange &&
                lineNum >= selectionRange.start.line &&
                lineNum <= selectionRange.end.line;

              return (
                <div
                  key={lineNum}
                  className={`editor-line${cursor.line === lineNum ? " editor-line-active" : ""}${hasSelection ? " editor-line-selected" : ""}`}
                  style={{
                    position: "absolute",
                    top: lineNum * LINE_HEIGHT,
                    height: LINE_HEIGHT,
                    lineHeight: `${LINE_HEIGHT}px`,
                    left: 0,
                    right: 0,
                  }}
                  onClick={(e) => {
                    if (e.shiftKey) {
                      handleLineShiftClick(lineNum, e);
                    } else {
                      handleLineClick(lineNum, e);
                    }
                  }}
                >
                  <span className="editor-line-text">{text}</span>
                </div>
              );
            })}

            {/* Cursor */}
            {cursor.line >= renderStart && cursor.line < renderEnd && (
              <div
                className="editor-cursor"
                style={{
                  position: "absolute",
                  top: cursor.line * LINE_HEIGHT,
                  left: cursor.column * CHAR_WIDTH,
                  height: LINE_HEIGHT,
                }}
              />
            )}
          </div>

          {/* Custom scrollbar track */}
          <div className="editor-scrollbar-track">
            <div
              className="editor-scrollbar-thumb"
              style={{
                height: thumbHeight,
                top: thumbTop,
              }}
              onMouseDown={handleThumbMouseDown}
            />
          </div>
        </div>
      )}
    </div>
  );
}
