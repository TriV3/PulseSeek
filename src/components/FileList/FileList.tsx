import { useEffect, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { moveToTrash } from "../../api/commandEnvelope";
import type { BrowserEntry } from "../FolderTree/folderTreeTypes";
import type { PlaybackSelectionStatus } from "../../hooks/usePlaybackSelection";
import { useKeyboardShortcuts } from "../../hooks/useKeyboardShortcuts";
import { ConfirmDialog } from "../ConfirmDialog/ConfirmDialog";
import {
  DEFAULT_FILE_SORT,
  type FileSort,
  type FileSortField,
} from "./fileSort";
import "./FileList.css";
import "../ConfirmDialog/ConfirmDialog.css";

const UNAVAILABLE = "—";

function formatDuration(durationMs: number | null): string {
  if (durationMs === null) return UNAVAILABLE;
  const totalSeconds = Math.floor(durationMs / 1_000);
  const hours = Math.floor(totalSeconds / 3_600);
  const minutes = Math.floor((totalSeconds % 3_600) / 60);
  const seconds = totalSeconds % 60;
  return hours > 0
    ? `${hours}:${minutes.toString().padStart(2, "0")}:${seconds.toString().padStart(2, "0")}`
    : `${minutes}:${seconds.toString().padStart(2, "0")}`;
}

function formatSize(sizeBytes: number | null): string {
  if (sizeBytes === null) return UNAVAILABLE;
  if (sizeBytes < 1_024) return `${sizeBytes} B`;
  const units = ["KiB", "MiB", "GiB", "TiB"];
  let value = sizeBytes / 1_024;
  let unitIndex = 0;
  while (value >= 1_024 && unitIndex < units.length - 1) {
    value /= 1_024;
    unitIndex += 1;
  }
  return `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 }).format(value)} ${units[unitIndex]}`;
}

function formatModified(modifiedAtMs: number | null): string {
  if (modifiedAtMs === null) return UNAVAILABLE;
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "short",
    timeStyle: "short",
  }).format(new Date(modifiedAtMs));
}

function formatChannels(channels: number | null): string {
  if (channels === null) return UNAVAILABLE;
  if (channels === 1) return "Mono";
  if (channels === 2) return "Stereo";
  return `${channels} ch`;
}

function formatSampleRate(sampleRate: number | null): string {
  if (sampleRate === null) return UNAVAILABLE;
  return `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 3 }).format(sampleRate / 1_000)} kHz`;
}

function metadataValue(value: string, isLoading: boolean) {
  if (isLoading && value === UNAVAILABLE) {
    return <span aria-label="Metadata loading">…</span>;
  }
  return value;
}

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
  playbackEntryId?: string | null;
  playbackStatus?: PlaybackSelectionStatus;
  playbackError?: string | null;
  /** Called with entries successfully moved to the OS trash. */
  onEntriesTrashed?: (entryIds: string[]) => void;
  /** Active sort; default name ascending when omitted. */
  sort?: FileSort;
  /** Called whenever the user changes the sort. */
  onSortChange?: (sort: FileSort) => void;
  /** Current folder search query; empty when no search is active. */
  searchQuery?: string;
  /** Called whenever the user changes the search query. */
  onSearchQueryChange?: (query: string) => void;
  /** Called when a matched folder row is activated for navigation. */
  onSelectFolder?: (path: string) => void;
}

/** Column headers that act as clickable sort controls. */
const SORTABLE_HEADERS: Array<{
  label: string;
  field: FileSortField;
}> = [
  { label: "Name", field: "name" },
  { label: "Duration", field: "duration" },
  { label: "Size", field: "size" },
  { label: "Modified", field: "date" },
];

export function FileList({
  entries,
  selectedPath,
  isLoading,
  error,
  onFileSelect,
  playbackEntryId = null,
  playbackStatus = "idle",
  playbackError = null,
  onEntriesTrashed,
  sort = DEFAULT_FILE_SORT,
  onSortChange,
  searchQuery = "",
  onSearchQueryChange,
  onSelectFolder,
}: FileListProps) {
  const parentRef = useRef<HTMLDivElement>(null);
  const [selectedEntryId, setSelectedEntryId] = useState<string | null>(null);
  const [activeEntryId, setActiveEntryId] = useState<string | null>(null);
  const [trashTarget, setTrashTarget] = useState<BrowserEntry | null>(null);
  const [trashStatus, setTrashStatus] = useState<"idle" | "moving" | "error">(
    "idle",
  );
  const [trashError, setTrashError] = useState<string | null>(null);
  const rowRefs = useRef(new Map<string, HTMLDivElement>());

  const requestSort = (field: FileSortField) => {
    if (!onSortChange) return;
    const next: FileSort =
      sort.field === field
        ? { field, direction: sort.direction === "asc" ? "desc" : "asc" }
        : { field, direction: "asc" };
    onSortChange(next);
  };

  useEffect(() => {
    setSelectedEntryId(null);
    setTrashTarget(null);
    setTrashStatus("idle");
    setTrashError(null);
  }, [selectedPath]);

  useEffect(() => {
    if (
      playbackEntryId &&
      entries.some((entry) => entry.id === playbackEntryId)
    ) {
      setSelectedEntryId(playbackEntryId);
      setActiveEntryId(playbackEntryId);
    }
  }, [entries, playbackEntryId]);

  const selectEntry = (entry: BrowserEntry) => {
    setSelectedEntryId(entry.id);
    onFileSelect?.(entry);
  };

  const requestTrash = (entry: BrowserEntry) => {
    setTrashError(null);
    setTrashStatus("idle");
    setTrashTarget(entry);
  };

  const confirmTrash = async () => {
    if (!trashTarget || trashStatus === "moving") return;
    setTrashStatus("moving");
    setTrashError(null);
    try {
      const [result] = await moveToTrash([trashTarget.id]);
      if (result?.ok) {
        onEntriesTrashed?.([trashTarget.id]);
        setSelectedEntryId(null);
        setTrashTarget(null);
        setTrashStatus("idle");
        return;
      }
      setTrashStatus("error");
      setTrashError(result?.message ?? "Unable to move file to Trash.");
    } catch (error: unknown) {
      setTrashStatus("error");
      setTrashError(
        error instanceof Error
          ? error.message
          : "Unable to move file to Trash.",
      );
    }
  };

  useKeyboardShortcuts({
    onMoveToTrash: () => {
      const selected = entries.find(
        (entry) => entry.id === selectedEntryIdForFolder,
      );
      if (selected) requestTrash(selected);
    },
  });

  const selectedEntryIdForFolder = entries.some(
    (entry) => entry.id === selectedEntryId,
  )
    ? selectedEntryId
    : null;
  const activeEntryIdForFolder = entries.some(
    (entry) => entry.id === activeEntryId,
  )
    ? activeEntryId
    : (entries[0]?.id ?? null);

  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: entries.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 32,
    overscan: 10,
  });

  const focusEntry = (index: number) => {
    if (entries.length === 0) return;
    const nextIndex = Math.max(0, Math.min(index, entries.length - 1));
    const nextEntry = entries[nextIndex];
    setActiveEntryId(nextEntry.id);
    virtualizer.scrollToIndex(nextIndex, { align: "auto" });
    window.setTimeout(() => rowRefs.current.get(nextEntry.id)?.focus(), 0);
  };

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

  // The actions bar (search, sort, trash) stays visible whenever a folder is
  // selected so a search that matches nothing never hides the search box.

  return (
    <div className="file-list" role="region" aria-label="File list">
      <div className="file-list-actions">
        <button
          type="button"
          onClick={() => {
            const selected = entries.find(
              (entry) => entry.id === selectedEntryIdForFolder,
            );
            if (selected) requestTrash(selected);
          }}
          disabled={!selectedEntryIdForFolder || trashStatus === "moving"}
        >
          Move to Trash
        </button>
        <input
          type="search"
          className="file-list-search"
          aria-label="Search files"
          placeholder="Search files"
          value={searchQuery}
          onChange={(event) => onSearchQueryChange?.(event.target.value)}
        />
        {trashStatus === "moving" ? (
          <span role="status" aria-live="polite">
            Moving to Trash…
          </span>
        ) : null}
        {trashError ? (
          <span className="file-list-trash-error" role="alert">
            {trashError}
          </span>
        ) : null}
      </div>

      {error ? (
        <div className="file-list-state file-list-state--error">
          <p className="file-list-placeholder file-list-placeholder--error">
            {error}
          </p>
        </div>
      ) : isLoading && entries.length === 0 ? (
        <div className="file-list-state file-list-state--loading">
          <p className="file-list-placeholder">Loading&#8230;</p>
        </div>
      ) : entries.length === 0 ? (
        <div className="file-list-state file-list-state--empty">
          <p className="file-list-placeholder">
            {searchQuery.trim() === ""
              ? "(no playable files)"
              : `(no files match “${searchQuery.trim()}”)`}
          </p>
        </div>
      ) : (
        <div
          ref={parentRef}
          className="file-list-viewport"
          role="grid"
          aria-label="Playable files"
          aria-colcount={9}
          aria-rowcount={entries.length + 1}
        >
          <div className="file-list-header" role="row">
            {SORTABLE_HEADERS.map((header) => {
              const isActive = sort.field === header.field;
              return (
                <button
                  key={header.field}
                  type="button"
                  role="columnheader"
                  className="file-list-sort-button"
                  aria-label={header.label}
                  aria-sort={
                    isActive
                      ? sort.direction === "asc"
                        ? "ascending"
                        : "descending"
                      : undefined
                  }
                  onClick={() => requestSort(header.field)}
                >
                  <span>{header.label}</span>
                  {isActive ? (
                    <span aria-hidden="true">
                      {sort.direction === "asc" ? " ↑" : " ↓"}
                    </span>
                  ) : null}
                </button>
              );
            })}
            <span role="columnheader">Channels</span>
            <span role="columnheader">Sample rate</span>
            <span role="columnheader">Bit depth</span>
            <span role="columnheader">Codec</span>
            <span role="columnheader">Status</span>
          </div>
          <div
            className="file-list-inner"
            style={{ height: `${virtualizer.getTotalSize()}px` }}
          >
            {virtualizer.getVirtualItems().map((virtualRow) => {
              const entry = entries[virtualRow.index];

              if (entry.kind === "folder") {
                return (
                  <div
                    key={entry.id}
                    ref={(node) => {
                      if (node) rowRefs.current.set(entry.id, node);
                      else rowRefs.current.delete(entry.id);
                    }}
                    data-row-id={entry.id}
                    className="file-list-row file-list-row--folder"
                    style={{
                      height: `${virtualRow.size}px`,
                      transform: `translateY(${virtualRow.start}px)`,
                    }}
                    onClick={() => onSelectFolder?.(entry.id)}
                    onKeyDown={(event) => {
                      if (event.key === "ArrowDown") {
                        event.preventDefault();
                        focusEntry(virtualRow.index + 1);
                        return;
                      }
                      if (event.key === "ArrowUp") {
                        event.preventDefault();
                        focusEntry(virtualRow.index - 1);
                        return;
                      }
                      if (event.key === "Home") {
                        event.preventDefault();
                        focusEntry(0);
                        return;
                      }
                      if (event.key === "End") {
                        event.preventDefault();
                        focusEntry(entries.length - 1);
                        return;
                      }
                      if (event.key === "Enter" || event.key === " ") {
                        event.preventDefault();
                        onSelectFolder?.(entry.id);
                      }
                    }}
                    role="row"
                    aria-rowindex={virtualRow.index + 2}
                    aria-selected="false"
                    tabIndex={activeEntryIdForFolder === entry.id ? 0 : -1}
                    aria-label={`Open folder ${entry.name}`}
                  >
                    <span className="file-list-row-name" role="gridcell">
                      {entry.name}
                    </span>
                    <span role="gridcell" aria-hidden="true"></span>
                    <span role="gridcell" aria-hidden="true"></span>
                    <span role="gridcell" aria-hidden="true"></span>
                    <span role="gridcell" aria-hidden="true"></span>
                    <span role="gridcell" aria-hidden="true"></span>
                    <span role="gridcell" aria-hidden="true"></span>
                    <span role="gridcell" aria-hidden="true"></span>
                    <span role="gridcell">Folder</span>
                  </div>
                );
              }

              const metadata = entry.metadata;
              const metadataLoading = isLoading && metadata == null;
              const isPlaybackEntry = playbackEntryId === entry.id;
              const statusLabel = isPlaybackEntry
                ? playbackStatus === "loading"
                  ? "Loading"
                  : playbackStatus === "playing"
                    ? "Playing"
                    : playbackStatus === "failed"
                      ? "Failed"
                      : ""
                : "";
              return (
                <div
                  key={entry.id}
                  ref={(node) => {
                    if (node) rowRefs.current.set(entry.id, node);
                    else rowRefs.current.delete(entry.id);
                  }}
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
                    if (event.key === "Delete" || event.key === "Backspace") {
                      event.preventDefault();
                      if (trashStatus !== "moving") requestTrash(entry);
                      return;
                    }
                    if (event.key === "ArrowDown") {
                      event.preventDefault();
                      focusEntry(virtualRow.index + 1);
                      return;
                    }
                    if (event.key === "ArrowUp") {
                      event.preventDefault();
                      focusEntry(virtualRow.index - 1);
                      return;
                    }
                    if (event.key === "Home") {
                      event.preventDefault();
                      focusEntry(0);
                      return;
                    }
                    if (event.key === "End") {
                      event.preventDefault();
                      focusEntry(entries.length - 1);
                      return;
                    }
                    if (event.key === "Enter" || event.key === " ") {
                      event.preventDefault();
                      selectEntry(entry);
                    }
                  }}
                  role="row"
                  aria-rowindex={virtualRow.index + 2}
                  aria-selected={selectedEntryIdForFolder === entry.id}
                  tabIndex={activeEntryIdForFolder === entry.id ? 0 : -1}
                  aria-label={`${entry.name}${statusLabel ? ` ${statusLabel}` : ""}`}
                  aria-describedby={
                    isPlaybackEntry && playbackError
                      ? "file-list-playback-error"
                      : undefined
                  }
                >
                  <span className="file-list-row-name" role="gridcell">
                    {entry.name}
                  </span>
                  <span role="gridcell">
                    {metadataValue(
                      formatDuration(metadata?.duration_ms ?? null),
                      metadataLoading,
                    )}
                  </span>
                  <span role="gridcell">
                    {metadataValue(
                      formatSize(metadata?.size_bytes ?? null),
                      metadataLoading,
                    )}
                  </span>
                  <span role="gridcell">
                    {metadataValue(
                      formatModified(metadata?.modified_at_ms ?? null),
                      metadataLoading,
                    )}
                  </span>
                  <span role="gridcell">
                    {metadataValue(
                      formatChannels(metadata?.channels ?? null),
                      metadataLoading,
                    )}
                  </span>
                  <span role="gridcell">
                    {metadataValue(
                      formatSampleRate(metadata?.sample_rate ?? null),
                      metadataLoading,
                    )}
                  </span>
                  <span role="gridcell">
                    {metadataValue(
                      metadata?.bit_depth == null
                        ? UNAVAILABLE
                        : `${metadata.bit_depth}-bit`,
                      metadataLoading,
                    )}
                  </span>
                  <span role="gridcell">
                    {metadataValue(
                      metadata?.codec ?? UNAVAILABLE,
                      metadataLoading,
                    )}
                  </span>
                  <span role="gridcell">{statusLabel}</span>
                </div>
              );
            })}
          </div>
        </div>
      )}
      {playbackError &&
      playbackEntryId &&
      entries.some((entry) => entry.id === playbackEntryId) ? (
        <p
          className="file-list-playback-error"
          id="file-list-playback-error"
          role="alert"
        >
          {playbackError}
        </p>
      ) : null}
      <ConfirmDialog
        open={trashTarget !== null}
        title="Move to Trash?"
        message={
          trashTarget
            ? `Move “${trashTarget.name}” to the operating system Trash?`
            : ""
        }
        confirmLabel={trashStatus === "moving" ? "Moving…" : "Move to Trash"}
        confirmDisabled={trashStatus === "moving"}
        onConfirm={() => {
          void confirmTrash();
        }}
        onCancel={() => {
          if (trashStatus !== "moving") {
            setTrashTarget(null);
            setTrashError(null);
          }
        }}
      />
    </div>
  );
}
