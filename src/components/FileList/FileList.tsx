import { useEffect, useMemo, useRef, useState } from "react";
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
import { FORMAT_OPTIONS, type AudioFileFormat } from "./fileFilter";
import {
  MARK_FILTERS,
  MARK_FILTER_LABELS,
  SESSION_MARK_LABELS,
  SESSION_MARKS,
  selectMarkedEntryIds,
  type MarkFilter,
  type SessionMark,
  type SessionMarks,
} from "./sessionMarks";
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
  /** Active decoded-format filters; empty when every format is shown. */
  formatFilter?: AudioFileFormat[];
  /** Called whenever the user changes the format filter selection. */
  onFormatFilterChange?: (formats: AudioFileFormat[]) => void;
  /** Session marks keyed by entry id; purely in-memory (FR-LS-007). */
  marks?: SessionMarks;
  /** Applies `mark` to ids, or clears it when `mark` is null. */
  onMarkChange?: (ids: string[], mark: SessionMark | null) => void;
  /** Active mark filter; "all" shows every file. */
  markFilter?: MarkFilter;
  /** Called whenever the user changes the mark filter. */
  onMarkFilterChange?: (filter: MarkFilter) => void;
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
  formatFilter = [],
  onFormatFilterChange,
  marks = {},
  onMarkChange,
  markFilter = "all",
  onMarkFilterChange,
  onSelectFolder,
}: FileListProps) {
  const parentRef = useRef<HTMLDivElement>(null);
  // Multi-selection stores stable backend entry ids (FR-LS-005) so rows keep
  // their selection across sort, search, and format-filter changes.
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [selectionAnchorId, setSelectionAnchorId] = useState<string | null>(
    null,
  );
  const [activeEntryId, setActiveEntryId] = useState<string | null>(null);
  const [trashTarget, setTrashTarget] = useState<BrowserEntry | null>(null);
  const [trashStatus, setTrashStatus] = useState<"idle" | "moving" | "error">(
    "idle",
  );
  const [trashError, setTrashError] = useState<string | null>(null);
  const rowRefs = useRef(new Map<string, HTMLDivElement>());

  // Only folders and confirmed playable files may be listed. Unsupported or
  // inaccessible entries must never render, even if a stale backend still
  // streams them (FR-BR-004).
  const visibleEntries = useMemo(
    () =>
      entries.filter(
        (entry) => entry.kind === "folder" || entry.kind === "playable",
      ),
    [entries],
  );

  const toggleFormat = (format: AudioFileFormat) => {
    if (!onFormatFilterChange) return;
    const next = formatFilter.includes(format)
      ? formatFilter.filter((active) => active !== format)
      : [...formatFilter, format];
    onFormatFilterChange(next);
  };

  const resetFormats = () => {
    if (!onFormatFilterChange) return;
    onFormatFilterChange([]);
  };

  const applyMarkToSelection = (mark: SessionMark) => {
    if (selectedIds.size === 0) return;
    onMarkChange?.([...selectedIds], mark);
  };

  const clearMarkOnSelection = () => {
    if (selectedIds.size === 0) return;
    onMarkChange?.([...selectedIds], null);
  };

  // Playable entries that already carry a session mark, used to batch-select
  // marked files (FR-LS-008). Folder rows are never marked.
  const markedVisibleIds = useMemo(
    () => selectMarkedEntryIds(visibleEntries, marks),
    [visibleEntries, marks],
  );

  const selectMarked = () => {
    setSelectedIds(new Set(markedVisibleIds));
    setSelectionAnchorId(markedVisibleIds[0] ?? null);
  };

  const requestSort = (field: FileSortField) => {
    if (!onSortChange) return;
    const next: FileSort =
      sort.field === field
        ? { field, direction: sort.direction === "asc" ? "desc" : "asc" }
        : { field, direction: "asc" };
    onSortChange(next);
  };

  // Track the last playback entry id so streaming in more rows (which changes
  // `visibleEntries`) never wipes a multi-selection; only an actual track
  // change replaces the selection with the newly playing file. Resetting the
  // ref on folder change preserves the previous behavior where returning to a
  // folder re-selects the row that is still playing.
  const lastPlaybackEntryIdRef = useRef<string | null>(null);

  useEffect(() => {
    setSelectedIds(new Set());
    setSelectionAnchorId(null);
    lastPlaybackEntryIdRef.current = null;
    setTrashTarget(null);
    setTrashStatus("idle");
    setTrashError(null);
  }, [selectedPath]);

  useEffect(() => {
    if (
      playbackEntryId &&
      visibleEntries.some((entry) => entry.id === playbackEntryId)
    ) {
      if (lastPlaybackEntryIdRef.current !== playbackEntryId) {
        setSelectedIds(new Set([playbackEntryId]));
        setSelectionAnchorId(playbackEntryId);
        setActiveEntryId(playbackEntryId);
        lastPlaybackEntryIdRef.current = playbackEntryId;
      }
    }
  }, [playbackEntryId, visibleEntries]);

  const selectEntry = (entry: BrowserEntry) => {
    setSelectedIds(new Set([entry.id]));
    setSelectionAnchorId(entry.id);
    onFileSelect?.(entry);
  };

  const toggleEntry = (entry: BrowserEntry) => {
    setSelectedIds((current) => {
      const next = new Set(current);
      if (next.has(entry.id)) {
        next.delete(entry.id);
      } else {
        next.add(entry.id);
      }
      return next;
    });
    setSelectionAnchorId(entry.id);
    setActiveEntryId(entry.id);
  };

  const rangeSelect = (toEntry: BrowserEntry) => {
    const anchorId =
      selectionAnchorId &&
      visibleEntries.some((entry) => entry.id === selectionAnchorId)
        ? selectionAnchorId
        : selectedIds.size > 0
          ? [...selectedIds][0]
          : toEntry.id;
    const anchorIndex = visibleEntries.findIndex(
      (entry) => entry.id === anchorId,
    );
    const toIndex = visibleEntries.findIndex(
      (entry) => entry.id === toEntry.id,
    );
    if (anchorIndex === -1 || toIndex === -1) {
      setSelectedIds(new Set([toEntry.id]));
      setActiveEntryId(toEntry.id);
      return;
    }
    const [start, end] =
      anchorIndex <= toIndex ? [anchorIndex, toIndex] : [toIndex, anchorIndex];
    const next = new Set<string>();
    for (let i = start; i <= end; i += 1) {
      if (visibleEntries[i]?.kind === "playable") {
        next.add(visibleEntries[i].id);
      }
    }
    setSelectedIds(next);
    setActiveEntryId(toEntry.id);
  };

  const selectAllPlayable = () => {
    const all = visibleEntries.filter((entry) => entry.kind === "playable");
    setSelectedIds(new Set(all.map((entry) => entry.id)));
    setSelectionAnchorId(all[0]?.id ?? null);
  };

  const handlePlayableRowClick = (
    event: React.MouseEvent<HTMLDivElement>,
    entry: BrowserEntry,
  ) => {
    setActiveEntryId(entry.id);
    if (event.shiftKey) {
      event.preventDefault();
      rangeSelect(entry);
      return;
    }
    if (event.metaKey || event.ctrlKey) {
      event.preventDefault();
      toggleEntry(entry);
      return;
    }
    selectEntry(entry);
  };

  // Shared grid keys: select all (Cmd/Ctrl+A) and range extension via
  // Shift+arrows. Returns true when the event was handled so each row handler
  // can continue with its own keys.
  const handleGridKeyDown = (
    event: React.KeyboardEvent,
    index: number,
  ): boolean => {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "a") {
      event.preventDefault();
      selectAllPlayable();
      return true;
    }
    if (
      event.shiftKey &&
      (event.key === "ArrowDown" || event.key === "ArrowUp")
    ) {
      event.preventDefault();
      const nextIndex = Math.max(
        0,
        Math.min(
          index + (event.key === "ArrowDown" ? 1 : -1),
          visibleEntries.length - 1,
        ),
      );
      const nextEntry = visibleEntries[nextIndex];
      if (nextEntry) {
        setActiveEntryId(nextEntry.id);
        focusEntry(nextIndex);
        rangeSelect(nextEntry);
      }
      return true;
    }
    return false;
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
        setSelectedIds((current) => {
          const next = new Set(current);
          next.delete(trashTarget.id);
          return next;
        });
        if (selectionAnchorId === trashTarget.id) {
          setSelectionAnchorId(null);
        }
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
      const selected = visibleEntries.find(
        (entry) => entry.id === primarySelectedId,
      );
      if (selected) requestTrash(selected);
    },
    onMarkKeep: () => applyMarkToSelection("keep"),
    onMarkMaybe: () => applyMarkToSelection("maybe"),
    onMarkReject: () => applyMarkToSelection("reject"),
    onMarkFavorite: () => applyMarkToSelection("favorite"),
    onMarkClear: () => clearMarkOnSelection(),
  });

  // The primary selection is the anchor (last clicked or focused row). It
  // drives single-file actions such as Move to Trash; batch operations are
  // intentionally out of scope for this feature.
  const primarySelectedId = useMemo(() => {
    if (!selectionAnchorId || !selectedIds.has(selectionAnchorId)) {
      return selectedIds.size > 0 ? [...selectedIds][0] : null;
    }
    return selectionAnchorId;
  }, [selectedIds, selectionAnchorId]);
  const activeEntryIdForFolder = visibleEntries.some(
    (entry) => entry.id === activeEntryId,
  )
    ? activeEntryId
    : (visibleEntries[0]?.id ?? null);

  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: visibleEntries.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 32,
    overscan: 10,
  });

  const focusEntry = (index: number) => {
    if (visibleEntries.length === 0) return;
    const nextIndex = Math.max(0, Math.min(index, visibleEntries.length - 1));
    const nextEntry = visibleEntries[nextIndex];
    setActiveEntryId(nextEntry.id);
    virtualizer.scrollToIndex(nextIndex, { align: "auto" });
    window.setTimeout(() => rowRefs.current.get(nextEntry.id)?.focus(), 0);
  };

  // Empty-list message names every active filter so the user can tell which
  // view produced the empty result.
  const hasSearch = searchQuery.trim() !== "";
  const hasFormat = formatFilter.length > 0;
  const hasMark = markFilter !== "all";
  let emptyStateMessage = "(no playable files)";
  if (hasSearch || hasFormat || hasMark) {
    const parts: string[] = [];
    if (hasSearch) parts.push(`“${searchQuery.trim()}”`);
    if (hasFormat) parts.push("the format filter");
    if (hasMark) parts.push("the mark filter");
    emptyStateMessage = `(no files match ${parts.join(" or ")})`;
  }

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
            const selected = visibleEntries.find(
              (entry) => entry.id === primarySelectedId,
            );
            if (selected) requestTrash(selected);
          }}
          disabled={!primarySelectedId || trashStatus === "moving"}
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
        <fieldset className="file-list-format-filter">
          <legend className="visually-hidden">Filter by format</legend>
          {FORMAT_OPTIONS.map((option) => (
            <label key={option.value} className="file-list-format-option">
              <input
                type="checkbox"
                checked={formatFilter.includes(option.value)}
                onChange={() => toggleFormat(option.value)}
              />
              <span>{option.label}</span>
            </label>
          ))}
          <button
            type="button"
            aria-label="Reset format filter"
            onClick={resetFormats}
            disabled={formatFilter.length === 0}
          >
            Reset
          </button>
        </fieldset>
        <fieldset className="file-list-mark-controls">
          <legend className="visually-hidden">Mark selection</legend>
          {SESSION_MARKS.map((mark) => (
            <button
              key={mark}
              type="button"
              aria-label={`Mark ${SESSION_MARK_LABELS[mark]}`}
              className={`file-list-mark-button file-list-mark-button--${mark}`}
              disabled={selectedIds.size === 0}
              onClick={() => applyMarkToSelection(mark)}
            >
              {SESSION_MARK_LABELS[mark]}
            </button>
          ))}
          <button
            type="button"
            aria-label="Clear mark"
            disabled={selectedIds.size === 0}
            onClick={clearMarkOnSelection}
          >
            Clear
          </button>
          <button
            type="button"
            aria-label="Select marked"
            disabled={markedVisibleIds.length === 0}
            onClick={selectMarked}
          >
            Select marked
          </button>
        </fieldset>
        <label className="file-list-mark-filter">
          <span className="visually-hidden">Filter by mark</span>
          <select
            aria-label="Filter by mark"
            value={markFilter}
            onChange={(event) =>
              onMarkFilterChange?.(event.target.value as MarkFilter)
            }
          >
            {MARK_FILTERS.map((filter) => (
              <option key={filter} value={filter}>
                {MARK_FILTER_LABELS[filter]}
              </option>
            ))}
          </select>
        </label>
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
      ) : isLoading && visibleEntries.length === 0 ? (
        <div className="file-list-state file-list-state--loading">
          <p className="file-list-placeholder">Loading&#8230;</p>
        </div>
      ) : visibleEntries.length === 0 ? (
        <div className="file-list-state file-list-state--empty">
          <p className="file-list-placeholder">{emptyStateMessage}</p>
        </div>
      ) : (
        <div
          ref={parentRef}
          className="file-list-viewport"
          role="grid"
          aria-label="Playable files"
          aria-multiselectable="true"
          aria-colcount={10}
          aria-rowcount={visibleEntries.length + 1}
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
            <span role="columnheader">Mark</span>
          </div>
          <div
            className="file-list-inner"
            style={{ height: `${virtualizer.getTotalSize()}px` }}
          >
            {virtualizer.getVirtualItems().map((virtualRow) => {
              const entry = visibleEntries[virtualRow.index];

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
                      if (handleGridKeyDown(event, virtualRow.index)) {
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
                        focusEntry(visibleEntries.length - 1);
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
                    <span role="gridcell" aria-hidden="true"></span>
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
                    selectedIds.has(entry.id) ? " selected" : ""
                  }`}
                  style={{
                    height: `${virtualRow.size}px`,
                    transform: `translateY(${virtualRow.start}px)`,
                  }}
                  onClick={(event) => handlePlayableRowClick(event, entry)}
                  onKeyDown={(event) => {
                    if (handleGridKeyDown(event, virtualRow.index)) {
                      return;
                    }
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
                  aria-selected={selectedIds.has(entry.id)}
                  tabIndex={activeEntryIdForFolder === entry.id ? 0 : -1}
                  aria-label={`${entry.name}${statusLabel ? ` ${statusLabel}` : ""}`}
                  aria-describedby={
                    isPlaybackEntry && playbackError
                      ? "file-list-playback-error"
                      : undefined
                  }
                >
                  <span className="file-list-row-name" role="gridcell">
                    {marks[entry.id] ? (
                      <span
                        className={`file-list-mark-dot file-list-mark-dot--${marks[entry.id]}`}
                        aria-hidden="true"
                      >
                        {marks[entry.id] === "favorite" ? "★" : ""}
                      </span>
                    ) : null}
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
                  <span role="gridcell">
                    {marks[entry.id] ? (
                      <span
                        className={`file-list-mark file-list-mark--${marks[entry.id]}`}
                      >
                        {SESSION_MARK_LABELS[marks[entry.id]]}
                      </span>
                    ) : null}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      )}
      {playbackError &&
      playbackEntryId &&
      visibleEntries.some((entry) => entry.id === playbackEntryId) ? (
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
