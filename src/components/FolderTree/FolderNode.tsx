import { useCallback } from "react";
import { FolderIcon, type FolderIconKind } from "./FolderIcon";
import type {
  BrowserLibraryKind,
  BrowserRootKind,
  FolderState,
} from "./folderTreeTypes";

interface FolderNodeProps {
  /** Full filesystem path of this folder. */
  path: string;
  /** Display name (last segment of the path). */
  name: string;
  /** Depth level for indentation. */
  depth: number;
  /** Folder state from the tree. */
  state: FolderState;
  /** Loaded state for descendant folders. */
  folders: Record<string, FolderState>;
  playableEntries: Record<string, Array<{ id: string }>>;
  /** Path of the currently selected item (for computing aria-selected). */
  selectedPath: string | null;
  /** Full path of the selected audio file, used to highlight its ancestors. */
  activeFilePath?: string | null;
  /** Reports whether any rendered filesystem path is bookmarked. */
  isPathBookmarked?: (path: string) => boolean;
  /** Special icon for a system, physical, or network root. */
  rootKind?: BrowserRootKind | "computer";
  libraryKind?: BrowserLibraryKind;
  /** Called when the expand/collapse toggle is clicked. */
  onToggle: (path: string) => void;
  /** Called when the folder name is clicked (select). */
  onSelect: (path: string) => void;
}

export function FolderNode({
  path,
  name,
  depth,
  state,
  folders,
  playableEntries,
  selectedPath,
  activeFilePath = null,
  isPathBookmarked,
  rootKind,
  libraryKind,
  onToggle,
  onSelect,
}: FolderNodeProps) {
  const isSelected = path === selectedPath;
  const isOnAudioPath = folderContainsFile(path, activeFilePath);
  const isBookmarked = isPathBookmarked?.(path) ?? false;
  const canExpand =
    state.hasSubfolders ??
    (state.isLoading || state.hasLoaded !== true || state.children.length > 0);
  const iconKind: FolderIconKind = libraryKind ?? rootKind ?? "folder";

  const handleToggle = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      onToggle(path);
    },
    [path, onToggle],
  );

  const handleSelect = useCallback(() => {
    onSelect(path);
    if (canExpand) onToggle(path);
  }, [canExpand, path, onSelect, onToggle]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        onSelect(path);
        if (canExpand) onToggle(path);
      }
    },
    [canExpand, path, onSelect, onToggle],
  );

  return (
    <li
      role="treeitem"
      aria-expanded={canExpand ? state.expanded : undefined}
      aria-selected={isSelected}
      tabIndex={isSelected ? 0 : -1}
      className={`folder-node${isSelected ? " selected" : ""}${
        isOnAudioPath ? " folder-node--audio-path" : ""
      }${isBookmarked ? " folder-node--bookmarked" : ""}`}
      data-bookmarked={isBookmarked || undefined}
      data-depth={depth}
      data-folder-path={path}
      onKeyDown={handleKeyDown}
    >
      <div className="folder-node-row" onClick={handleSelect}>
        {/* Expand / collapse toggle */}
        {canExpand ? (
          <button
            type="button"
            className="folder-toggle"
            onClick={handleToggle}
            aria-label={state.expanded ? "Collapse folder" : "Expand folder"}
            tabIndex={-1}
          >
            {state.isLoading ? (
              <span className="folder-spinner" aria-label="Loading" />
            ) : (
              <span
                className={`folder-arrow${state.expanded ? " expanded" : ""}`}
              >
                &#9654;
              </span>
            )}
          </button>
        ) : (
          <span className="folder-toggle" aria-hidden="true" />
        )}

        {/* Folder icon + name */}
        <FolderIcon kind={iconKind} expanded={state.expanded} />
        <span className="folder-name">{name}</span>
      </div>

      {/* Error message */}
      {state.error && <div className="folder-error">{state.error}</div>}

      {/* Children (subfolders) */}
      {canExpand && state.expanded && !state.error && (
        <ul role="group" className="folder-children">
          {state.isLoading && state.children.length === 0 && (
            <li className="folder-empty">Loading&#8230;</li>
          )}
          {!state.isLoading &&
            state.children.length === 0 &&
            (playableEntries[path]?.length ?? 0) === 0 && (
              <li className="folder-empty">(empty)</li>
            )}
          {state.children.map((child) => {
            const childPath = child.id;
            return (
              <FolderNode
                key={childPath}
                path={childPath}
                name={child.name}
                depth={depth + 1}
                state={
                  folders[childPath] ?? {
                    expanded: false,
                    children: [],
                    isLoading: false,
                    hasLoaded: false,
                    error: null,
                  }
                }
                folders={folders}
                playableEntries={playableEntries}
                selectedPath={selectedPath}
                activeFilePath={activeFilePath}
                isPathBookmarked={isPathBookmarked}
                rootKind={child.rootKind}
                libraryKind={child.libraryKind}
                onToggle={onToggle}
                onSelect={onSelect}
              />
            );
          })}
        </ul>
      )}
    </li>
  );
}

/** True only when `folderPath` is a complete directory segment of the file path. */
function folderContainsFile(
  folderPath: string,
  activeFilePath: string | null,
): boolean {
  if (!activeFilePath || folderPath === "computer://") return false;
  const normalizedFolder = folderPath.replace(/\/+$/, "");
  if (normalizedFolder === "") return activeFilePath.startsWith("/");
  return activeFilePath.startsWith(`${normalizedFolder}/`);
}
