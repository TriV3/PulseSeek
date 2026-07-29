import { useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { BrowserEntry } from "../FolderTree/folderTreeTypes";
import "./FileList.css";

interface FileListProps {
  /** Playable entries for the currently selected folder. */
  entries: BrowserEntry[];
  /** Currently selected folder path, or null. */
  selectedPath: string | null;
  /** Whether the folder is currently being enumerated. */
  isLoading: boolean;
  /** Error message for the folder, or null. */
  error: string | null;
  /** Called when a file row is clicked. */
  onFileSelect?: (entry: BrowserEntry) => void;
}

export function FileList({
  entries,
  selectedPath,
  isLoading,
  error,
  onFileSelect,
}: FileListProps) {
  const parentRef = useRef<HTMLDivElement>(null);
  const [selectedEntryId, setSelectedEntryId] = useState<string | null>(null);

  const selectEntry = (entry: BrowserEntry) => {
    setSelectedEntryId(entry.id);
    onFileSelect?.(entry);
  };

  const selectedEntryIdForFolder = entries.some(
    (entry) => entry.id === selectedEntryId,
  )
    ? selectedEntryId
    : null;

  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: entries.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 32,
    overscan: 10,
  });

  // ── No folder selected ──────────────────────────────────────────────

  if (!selectedPath) {
    return (
      <div
        className="file-list file-list--empty"
        role="region"
        aria-label="File list"
      >
        <p className="file-list-placeholder">
          Select a folder to browse files.
        </p>
      </div>
    );
  }

  // ── Error ───────────────────────────────────────────────────────────

  if (error) {
    return (
      <div
        className="file-list file-list--error"
        role="region"
        aria-label="File list"
      >
        <p className="file-list-placeholder file-list-placeholder--error">
          {error}
        </p>
      </div>
    );
  }

  // ── Loading (no entries yet) ────────────────────────────────────────

  if (isLoading && entries.length === 0) {
    return (
      <div
        className="file-list file-list--loading"
        role="region"
        aria-label="File list"
      >
        <p className="file-list-placeholder">Loading&#8230;</p>
      </div>
    );
  }

  // ── Empty folder ────────────────────────────────────────────────────

  if (!isLoading && entries.length === 0) {
    return (
      <div
        className="file-list file-list--empty"
        role="region"
        aria-label="File list"
      >
        <p className="file-list-placeholder">(no playable files)</p>
      </div>
    );
  }

  // ── Virtualized list ────────────────────────────────────────────────

  return (
    <div className="file-list" role="region" aria-label="File list">
      <div className="file-list-header">
        <span className="file-list-header-name">Name</span>
      </div>
      <div
        ref={parentRef}
        className="file-list-viewport"
        role="listbox"
        aria-label="Playable files"
      >
        <div
          className="file-list-inner"
          style={{ height: `${virtualizer.getTotalSize()}px` }}
        >
          {virtualizer.getVirtualItems().map((virtualRow) => {
            const entry = entries[virtualRow.index];
            return (
              <div
                key={entry.id}
                data-row-id={entry.id}
                className={`file-list-row${
                  selectedEntryIdForFolder === entry.id ? " selected" : ""
                }`}
                style={{
                  height: `${virtualRow.size}px`,
                  transform: `translateY(${virtualRow.start}px)`,
                }}
                onClick={() => selectEntry(entry)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    selectEntry(entry);
                  }
                }}
                role="option"
                aria-selected={selectedEntryIdForFolder === entry.id}
                tabIndex={0}
                aria-label={entry.name}
              >
                <span className="file-list-row-name">{entry.name}</span>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
