import { useCallback, useEffect, useRef, useState } from "react";
import {
  clearRecentFolders,
  listRecentFolders,
  recordRecentFolder,
  type RecentFolderData,
} from "../api/commandEnvelope";

/** Backend limit for the recent-folder history (mirrors Rust). */
export const RECENT_FOLDERS_LIMIT = 10;

export interface UseRecentFoldersReturn {
  folders: RecentFolderData[];
  isLoading: boolean;
  error: string | null;
  /** Records a folder as most recently opened. Best-effort and optimistic:
   * the local list updates immediately and backend failures are ignored
   * because the history is non-critical technical cache data.
   */
  record: (path: string, name?: string) => void;
  /** Clears the history. Resolves to `true` when the backend confirmed. */
  clear: () => Promise<boolean>;
  /** Reloads the history from the backend. */
  refresh: () => Promise<void>;
}

export function useRecentFolders(): UseRecentFoldersReturn {
  const [folders, setFolders] = useState<RecentFolderData[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    let active = true;
    void listRecentFolders()
      .then((loaded) => {
        if (!active) return;
        setFolders(loaded);
        setError(null);
      })
      .catch((cause: unknown) => {
        if (!active) return;
        setError(
          cause instanceof Error
            ? cause.message
            : "Recent folders are unavailable.",
        );
      })
      .finally(() => {
        if (active) setIsLoading(false);
      });
    return () => {
      mounted.current = false;
      active = false;
    };
  }, []);

  const refresh = useCallback(async () => {
    try {
      const loaded = await listRecentFolders();
      if (!mounted.current) return;
      setFolders(loaded);
      setError(null);
    } catch (cause: unknown) {
      if (!mounted.current) return;
      setError(
        cause instanceof Error
          ? cause.message
          : "Recent folders are unavailable.",
      );
    }
  }, []);

  const record = useCallback((path: string, name?: string) => {
    // Virtual browser roots are rejected by the backend; keep the optimistic
    // list consistent so they never appear as recent folders.
    if (path === "computer://" || path.startsWith("computer://")) return;
    const displayName =
      name ??
      path.replace(/\/+$/, "").split("/").filter(Boolean).at(-1) ??
      path;
    setFolders((current) => {
      const next = [
        {
          path,
          name: displayName,
          last_opened_ms: Date.now(),
        },
        ...current.filter((folder) => folder.path !== path),
      ].slice(0, RECENT_FOLDERS_LIMIT);
      return next;
    });
    setError(null);
    void recordRecentFolder(path).catch(() => {
      // Recording is non-critical: a failed backend write never blocks
      // browsing and never surfaces an error banner.
    });
  }, []);

  const clear = useCallback(async () => {
    try {
      await clearRecentFolders();
      if (mounted.current) {
        setFolders([]);
        setError(null);
      }
      return true;
    } catch (cause: unknown) {
      if (mounted.current) {
        setError(
          cause instanceof Error
            ? cause.message
            : "Could not clear recent folders.",
        );
      }
      return false;
    }
  }, []);

  return { folders, isLoading, error, record, clear, refresh };
}
