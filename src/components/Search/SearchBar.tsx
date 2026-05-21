import { useState, useRef, useEffect, useCallback } from "react";

interface Props {
  onSearch: (query: string, options: { case_sensitive: boolean; whole_word: boolean; regex: boolean }) => void;
  onNext: () => void;
  onPrev: () => void;
  onReplace: (replacement: string) => void;
  onReplaceAll: (replacement: string) => void;
  onClose: () => void;
  matchCount: number;
  currentMatch: number;
}

export default function SearchBar({
  onSearch,
  onNext,
  onPrev,
  onReplace,
  onReplaceAll,
  onClose,
  matchCount,
  currentMatch,
}: Props) {
  const [query, setQuery] = useState("");
  const [replacement, setReplacement] = useState("");
  const [showReplace, setShowReplace] = useState(false);
  const [caseSensitive, setCaseSensitive] = useState(false);
  const [wholeWord, setWholeWord] = useState(false);
  const [useRegex, setUseRegex] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const handleSearch = useCallback(() => {
    if (query) {
      onSearch(query, { case_sensitive: caseSensitive, whole_word: wholeWord, regex: useRegex });
    }
  }, [query, caseSensitive, wholeWord, useRegex, onSearch]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      onClose();
    } else if (e.key === "Enter") {
      if (e.shiftKey) {
        onPrev();
      } else {
        if (matchCount === 0) {
          handleSearch();
        } else {
          onNext();
        }
      }
    }
  };

  return (
    <div className="search-bar" onKeyDown={handleKeyDown}>
      <div className="search-row">
        <button
          className="search-toggle-btn"
          onClick={() => setShowReplace(!showReplace)}
          title="Toggle Replace"
        >
          {showReplace ? "▼" : "▶"}
        </button>

        <input
          ref={inputRef}
          type="text"
          className="search-input"
          placeholder="Search..."
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            onSearch(e.target.value, { case_sensitive: caseSensitive, whole_word: wholeWord, regex: useRegex });
          }}
        />

        <div className="search-options">
          <button
            className={`search-opt-btn${caseSensitive ? " active" : ""}`}
            onClick={() => { setCaseSensitive(!caseSensitive); }}
            title="Case Sensitive"
          >
            Aa
          </button>
          <button
            className={`search-opt-btn${wholeWord ? " active" : ""}`}
            onClick={() => { setWholeWord(!wholeWord); }}
            title="Whole Word"
          >
            W
          </button>
          <button
            className={`search-opt-btn${useRegex ? " active" : ""}`}
            onClick={() => { setUseRegex(!useRegex); }}
            title="Regex"
          >
            .*
          </button>
        </div>

        <span className="search-count">
          {matchCount > 0 ? `${currentMatch + 1} / ${matchCount}` : "No results"}
        </span>

        <button className="search-nav-btn" onClick={onPrev} title="Previous (Shift+Enter)">
          {"▲"}
        </button>
        <button className="search-nav-btn" onClick={onNext} title="Next (Enter)">
          {"▼"}
        </button>

        <button className="search-close-btn" onClick={onClose} title="Close (Esc)">
          {"✕"}
        </button>
      </div>

      {showReplace && (
        <div className="search-row">
          <div style={{ width: 24 }} />
          <input
            type="text"
            className="search-input"
            placeholder="Replace..."
            value={replacement}
            onChange={(e) => setReplacement(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                onReplace(replacement);
              }
            }}
          />
          <button
            className="search-action-btn"
            onClick={() => onReplace(replacement)}
            title="Replace"
          >
            Replace
          </button>
          <button
            className="search-action-btn"
            onClick={() => onReplaceAll(replacement)}
            title="Replace All"
          >
            All
          </button>
        </div>
      )}
    </div>
  );
}
