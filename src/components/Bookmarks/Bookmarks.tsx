import type { FolderBookmarkData } from "../../api/commandEnvelope";
import "../RecentFolders/RecentFolders.css";

export function Bookmarks({
  bookmarks,
  isLoading,
  error,
  onReopen,
  onRemove,
}: {
  bookmarks: FolderBookmarkData[];
  isLoading: boolean;
  error: string | null;
  onReopen: (path: string) => void;
  onRemove: (path: string) => void;
}) {
  return (
    <section className="recent-folders" aria-label="Bookmarks">
      <div className="recent-folders-header">
        <h2 className="recent-folders-title">Bookmarks</h2>
      </div>
      {error && (
        <p className="recent-folders-error" role="alert">
          {error}
        </p>
      )}
      {bookmarks.length === 0 ? (
        <p className="recent-folders-empty">
          {isLoading ? "Loading bookmarks…" : "No bookmarks yet."}
        </p>
      ) : (
        <ul className="recent-folders-list">
          {bookmarks.map((bookmark) => (
            <li
              key={bookmark.path}
              className="recent-folders-item bookmark-item"
            >
              <button
                type="button"
                className="recent-folders-button"
                title={bookmark.path}
                onClick={() => onReopen(bookmark.path)}
              >
                {bookmark.name}
              </button>
              <button
                type="button"
                className="bookmark-remove"
                aria-label={`Remove ${bookmark.name} bookmark`}
                onClick={() => onRemove(bookmark.path)}
              >
                ×
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
