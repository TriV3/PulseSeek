import { useEffect, useRef } from "react";
import type { BrowserEntry } from "../components/FolderTree/folderTreeTypes";
import type { FolderTreeState } from "../components/FolderTree/folderTreeTypes";
import {
  transferMarksByMetadata,
  type SessionMarks,
} from "../components/FileList/sessionMarks";

/**
 * Keeps session marks attached to their file across external renames
 * (FR-FM-010, FR-LS-007).
 *
 * Marks are keyed by the stable backend entry id, and an external rename
 * (for example from Finder) changes that id. The file watcher re-enumerates
 * the folder, so the renamed row appears under a new id with no mark. This
 * hook watches completed enumerations and, when the entry set for the
 * selected folder changed, transfers orphaned marks to the matching renamed
 * entry by metadata (see `transferMarksByMetadata`).
 *
 * The last-complete snapshot is only refreshed on `ready` states, so
 * mid-streaming resets never trigger a transfer.
 */
export function useMarkReconciliation(
  selectedPath: string | null,
  status: FolderTreeState["status"],
  entriesByPath: Record<string, BrowserEntry[]>,
  marks: SessionMarks,
  replaceMarks: (next: SessionMarks) => void,
): void {
  const lastCompleteRef = useRef<Record<string, BrowserEntry[]>>({});

  useEffect(() => {
    if (status !== "ready" || !selectedPath) return;
    const next = entriesByPath[selectedPath] ?? [];
    const previous = lastCompleteRef.current[selectedPath];
    if (previous && previous !== next) {
      const transferred = transferMarksByMetadata(marks, previous, next);
      if (transferred !== marks) replaceMarks(transferred);
    }
    lastCompleteRef.current[selectedPath] = next;
  }, [status, selectedPath, entriesByPath, marks, replaceMarks]);
}
