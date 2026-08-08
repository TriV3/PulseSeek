import { useCallback, useEffect, useState } from "react";
import {
  addFolderBookmark,
  listFolderBookmarks,
  removeFolderBookmark,
  type FolderBookmarkData,
} from "../api/commandEnvelope";

export function useFolderBookmarks() {
  const [bookmarks, setBookmarks] = useState<FolderBookmarkData[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    void listFolderBookmarks()
      .then((loaded) => {
        if (active) setBookmarks(loaded);
      })
      .catch(() => {
        if (active) setError("Bookmarks are unavailable.");
      })
      .finally(() => {
        if (active) setIsLoading(false);
      });
    return () => {
      active = false;
    };
  }, []);

  const toggle = useCallback(
    async (path: string) => {
      const existing = bookmarks.some((bookmark) => bookmark.path === path);
      try {
        if (existing) {
          await removeFolderBookmark(path);
          setBookmarks((current) =>
            current.filter((bookmark) => bookmark.path !== path),
          );
        } else {
          await addFolderBookmark(path);
          const name =
            path.replace(/\/+$/, "").split("/").filter(Boolean).at(-1) ?? path;
          setBookmarks((current) =>
            [...current, { path, name }].sort((a, b) =>
              a.name.localeCompare(b.name),
            ),
          );
        }
        setError(null);
        return true;
      } catch {
        setError("Could not update bookmarks.");
        return false;
      }
    },
    [bookmarks],
  );

  return {
    bookmarks,
    isLoading,
    error,
    toggle,
    isBookmarked: (path: string | null) =>
      path !== null && bookmarks.some((bookmark) => bookmark.path === path),
  };
}
