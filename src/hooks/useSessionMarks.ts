import { useCallback, useState } from "react";
import {
  markFiles,
  unmarkFiles,
  type SessionMark,
  type SessionMarks,
} from "../components/FileList/sessionMarks";

export interface UseSessionMarksResult {
  /** Marks keyed by stable backend entry id, for the current session only. */
  marks: SessionMarks;
  /** Applies `mark` to every id, replacing any previous mark. */
  setMark: (ids: readonly string[], mark: SessionMark) => void;
  /** Removes the mark from every id. */
  unmark: (ids: readonly string[]) => void;
  /** Clears every session mark. */
  clear: () => void;
}

/**
 * Holds the session-only file marks (FR-LS-006). The state lives purely in
 * frontend memory: marking a file never calls the backend and never creates a
 * library item or manager database record (FR-LS-007). Marks survive folder
 * navigation because they are keyed by the stable backend entry id, and they
 * disappear when the app session ends.
 */
export function useSessionMarks(): UseSessionMarksResult {
  const [marks, setMarks] = useState<SessionMarks>({});

  const setMark = useCallback((ids: readonly string[], mark: SessionMark) => {
    setMarks((current) => markFiles(current, ids, mark));
  }, []);

  const unmark = useCallback((ids: readonly string[]) => {
    setMarks((current) => unmarkFiles(current, ids));
  }, []);

  const clear = useCallback(() => {
    setMarks({});
  }, []);

  return { marks, setMark, unmark, clear };
}
