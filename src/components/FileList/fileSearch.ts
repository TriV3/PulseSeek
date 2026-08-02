import type { BrowserEntry } from "../FolderTree/folderTreeTypes";

/**
 * Filters playable entries by a text query against the file name and the full
 * path. The query is trimmed and compared case-insensitively using Unicode
 * case folding so accented and non-latin names match naturally.
 *
 * This is a pure in-memory filter: it never touches the filesystem, so
 * searching never re-enumerates the folder (FR-LS-004).
 */
export function filterFileEntries(
  entries: BrowserEntry[],
  query: string,
): BrowserEntry[] {
  const needle = query.trim().toLowerCase();
  if (needle === "") return [...entries];

  return entries.filter((entry) => {
    const name = entry.name.toLowerCase();
    const path = entry.id.toLowerCase();
    return name.includes(needle) || path.includes(needle);
  });
}
