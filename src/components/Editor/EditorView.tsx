import React, { useRef, useCallback, useEffect, useState, useMemo } from "react";
import type { ViewportData, CursorPosition } from "../../types/editor";

const LINE_HEIGHT = 22; // px per line
const OVERSCAN = 10; // extra lines rendered above/below visible area

interface Props {
  viewport: ViewportData;
  cursor: CursorPosition;
  onScroll: (startLine: number) => void;
  onCursorChange: (pos: CursorPosition) => void;
}

export default function EditorView({ viewport, cursor, onScroll, onCursorChange }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [containerHeight, setContainerHeight] = useState(600);
  const [scrollTop, setScrollTop] = useState(0);

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
    // Rough column estimate from click position
    const column = Math.floor(e.nativeEvent.offsetX / 8.4);
    onCursorChange({ line: lineNum, column });
  }, [onCursorChange]);

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

  const isEmpty = viewport.lines.length === 0;

  return (
    <div className="editor-container" ref={containerRef} onScroll={handleScroll}>
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
            {renderLines.map(({ lineNum, text }) => (
              <div
                key={lineNum}
                className={`editor-line${cursor.line === lineNum ? " editor-line-active" : ""}`}
                style={{
                  position: "absolute",
                  top: lineNum * LINE_HEIGHT,
                  height: LINE_HEIGHT,
                  lineHeight: `${LINE_HEIGHT}px`,
                  left: 0,
                  right: 0,
                }}
                onClick={(e) => handleLineClick(lineNum, e)}
              >
                <span className="editor-line-text">{text}</span>
              </div>
            ))}

            {/* Cursor */}
            {cursor.line >= renderStart && cursor.line < renderEnd && (
              <div
                className="editor-cursor"
                style={{
                  position: "absolute",
                  top: cursor.line * LINE_HEIGHT,
                  left: cursor.column * 8.4,
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
