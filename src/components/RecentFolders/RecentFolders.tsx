import type { RecentFolderData } from "../../api/commandEnvelope";
import "./RecentFolders.css";

export interface RecentFoldersProps {
  folders: RecentFolderData[];
  isLoading: boolean;
  error: string | null;
  /** Reopens a folder from the history (FR-BR-011). */
  onReopen: (path: string) => void;
  /** Clears the whole history after backend confirmation. */
  onClear: () => void;
}

/**
 * Bounded recent-folder history shown in the browser sidebar.
 *
 * Each entry is a plain button so keyboard users can reopen with Enter or
 * Space. The display name is the basename; the full path is only exposed as a
 * hover title so personal paths are not announced unless the user asks.
 */
export function RecentFolders({
  folders,
  isLoading,
  error,
  onReopen,
  onClear,
}: RecentFoldersProps) {
  return (
    <section className="recent-folders" aria-label="Recent folders">
      <div className="recent-folders-header">
        <h2 className="recent-folders-title">Recent folders</h2>
        {folders.length > 0 && (
          <button
            type="button"
            className="recent-folders-clear"
            aria-label="Clear recent folders"
            onClick={onClear}
          >
            Clear
          </button>
        )}
      </div>
      {error && (
        <p className="recent-folders-error" role="alert">
          {error}
        </p>
      )}
      {folders.length === 0 ? (
        <p className="recent-folders-empty">
          {isLoading ? "Loading recent folders…" : "No recent folders yet."}
        </p>
      ) : (
        <ul className="recent-folders-list">
          {folders.map((folder) => (
            <li key={folder.path} className="recent-folders-item">
              <button
                type="button"
                className="recent-folders-button"
                title={folder.path}
                onClick={() => onReopen(folder.path)}
              >
                {folder.name}
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
