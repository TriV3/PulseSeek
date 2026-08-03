import type { BrowserEntry } from "../FolderTree/folderTreeTypes";

/** Session-only marks applied to a single file (FR-LS-006). */
export type SessionMark = "keep" | "maybe" | "reject" | "favorite";

/** Mark filters for the file list (FR-LS-008). */
export type MarkFilter = "all" | "marked" | SessionMark;

/** The four marks a file can carry, in presentation order. */
export const SESSION_MARKS: readonly SessionMark[] = [
  "keep",
  "maybe",
  "reject",
  "favorite",
];

/** Mark filter options, in presentation order. */
export const MARK_FILTERS: readonly MarkFilter[] = [
  "all",
  "marked",
  ...SESSION_MARKS,
];

/** Human-readable labels for each mark. */
export const SESSION_MARK_LABELS: Record<SessionMark, string> = {
  keep: "Keep",
  maybe: "Maybe",
  reject: "Reject",
  favorite: "Favorite",
};

/** Human-readable labels for each mark filter. */
export const MARK_FILTER_LABELS: Record<MarkFilter, string> = {
  all: "All marks",
  marked: "Marked",
  keep: "Keep",
  maybe: "Maybe",
  reject: "Reject",
  favorite: "Favorite",
};

/**
 * Session marks keyed by the stable backend entry id. Marks live only in
 * frontend memory for the current session and never create a library item or
 * manager database record (FR-LS-007).
 */
export type SessionMarks = Readonly<Record<string, SessionMark>>;

/**
 * Sets `mark` on every id, replacing any previous mark. Marks are mutually
 * exclusive, so applying a new mark removes the old one. Returns a new map
 * without mutating the input.
 */
export function markFiles(
  marks: SessionMarks,
  ids: readonly string[],
  mark: SessionMark,
): SessionMarks {
  const next: Record<string, SessionMark> = { ...marks };
  for (const id of ids) {
    next[id] = mark;
  }
  return next;
}

/**
 * Removes the mark from every id. Returns a new map without mutating the
 * input.
 */
export function unmarkFiles(
  marks: SessionMarks,
  ids: readonly string[],
): SessionMarks {
  const next: Record<string, SessionMark> = { ...marks };
  for (const id of ids) {
    delete next[id];
  }
  return next;
}

/** True when an entry's mark satisfies the active filter. */
export function matchesMarkFilter(
  mark: SessionMark | undefined,
  filter: MarkFilter,
): boolean {
  if (filter === "all") return true;
  if (filter === "marked") return mark !== undefined;
  return mark === filter;
}

/**
 * Filters entries by session mark. This is a pure in-memory filter over
 * entries already streamed by Rust: it never touches the filesystem or a
 * database. Folder rows always stay visible so the user can navigate out of a
 * filtered list (matching the format filter behavior).
 */
export function filterByMark(
  entries: readonly BrowserEntry[],
  marks: SessionMarks,
  filter: MarkFilter,
): BrowserEntry[] {
  if (filter === "all") return [...entries];
  return entries.filter((entry) => {
    if (entry.kind === "folder") return true;
    return matchesMarkFilter(marks[entry.id], filter);
  });
}

/**
 * Returns the ids of every playable entry carrying any mark. Used to
 * batch-select marked files (FR-LS-008).
 */
export function selectMarkedEntryIds(
  entries: readonly BrowserEntry[],
  marks: SessionMarks,
): string[] {
  return entries
    .filter(
      (entry) => entry.kind === "playable" && marks[entry.id] !== undefined,
    )
    .map((entry) => entry.id);
}
