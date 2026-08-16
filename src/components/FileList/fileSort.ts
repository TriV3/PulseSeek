import type { BrowserEntry } from "../FolderTree/folderTreeTypes";

/** Field used to order the visible playable file list. */
export type FileSortField =
  "name" | "duration" | "size" | "type" | "date" | "path";

export type FileSortDirection = "asc" | "desc";

export interface FileSort {
  field: FileSortField;
  direction: FileSortDirection;
}

/** Default ordering: name ascending, matching the canonical reducer order. */
export const DEFAULT_FILE_SORT: FileSort = {
  field: "name",
  direction: "asc",
};

/** Returns the lowercase file extension from a file name. */
export function getFileExtension(name: string): string {
  const lastDot = name.lastIndexOf(".");
  if (lastDot < 0) return "";
  return name.slice(lastDot + 1).toLowerCase();
}

/** Compares values that may be null; entries with missing values sort last. */
function compareOptionalNumber(
  left: number | null | undefined,
  right: number | null | undefined,
): number {
  if (left === right) return 0;
  if (left == null) return 1;
  if (right == null) return -1;
  return left - right;
}

/** Compares strings with locale rules and base (case/Unicode-insensitive) sensitivity. */
function compareLocale(left: string, right: string): number {
  return left.localeCompare(right, undefined, { sensitivity: "base" });
}

/**
 * Sorts playable entries by a user-selected field and direction.
 *
 * The sort is deterministic:
 * - Entries with missing metadata sort last in both directions.
 * - Equal keys fall back to the full path (`id`), so results never depend on
 *   enumeration order or on the relative order of the input array.
 * - String fields compare with locale rules; numbers compare numerically.
 */
export function sortFileEntries(
  entries: BrowserEntry[],
  sort: FileSort,
): BrowserEntry[] {
  const multiplier = sort.direction === "asc" ? 1 : -1;

  return [...entries].sort((left, right) => {
    const byField = compareByField(left, right, sort.field);
    if (byField !== 0) {
      // Entries with missing metadata stay last in both directions; only
      // flip the comparison when both sides have values.
      if (
        isMissingField(left, sort.field) ||
        isMissingField(right, sort.field)
      ) {
        return byField;
      }
      return multiplier * byField;
    }
    const byId = left.id.localeCompare(right.id, undefined, {
      sensitivity: "base",
    });
    return multiplier * byId;
  });
}

function compareByField(
  left: BrowserEntry,
  right: BrowserEntry,
  field: FileSortField,
): number {
  switch (field) {
    case "name":
      return compareLocale(left.name, right.name);
    case "path":
      return compareLocale(left.id, right.id);
    case "type":
      return compareLocale(
        getFileExtension(left.name),
        getFileExtension(right.name),
      );
    case "duration":
      return compareOptionalNumber(
        left.metadata?.duration_ms,
        right.metadata?.duration_ms,
      );
    case "size":
      return compareOptionalNumber(
        left.metadata?.size_bytes,
        right.metadata?.size_bytes,
      );
    case "date":
      return compareOptionalNumber(
        left.metadata?.modified_at_ms,
        right.metadata?.modified_at_ms,
      );
  }
}

function isMissingField(entry: BrowserEntry, field: FileSortField): boolean {
  switch (field) {
    case "duration":
      return entry.metadata?.duration_ms == null;
    case "size":
      return entry.metadata?.size_bytes == null;
    case "date":
      return entry.metadata?.modified_at_ms == null;
    case "name":
    case "path":
    case "type":
      return false;
  }
}
