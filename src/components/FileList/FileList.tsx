import { useEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  cancelCopyFiles,
  cancelMoveFiles,
  dragOut,
  moveToTrash,
  openWith,
  pickFolder,
  renameFile,
  revealFile,
  startCopyFiles,
  startMoveFiles,
} from "../../api/commandEnvelope";
import {
  onCopyProgress,
  onMoveProgress,
  type CopyProgressPayload,
  type MoveProgressPayload,
} from "../../api/playbackEvents";
import type { BrowserEntry } from "../FolderTree/folderTreeTypes";
import { relativeEntryPath } from "../FolderTree/folderTreeTypes";
import type { PlaybackSelectionStatus } from "../../hooks/usePlaybackSelection";
import { useKeyboardShortcuts } from "../../hooks/useKeyboardShortcuts";
import {
  DEFAULT_SHORTCUTS,
  getShortcutPlatform,
  matchShortcut,
  type ShortcutBindings,
} from "../../shortcuts/keyboardShortcuts";
import { ConfirmDialog } from "../ConfirmDialog/ConfirmDialog";
import { RenameDialog } from "../RenameDialog/RenameDialog";
import {
  CopyDialog,
  type CopyProgress,
  type CopySummary,
} from "../CopyDialog/CopyDialog";
import {
  MoveDialog,
  type MoveProgress,
  type MoveSummary,
} from "../MoveDialog/MoveDialog";
import { ContextMenu } from "../ContextMenu/ContextMenu";

/** One successfully moved file: the source id and the destination id. */
export interface MovedEntry {
  oldId: string;
  newId: string;
}
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
import "../RenameDialog/RenameDialog.css";
import "../MoveDialog/MoveDialog.css";
import "../CopyDialog/CopyDialog.css";

const UNAVAILABLE = "—";
const NATIVE_DRAG_THRESHOLD_PX = 5;
const HORIZONTAL_SCROLL_INITIAL_DELAY_MS = 300;
const HORIZONTAL_SCROLL_REPEAT_MS = 120;

/** True when running on macOS, where WKWebView cannot deliver file URLs to
 * an external drag session and the native drag-out command is used instead.
 */
function isMacOSPlatform(): boolean {
  const buildPlatform = import.meta.env.TAURI_ENV_PLATFORM?.toLowerCase();
  if (buildPlatform) return buildPlatform === "darwin";
  if (typeof navigator === "undefined") return false;
  const platform = navigator.platform?.toLowerCase() ?? "";
  const userAgent = navigator.userAgent.toLowerCase();
  return platform.includes("mac") || userAgent.includes("macintosh");
}

/** Builds a percent-encoded `file://` URI for a drag payload entry id (a
 * filesystem path). Each path segment is encoded so spaces, `#`, `?`, and
 * non-ASCII characters survive the `text/uri-list` payload intact. */
function fileUriForPath(path: string): string {
  const normalized = path.replaceAll("\\", "/");
  const unc = normalized.match(/^\/\/([^/]+)(\/.*)$/);
  if (unc) {
    const [, host, pathname] = unc;
    const encodedPath = pathname
      .split("/")
      .map((segment) => encodeURIComponent(segment))
      .join("/");
    return `file://${encodeURIComponent(host)}${encodedPath}`;
  }
  const encoded = normalized
    .split("/")
    .map((segment, index) =>
      index === 0 && /^[A-Za-z]:$/.test(segment)
        ? segment
        : encodeURIComponent(segment),
    )
    .join("/");
  return /^[A-Za-z]:\//.test(normalized)
    ? `file:///${encoded}`
    : `file://${encoded}`;
}

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
  /** Called after a successful rename so owners can reconcile state. */
  onEntryRenamed?: (oldId: string, newId: string, newName: string) => void;
  /** Called with the source and destination ids of files successfully moved. */
  onEntriesMoved?: (moved: MovedEntry[]) => void;
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
  /** Reports bookmark state for folder search-result rows. */
  isFolderBookmarked?: (path: string) => boolean;
  /** Adds or removes a bookmark for a folder search-result row. */
  onToggleFolderBookmark?: (path: string) => void;
  /** Whether the current folder is shown as a flat recursive file view. */
  recursive?: boolean;
  /** Called when the user toggles the recursive file view. */
  onRecursiveChange?: (recursive: boolean) => void;
  shortcutBindings?: ShortcutBindings;
  focusSearchRevision?: number;
  /** Reduces columns to Name and Duration for the compact player mode. */
  compact?: boolean;
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
  onEntryRenamed,
  onEntriesMoved,
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
  isFolderBookmarked,
  onToggleFolderBookmark,
  recursive = false,
  onRecursiveChange,
  shortcutBindings,
  focusSearchRevision = 0,
  compact = false,
}: FileListProps) {
  const parentRef = useRef<HTMLDivElement>(null);
  const headerRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const initialFocusSearchRevision = useRef(focusSearchRevision);
  const activeShortcutBindings = shortcutBindings ?? DEFAULT_SHORTCUTS;
  const shortcutPlatform = getShortcutPlatform();
  const usesNativeDragOut = isMacOSPlatform();
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
  const [renameTarget, setRenameTarget] = useState<BrowserEntry | null>(null);
  const [renameStatus, setRenameStatus] = useState<
    "idle" | "renaming" | "error"
  >("idle");
  const [renameError, setRenameError] = useState<string | null>(null);
  const [moveOpen, setMoveOpen] = useState(false);
  const [moveTarget, setMoveTarget] = useState<string | null>(null);
  const [moveStatus, setMoveStatus] = useState<
    "idle" | "moving" | "done" | "error"
  >("idle");
  const [moveSessionId, setMoveSessionId] = useState<string | null>(null);
  const [moveProgress, setMoveProgress] = useState<MoveProgress | null>(null);
  const [moveSummary, setMoveSummary] = useState<MoveSummary | null>(null);
  // Refs keep the progress listener and its session id reachable outside the
  // render cycle: the listener is subscribed before the batch starts so a
  // fast batch can never drop its final done event, and payloads that race
  // the start reply are buffered until the session id is known.
  const moveSessionIdRef = useRef<string | null>(null);
  const movePendingRef = useRef<MoveProgressPayload | null>(null);
  const moveUnlistenRef = useRef<(() => void) | null>(null);
  const [moveError, setMoveError] = useState<string | null>(null);
  const onEntriesMovedRef = useRef(onEntriesMoved);
  // Copy flow state (FR-FM-004, FR-FM-005). Copying never modifies the
  // source, so copied rows stay in the current view; the dialog only reports
  // progress and the separate success/failure summary.
  const [copyOpen, setCopyOpen] = useState(false);
  const [copyTarget, setCopyTarget] = useState<string | null>(null);
  const [copyStatus, setCopyStatus] = useState<
    "idle" | "moving" | "done" | "error"
  >("idle");
  const [copyError, setCopyError] = useState<string | null>(null);
  const [copySessionId, setCopySessionId] = useState<string | null>(null);
  const [copyProgress, setCopyProgress] = useState<CopyProgress | null>(null);
  const [copySummary, setCopySummary] = useState<CopySummary | null>(null);
  const copySessionIdRef = useRef<string | null>(null);
  const copyPendingRef = useRef<CopyProgressPayload | null>(null);
  const copyUnlistenRef = useRef<(() => void) | null>(null);
  // External-action state (FR-FM-006, FR-FM-007). Reveal and open-with are
  // single-file, fire-and-forget operations on the primary selected row.
  const [externalBusy, setExternalBusy] = useState(false);
  const [externalError, setExternalError] = useState<string | null>(null);
  const [contextTarget, setContextTarget] = useState<{
    entry: BrowserEntry;
    x: number;
    y: number;
    anchor: HTMLElement;
  } | null>(null);
  const rowRefs = useRef(new Map<string, HTMLDivElement>());
  const nativeDragMouseRef = useRef<{
    startX: number;
    startY: number;
    entryId: string;
  } | null>(null);
  const [canScrollLeft, setCanScrollLeft] = useState(false);
  const [canScrollRight, setCanScrollRight] = useState(false);
  const horizontalScrollTimeoutRef = useRef<number | null>(null);
  const horizontalScrollIntervalRef = useRef<number | null>(null);
  const middleMousePanRef = useRef<{ lastX: number } | null>(null);

  useEffect(() => {
    onEntriesMovedRef.current = onEntriesMoved;
  }, [onEntriesMoved]);

  useEffect(() => {
    if (focusSearchRevision !== initialFocusSearchRevision.current) {
      searchRef.current?.focus();
    }
  }, [focusSearchRevision]);

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

  useEffect(() => {
    const viewport = parentRef.current;
    if (!viewport) return;

    const updateHorizontalScrollControls = () => {
      const maxScrollLeft = viewport.scrollWidth - viewport.clientWidth;
      setCanScrollLeft(viewport.scrollLeft > 0);
      setCanScrollRight(viewport.scrollLeft < maxScrollLeft - 1);
    };

    updateHorizontalScrollControls();
    viewport.addEventListener("scroll", updateHorizontalScrollControls, {
      passive: true,
    });
    window.addEventListener("resize", updateHorizontalScrollControls);
    const resizeObserver =
      typeof ResizeObserver === "undefined"
        ? null
        : new ResizeObserver(updateHorizontalScrollControls);
    resizeObserver?.observe(viewport);
    return () => {
      viewport.removeEventListener("scroll", updateHorizontalScrollControls);
      window.removeEventListener("resize", updateHorizontalScrollControls);
      resizeObserver?.disconnect();
    };
  }, [visibleEntries.length]);

  const scrollHorizontally = (
    direction: "left" | "right",
    behavior: ScrollBehavior = "smooth",
  ) => {
    const viewport = parentRef.current;
    if (!viewport) return;
    const columnOffsets = headerRef.current
      ? Array.from(headerRef.current.children).map(
          (column) => (column as HTMLElement).offsetLeft,
        )
      : [];
    const current = viewport.scrollLeft;
    const nextOffset =
      direction === "right"
        ? columnOffsets.find((offset) => offset > current + 1)
        : [...columnOffsets].reverse().find((offset) => offset < current - 1);

    if (nextOffset !== undefined) {
      viewport.scrollTo({ left: nextOffset, behavior });
      return;
    }

    viewport.scrollBy({
      left:
        direction === "right" ? viewport.clientWidth : -viewport.clientWidth,
      behavior,
    });
  };

  const stopHorizontalScroll = () => {
    if (horizontalScrollTimeoutRef.current !== null) {
      window.clearTimeout(horizontalScrollTimeoutRef.current);
      horizontalScrollTimeoutRef.current = null;
    }
    if (horizontalScrollIntervalRef.current !== null) {
      window.clearInterval(horizontalScrollIntervalRef.current);
      horizontalScrollIntervalRef.current = null;
    }
  };

  const stopMiddleMousePan = () => {
    middleMousePanRef.current = null;
  };

  const handleMiddleMousePanStart = (event: React.MouseEvent) => {
    if (event.button !== 1) return;
    event.preventDefault();
    middleMousePanRef.current = { lastX: event.clientX };
  };

  const handleMiddleMousePanMove = (event: React.MouseEvent) => {
    const pan = middleMousePanRef.current;
    const viewport = parentRef.current;
    if (!pan || !viewport) return;
    event.preventDefault();
    viewport.scrollLeft -= event.clientX - pan.lastX;
    pan.lastX = event.clientX;
  };

  const startHorizontalScroll = (direction: "left" | "right") => {
    stopHorizontalScroll();
    scrollHorizontally(direction);
    horizontalScrollTimeoutRef.current = window.setTimeout(() => {
      horizontalScrollIntervalRef.current = window.setInterval(() => {
        scrollHorizontally(direction, "auto");
      }, HORIZONTAL_SCROLL_REPEAT_MS);
    }, HORIZONTAL_SCROLL_INITIAL_DELAY_MS);
  };

  useEffect(
    () => () => {
      stopHorizontalScroll();
      stopMiddleMousePan();
    },
    [],
  );

  useEffect(() => {
    const stopPan = () => stopMiddleMousePan();
    window.addEventListener("mouseup", stopPan);
    window.addEventListener("blur", stopPan);
    return () => {
      window.removeEventListener("mouseup", stopPan);
      window.removeEventListener("blur", stopPan);
    };
  }, []);

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
    setRenameTarget(null);
    setRenameStatus("idle");
    setRenameError(null);
    setContextTarget(null);
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

  const openContextMenu = (
    event: React.MouseEvent<HTMLElement>,
    entry: BrowserEntry,
  ) => {
    event.preventDefault();
    event.stopPropagation();
    nativeDragMouseRef.current = null;
    if (entry.kind === "playable") {
      if (!selectedIds.has(entry.id)) setSelectedIds(new Set([entry.id]));
      setSelectionAnchorId(entry.id);
    }
    setActiveEntryId(entry.id);
    setContextTarget({
      entry,
      x: event.clientX,
      y: event.clientY,
      anchor: event.currentTarget,
    });
  };

  const openContextMenuFromKeyboard = (
    event: React.KeyboardEvent<HTMLElement>,
    entry: BrowserEntry,
  ): boolean => {
    if (
      event.key !== "ContextMenu" &&
      !(event.shiftKey && event.key === "F10")
    ) {
      return false;
    }
    event.preventDefault();
    const rect = event.currentTarget.getBoundingClientRect();
    if (entry.kind === "playable") {
      if (!selectedIds.has(entry.id)) setSelectedIds(new Set([entry.id]));
      setSelectionAnchorId(entry.id);
    }
    setActiveEntryId(entry.id);
    setContextTarget({
      entry,
      x: rect.left + 12,
      y: rect.top + 12,
      anchor: event.currentTarget,
    });
    return true;
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

  const requestRename = (entry: BrowserEntry) => {
    setRenameError(null);
    setRenameStatus("idle");
    setRenameTarget(entry);
  };

  const confirmRename = async (newName: string) => {
    if (!renameTarget || renameStatus === "renaming") return;
    setRenameStatus("renaming");
    setRenameError(null);
    try {
      const outcome = await renameFile(renameTarget.id, newName);
      const oldId = renameTarget.id;
      const newId = outcome.new_path;
      onEntryRenamed?.(oldId, newId, newName);
      setSelectedIds((current) => {
        if (!current.has(oldId)) return current;
        const next = new Set(current);
        next.delete(oldId);
        next.add(newId);
        return next;
      });
      if (selectionAnchorId === oldId) {
        setSelectionAnchorId(newId);
      }
      if (activeEntryId === oldId) {
        setActiveEntryId(newId);
      }
      setRenameTarget(null);
      setRenameStatus("idle");
    } catch (error: unknown) {
      setRenameStatus("error");
      setRenameError(
        error instanceof Error ? error.message : "Unable to rename file.",
      );
    }
  };

  // ── External actions (FR-FM-006, FR-FM-007) ─────────────────────────
  //
  // Reveal and open-with act on the primary selected playable row through
  // narrow backend commands; React never receives a general launch
  // capability.

  const runExternalAction = async (action: (path: string) => Promise<void>) => {
    if (!primarySelectedEntry || externalBusy) return;
    setExternalBusy(true);
    setExternalError(null);
    try {
      await action(primarySelectedEntry.id);
    } catch (error: unknown) {
      setExternalError(
        error instanceof Error ? error.message : "Unable to complete action.",
      );
    } finally {
      setExternalBusy(false);
    }
  };

  // ── Drag-out (FR-FM-011) ────────────────────────────────────────────
  //
  // Dragging a playable row carries the whole selection when the row is part
  // of it, otherwise just that row. Non-macOS webviews receive the file URLs
  // through `text/uri-list`; macOS hands the paths to the native drag session
  // because WKWebView cannot drag files out by itself.

  const draggedPathsForEntry = (entry: BrowserEntry): string[] =>
    selectedIds.has(entry.id) && selectedPlayableIds.length > 0
      ? selectedPlayableIds
      : [entry.id];

  const startNativeDrag = (entry: BrowserEntry) => {
    const draggedPaths = draggedPathsForEntry(entry);
    if (draggedPaths.length === 0) return;
    setExternalError(null);
    setExternalBusy(true);
    void dragOut(draggedPaths)
      .catch((error: unknown) => {
        setExternalError(
          error instanceof Error ? error.message : "Unable to start drag.",
        );
      })
      .finally(() => {
        setExternalBusy(false);
      });
  };

  const handleHtmlDragStart = (
    event: React.DragEvent<HTMLDivElement>,
    entry: BrowserEntry,
  ) => {
    if (entry.kind !== "playable") return;
    const draggedPaths = draggedPathsForEntry(entry);
    if (draggedPaths.length === 0) return;
    event.dataTransfer.setData(
      "text/uri-list",
      draggedPaths.map((path) => fileUriForPath(path)).join("\n"),
    );
    event.dataTransfer.effectAllowed = "copy";
  };

  const handleNativeMouseDown = (
    event: React.MouseEvent<HTMLDivElement>,
    entry: BrowserEntry,
  ) => {
    if (!usesNativeDragOut || event.button !== 0) return;
    nativeDragMouseRef.current = {
      startX: event.clientX,
      startY: event.clientY,
      entryId: entry.id,
    };
  };

  const handleNativeMouseMove = (
    event: React.MouseEvent<HTMLDivElement>,
    entry: BrowserEntry,
  ) => {
    const pending = nativeDragMouseRef.current;
    if (!usesNativeDragOut || !pending || pending.entryId !== entry.id) {
      return;
    }
    const distance = Math.hypot(
      event.clientX - pending.startX,
      event.clientY - pending.startY,
    );
    if (distance < NATIVE_DRAG_THRESHOLD_PX) return;

    nativeDragMouseRef.current = null;
    event.preventDefault();
    startNativeDrag(entry);
  };

  const clearNativeDragMouse = () => {
    nativeDragMouseRef.current = null;
  };

  const handleDragEnd = () => {
    setExternalBusy(false);
  };

  // ── Move flow (FR-FM-004, FR-FM-005) ────────────────────────────────
  //
  // The user picks a target folder, then the batch runs on a backend worker.
  // Per-file progress arrives through `browser:move-progress`; when the batch
  // finishes, successful and failed targets are reported separately and the
  // successful source ids are handed to the owner so the view can drop them.

  const requestMove = () => {
    moveSessionIdRef.current = null;
    movePendingRef.current = null;
    moveUnlistenRef.current?.();
    moveUnlistenRef.current = null;
    setMoveError(null);
    setMoveStatus("idle");
    setMoveTarget(null);
    setMoveProgress(null);
    setMoveSummary(null);
    setMoveSessionId(null);
    setMoveOpen(true);
  };

  const pickMoveTarget = async () => {
    if (moveStatus === "moving") return;
    try {
      const picked = await pickFolder();
      if (picked) {
        setMoveTarget(picked);
        setMoveError(null);
      }
    } catch (error: unknown) {
      setMoveError(
        error instanceof Error ? error.message : "Unable to choose a folder.",
      );
    }
  };

  // Applies one move-progress payload. Events can arrive before the start
  // reply resolves (the worker emits as soon as the batch starts), so a
  // pending payload is buffered and replayed once the session id is known.
  const handleMoveProgress = (payload: MoveProgressPayload) => {
    if (!moveSessionIdRef.current) {
      movePendingRef.current = payload;
      return;
    }
    if (payload.session_id !== moveSessionIdRef.current) return;
    movePendingRef.current = null;
    setMoveProgress({ completed: payload.completed, total: payload.total });
    if (payload.done) {
      const okCount = payload.results.filter((item) => item.ok).length;
      const failed = payload.results.filter((item) => !item.ok);
      const moved: MovedEntry[] = payload.results
        .filter((item) => item.ok)
        .map((item) => ({
          oldId: item.path,
          newId: item.new_path ?? item.path,
        }));
      setMoveSummary({ okCount, failed });
      setMoveStatus("done");
      moveSessionIdRef.current = null;
      setMoveSessionId(null);
      moveUnlistenRef.current?.();
      moveUnlistenRef.current = null;
      // Drop moved rows from the local selection so stale ids can never
      // feed later batch actions.
      setSelectedIds((current) => {
        if (moved.length === 0) return current;
        const next = new Set(current);
        for (const entry of moved) next.delete(entry.oldId);
        return next;
      });
      onEntriesMovedRef.current?.(moved);
    }
  };

  const confirmMove = async () => {
    if (!moveTarget || moveStatus === "moving") return;
    const selected = visibleEntries
      .filter((entry) => entry.kind === "playable" && selectedIds.has(entry.id))
      .map((entry) => entry.id);
    if (selected.length === 0) return;
    setMoveError(null);
    setMoveSummary(null);
    setMoveProgress({ completed: 0, total: selected.length });
    setMoveStatus("moving");
    try {
      // Subscribe before starting the batch: the worker can emit the final
      // done event within the start_move IPC round trip, so the listener must
      // already exist before the backend begins.
      movePendingRef.current = null;
      const unlisten = await onMoveProgress(handleMoveProgress);
      moveUnlistenRef.current = unlisten;
      const sessionId = await startMoveFiles(selected, moveTarget);
      moveSessionIdRef.current = sessionId;
      setMoveSessionId(sessionId);
      // Replay an event that raced the start reply. The cast defeats
      // control-flow narrowing from the null assignment above.
      const pending = movePendingRef.current as MoveProgressPayload | null;
      if (pending && pending.session_id === sessionId) {
        handleMoveProgress(pending);
      }
    } catch (error: unknown) {
      moveUnlistenRef.current?.();
      moveUnlistenRef.current = null;
      moveSessionIdRef.current = null;
      movePendingRef.current = null;
      setMoveStatus("error");
      setMoveProgress(null);
      setMoveError(
        error instanceof Error ? error.message : "Unable to start move.",
      );
    }
  };

  // Releases the move-progress listener if the component unmounts mid-batch.
  useEffect(
    () => () => {
      moveUnlistenRef.current?.();
    },
    [],
  );

  const cancelMove = () => {
    if (moveSessionId) {
      // Ask the backend to stop; the remaining files report Cancelled and
      // the batch still emits a done event so the summary stays accurate.
      void cancelMoveFiles(moveSessionId).catch(() => {
        // Best-effort cancel.
      });
      return;
    }
    moveUnlistenRef.current?.();
    moveUnlistenRef.current = null;
    moveSessionIdRef.current = null;
    movePendingRef.current = null;
    if (moveStatus !== "moving") {
      setMoveOpen(false);
      setMoveStatus("idle");
    }
  };

  // ── Copy flow (FR-FM-004, FR-FM-005) ────────────────────────────────
  //
  // The user picks a target folder, then the batch runs on a backend worker.
  // Per-file progress arrives through `browser:copy-progress`; when the batch
  // finishes, successful and failed targets are reported separately. Copying
  // never modifies the source, so copied rows stay in the current view.

  const requestCopy = () => {
    copySessionIdRef.current = null;
    copyPendingRef.current = null;
    copyUnlistenRef.current?.();
    copyUnlistenRef.current = null;
    setCopyError(null);
    setCopyStatus("idle");
    setCopyTarget(null);
    setCopyProgress(null);
    setCopySummary(null);
    setCopySessionId(null);
    setCopyOpen(true);
  };

  const pickCopyTarget = async () => {
    if (copyStatus === "moving") return;
    try {
      const picked = await pickFolder();
      if (picked) {
        setCopyTarget(picked);
        setCopyError(null);
      }
    } catch (error: unknown) {
      setCopyError(
        error instanceof Error ? error.message : "Unable to choose a folder.",
      );
    }
  };

  // Applies one copy-progress payload. Events can arrive before the start
  // reply resolves (the worker emits as soon as the batch starts), so a
  // pending payload is buffered and replayed once the session id is known.
  const handleCopyProgress = (payload: CopyProgressPayload) => {
    if (!copySessionIdRef.current) {
      copyPendingRef.current = payload;
      return;
    }
    if (payload.session_id !== copySessionIdRef.current) return;
    copyPendingRef.current = null;
    setCopyProgress({ completed: payload.completed, total: payload.total });
    if (payload.done) {
      const okCount = payload.results.filter((item) => item.ok).length;
      const failed = payload.results.filter((item) => !item.ok);
      setCopySummary({ okCount, failed });
      setCopyStatus("done");
      copySessionIdRef.current = null;
      setCopySessionId(null);
      copyUnlistenRef.current?.();
      copyUnlistenRef.current = null;
    }
  };

  const confirmCopy = async () => {
    if (!copyTarget || copyStatus === "moving") return;
    const selected = visibleEntries
      .filter((entry) => entry.kind === "playable" && selectedIds.has(entry.id))
      .map((entry) => entry.id);
    if (selected.length === 0) return;
    setCopyError(null);
    setCopySummary(null);
    setCopyProgress({ completed: 0, total: selected.length });
    setCopyStatus("moving");
    try {
      // Subscribe before starting the batch: the worker can emit the final
      // done event within the start_copy IPC round trip, so the listener must
      // already exist before the backend begins.
      copyPendingRef.current = null;
      const unlisten = await onCopyProgress(handleCopyProgress);
      copyUnlistenRef.current = unlisten;
      const sessionId = await startCopyFiles(selected, copyTarget);
      copySessionIdRef.current = sessionId;
      setCopySessionId(sessionId);
      // Replay an event that raced the start reply. The cast defeats
      // control-flow narrowing from the null assignment above.
      const pending = copyPendingRef.current as CopyProgressPayload | null;
      if (pending && pending.session_id === sessionId) {
        handleCopyProgress(pending);
      }
    } catch (error: unknown) {
      copyUnlistenRef.current?.();
      copyUnlistenRef.current = null;
      copySessionIdRef.current = null;
      copyPendingRef.current = null;
      setCopyStatus("error");
      setCopyProgress(null);
      setCopyError(
        error instanceof Error ? error.message : "Unable to start copy.",
      );
    }
  };

  // Releases the copy-progress listener if the component unmounts mid-batch.
  useEffect(
    () => () => {
      copyUnlistenRef.current?.();
    },
    [],
  );

  const cancelCopy = () => {
    if (copySessionId) {
      // Ask the backend to stop; the remaining files report Cancelled and
      // the batch still emits a done event so the summary stays accurate.
      void cancelCopyFiles(copySessionId).catch(() => {
        // Best-effort cancel.
      });
      return;
    }
    copyUnlistenRef.current?.();
    copyUnlistenRef.current = null;
    copySessionIdRef.current = null;
    copyPendingRef.current = null;
    if (copyStatus !== "moving") {
      setCopyOpen(false);
      setCopyStatus("idle");
    }
  };

  useKeyboardShortcuts(
    {
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
      onPlaySelection: () => {
        const selected = visibleEntries.find(
          (entry) =>
            entry.id === primarySelectedId && entry.kind === "playable",
        );
        if (selected) onFileSelect?.(selected);
      },
    },
    shortcutBindings,
  );

  // The primary selection is the anchor (last clicked or focused row). It
  // drives single-file actions such as Move to Trash and Rename; batch
  // operations (Move) act on the whole playable selection (FR-FM-005).
  const primarySelectedId = useMemo(() => {
    if (!selectionAnchorId || !selectedIds.has(selectionAnchorId)) {
      return selectedIds.size > 0 ? [...selectedIds][0] : null;
    }
    return selectionAnchorId;
  }, [selectedIds, selectionAnchorId]);
  // Rename targets files only (FR-FM-004); folder rows stay navigable and are
  // never exposed to the rename action.
  const primarySelectedEntry = useMemo(
    () => visibleEntries.find((entry) => entry.id === primarySelectedId),
    [visibleEntries, primarySelectedId],
  );
  const canRenamePrimary = primarySelectedEntry?.kind === "playable";
  // External actions (Reveal, Open With…) also target a single playable file,
  // so they share the same eligibility without depending on rename semantics.
  const hasPlayablePrimary = primarySelectedEntry?.kind === "playable";
  // A stale external-action error must not outlive its target row: switching
  // the primary selection clears the previous failure message.
  useEffect(() => {
    setExternalError(null);
  }, [primarySelectedId]);
  // Move targets every selected playable file, so the action needs at least
  // one selected playable row and a batch that is not already running.
  const selectedPlayableIds = useMemo(
    () =>
      visibleEntries
        .filter(
          (entry) => entry.kind === "playable" && selectedIds.has(entry.id),
        )
        .map((entry) => entry.id),
    [visibleEntries, selectedIds],
  );
  const canMoveSelection = selectedPlayableIds.length > 0;
  const canCopySelection = selectedPlayableIds.length > 0;
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
      {!compact && (
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
          <details className="file-list-action-menu">
            <summary>File actions</summary>
            <div className="file-list-action-menu-content">
              <button
                type="button"
                className="file-list-rename-button"
                onClick={() => {
                  if (primarySelectedEntry) requestRename(primarySelectedEntry);
                }}
                disabled={!canRenamePrimary || renameStatus === "renaming"}
              >
                Rename
              </button>
              <button
                type="button"
                className="file-list-move-button"
                onClick={requestMove}
                disabled={!canMoveSelection || moveStatus === "moving"}
              >
                Move…
              </button>
              <button
                type="button"
                className="file-list-copy-button"
                onClick={requestCopy}
                disabled={!canCopySelection || copyStatus === "moving"}
              >
                Copy…
              </button>
              <button
                type="button"
                className="file-list-reveal-button"
                onClick={() => {
                  void runExternalAction(revealFile);
                }}
                disabled={!hasPlayablePrimary || externalBusy}
              >
                Reveal
              </button>
              <button
                type="button"
                className="file-list-open-with-button"
                onClick={() => {
                  void runExternalAction(openWith);
                }}
                disabled={!hasPlayablePrimary || externalBusy}
              >
                Open With…
              </button>
            </div>
          </details>
          {externalError && (
            <span className="file-list-external-error" role="alert">
              {externalError}
            </span>
          )}
          <button
            type="button"
            className="file-list-recursive-toggle"
            aria-pressed={recursive}
            onClick={() => onRecursiveChange?.(!recursive)}
          >
            Recursive view
          </button>
          <input
            ref={searchRef}
            type="search"
            className="file-list-search"
            aria-label="Search files"
            placeholder="Search files"
            value={searchQuery}
            onChange={(event) => onSearchQueryChange?.(event.target.value)}
          />
          <details className="file-list-filter-menu">
            <summary>Filters</summary>
            <div className="file-list-filter-menu-content">
              <fieldset className="file-list-format-filter">
                <legend>Filter by format</legend>
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
            </div>
          </details>
          <details className="file-list-mark-menu">
            <summary>Marks</summary>
            <div className="file-list-mark-menu-content">
              <fieldset className="file-list-mark-controls">
                <legend>Mark selection</legend>
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
                <span>Filter by mark</span>
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
            </div>
          </details>
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
      )}

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
        <>
          {!compact && (
            <div
              className="file-list-horizontal-controls"
              aria-label="Horizontal column navigation"
            >
              <button
                type="button"
                aria-label="Show columns to the left"
                onMouseDown={(event) => {
                  if (event.button === 0) startHorizontalScroll("left");
                }}
                onMouseUp={stopHorizontalScroll}
                onMouseLeave={stopHorizontalScroll}
                onKeyDown={(event) => {
                  if (event.key === "ArrowLeft" && !event.repeat) {
                    event.preventDefault();
                    startHorizontalScroll("left");
                  }
                }}
                onKeyUp={stopHorizontalScroll}
                onBlur={stopHorizontalScroll}
                disabled={!canScrollLeft}
              >
                ←
              </button>
              <button
                type="button"
                aria-label="Show columns to the right"
                onMouseDown={(event) => {
                  if (event.button === 0) startHorizontalScroll("right");
                }}
                onMouseUp={stopHorizontalScroll}
                onMouseLeave={stopHorizontalScroll}
                onKeyDown={(event) => {
                  if (event.key === "ArrowRight" && !event.repeat) {
                    event.preventDefault();
                    startHorizontalScroll("right");
                  }
                }}
                onKeyUp={stopHorizontalScroll}
                onBlur={stopHorizontalScroll}
                disabled={!canScrollRight}
              >
                →
              </button>
            </div>
          )}
          <div
            ref={parentRef}
            className="file-list-viewport"
            onMouseDown={handleMiddleMousePanStart}
            onMouseMove={handleMiddleMousePanMove}
            onMouseUp={stopMiddleMousePan}
            onMouseLeave={stopMiddleMousePan}
            onContextMenu={(event) => {
              if (middleMousePanRef.current) event.preventDefault();
            }}
            role="grid"
            aria-label="Playable files"
            aria-multiselectable="true"
            aria-colcount={compact ? 2 : 10}
            aria-rowcount={visibleEntries.length + 1}
          >
            <div ref={headerRef} className="file-list-header" role="row">
              {SORTABLE_HEADERS.filter(
                (header) =>
                  !compact ||
                  header.field === "name" ||
                  header.field === "duration",
              ).map((header) => {
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
              {!compact && (
                <>
                  <span role="columnheader">Channels</span>
                  <span role="columnheader">Sample rate</span>
                  <span role="columnheader">Bit depth</span>
                  <span role="columnheader">Codec</span>
                  <span role="columnheader">Status</span>
                  <span role="columnheader">Mark</span>
                </>
              )}
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
                      onContextMenu={(event) => openContextMenu(event, entry)}
                      onKeyDown={(event) => {
                        if (openContextMenuFromKeyboard(event, entry)) return;
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
                      {compact ? (
                        <span role="gridcell" aria-hidden="true"></span>
                      ) : (
                        <>
                          <span role="gridcell" aria-hidden="true"></span>
                          <span role="gridcell" aria-hidden="true"></span>
                          <span role="gridcell" aria-hidden="true"></span>
                          <span role="gridcell" aria-hidden="true"></span>
                          <span role="gridcell" aria-hidden="true"></span>
                          <span role="gridcell" aria-hidden="true"></span>
                          <span role="gridcell" aria-hidden="true"></span>
                          <span role="gridcell">Folder</span>
                          <span role="gridcell" aria-hidden="true"></span>
                        </>
                      )}
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
                    draggable={entry.kind === "playable" && !usesNativeDragOut}
                    onDragStart={
                      usesNativeDragOut
                        ? undefined
                        : (event) => handleHtmlDragStart(event, entry)
                    }
                    onDragEnd={handleDragEnd}
                    onMouseDown={(event) => handleNativeMouseDown(event, entry)}
                    onMouseMove={(event) => handleNativeMouseMove(event, entry)}
                    onMouseUp={clearNativeDragMouse}
                    onMouseLeave={clearNativeDragMouse}
                    onClick={(event) => handlePlayableRowClick(event, entry)}
                    onContextMenu={(event) => openContextMenu(event, entry)}
                    onKeyDown={(event) => {
                      if (openContextMenuFromKeyboard(event, entry)) return;
                      if (handleGridKeyDown(event, virtualRow.index)) {
                        return;
                      }
                      if (
                        activeShortcutBindings.move_to_trash &&
                        matchShortcut(
                          event.nativeEvent,
                          activeShortcutBindings.move_to_trash,
                          shortcutPlatform,
                        )
                      ) {
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
                      if (
                        activeShortcutBindings.play_selection &&
                        matchShortcut(
                          event.nativeEvent,
                          activeShortcutBindings.play_selection,
                          shortcutPlatform,
                        )
                      ) {
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
                      {recursive
                        ? relativeEntryPath(entry.id, selectedPath ?? "")
                        : entry.name}
                    </span>
                    <span role="gridcell">
                      {metadataValue(
                        formatDuration(metadata?.duration_ms ?? null),
                        metadataLoading,
                      )}
                    </span>
                    {!compact && (
                      <>
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
                      </>
                    )}
                  </div>
                );
              })}
            </div>
          </div>
        </>
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
      <RenameDialog
        key={renameTarget ? "open" : "closed"}
        open={renameTarget !== null}
        title="Rename File"
        initialName={renameTarget?.name ?? ""}
        busy={renameStatus === "renaming"}
        error={renameError}
        onConfirm={(newName) => {
          void confirmRename(newName);
        }}
        onCancel={() => {
          if (renameStatus !== "renaming") {
            setRenameTarget(null);
            setRenameError(null);
          }
        }}
      />
      <MoveDialog
        open={moveOpen}
        title="Move Files"
        fileNameCount={selectedPlayableIds.length}
        targetDir={moveTarget}
        busy={moveStatus === "moving"}
        error={moveError}
        progress={moveProgress}
        summary={moveStatus === "done" ? moveSummary : null}
        onPickTarget={() => {
          void pickMoveTarget();
        }}
        onConfirm={() => {
          void confirmMove();
        }}
        onCancel={cancelMove}
      />
      <CopyDialog
        open={copyOpen}
        title="Copy Files"
        fileNameCount={selectedPlayableIds.length}
        targetDir={copyTarget}
        busy={copyStatus === "moving"}
        error={copyError}
        progress={copyProgress}
        summary={copyStatus === "done" ? copySummary : null}
        onPickTarget={() => {
          void pickCopyTarget();
        }}
        onConfirm={() => {
          void confirmCopy();
        }}
        onCancel={cancelCopy}
      />
      {contextTarget?.entry.kind === "playable" ? (
        <ContextMenu
          label={`File actions for ${contextTarget.entry.name}`}
          x={contextTarget.x}
          y={contextTarget.y}
          returnFocus={contextTarget.anchor}
          onClose={() => setContextTarget(null)}
        >
          <button
            type="button"
            role="menuitem"
            onClick={() => onFileSelect?.(contextTarget.entry)}
          >
            Play
          </button>
          {SESSION_MARKS.map((mark) => (
            <button
              key={mark}
              type="button"
              role="menuitem"
              className={`context-menu-mark--${mark}`}
              onClick={() => applyMarkToSelection(mark)}
            >
              Mark {SESSION_MARK_LABELS[mark]}
            </button>
          ))}
          <button type="button" role="menuitem" onClick={clearMarkOnSelection}>
            Clear mark
          </button>
          <div className="context-menu-separator" role="separator" />
          <button
            type="button"
            role="menuitem"
            onClick={() => requestRename(contextTarget.entry)}
            disabled={renameStatus === "renaming"}
          >
            Rename
          </button>
          <button
            type="button"
            role="menuitem"
            onClick={requestMove}
            disabled={!canMoveSelection || moveStatus === "moving"}
          >
            Move…
          </button>
          <button
            type="button"
            role="menuitem"
            onClick={requestCopy}
            disabled={!canCopySelection || copyStatus === "moving"}
          >
            Copy…
          </button>
          <button
            type="button"
            role="menuitem"
            onClick={() => void runExternalAction(revealFile)}
            disabled={externalBusy}
          >
            Reveal
          </button>
          <button
            type="button"
            role="menuitem"
            onClick={() => void runExternalAction(openWith)}
            disabled={externalBusy}
          >
            Open With…
          </button>
          <div className="context-menu-separator" role="separator" />
          <button
            type="button"
            role="menuitem"
            onClick={() => requestTrash(contextTarget.entry)}
            disabled={trashStatus === "moving"}
          >
            Move to Trash
          </button>
        </ContextMenu>
      ) : contextTarget ? (
        <ContextMenu
          label={`Folder actions for ${contextTarget.entry.name}`}
          x={contextTarget.x}
          y={contextTarget.y}
          returnFocus={contextTarget.anchor}
          onClose={() => setContextTarget(null)}
        >
          <button
            type="button"
            role="menuitem"
            onClick={() => onSelectFolder?.(contextTarget.entry.id)}
          >
            Open
          </button>
          <button
            type="button"
            role="menuitem"
            onClick={() => onToggleFolderBookmark?.(contextTarget.entry.id)}
            disabled={!onToggleFolderBookmark}
          >
            {isFolderBookmarked?.(contextTarget.entry.id)
              ? "Remove folder bookmark"
              : "Bookmark folder"}
          </button>
        </ContextMenu>
      ) : null}
    </div>
  );
}
