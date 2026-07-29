import { useCallback } from "react";
import type { FolderState } from "./folderTreeTypes";

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
  /** Path of the currently selected item (for computing aria-selected). */
  selectedPath: string | null;
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
  selectedPath,
  onToggle,
  onSelect,
}: FolderNodeProps) {
  const isSelected = path === selectedPath;

  const handleToggle = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      onToggle(path);
    },
    [path, onToggle],
  );

  const handleSelect = useCallback(() => {
    onSelect(path);
  }, [path, onSelect]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        onToggle(path);
      }
    },
    [path, onToggle],
  );

  const indent = depth * 16;

  return (
    <li
      role="treeitem"
      aria-expanded={state.expanded}
      aria-selected={isSelected}
      tabIndex={isSelected ? 0 : -1}
      className={`folder-node${isSelected ? " selected" : ""}`}
      style={{ paddingLeft: `${indent + 8}px` }}
      onKeyDown={handleKeyDown}
    >
      <div className="folder-node-row" onClick={handleSelect}>
        {/* Expand / collapse toggle */}
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

        {/* Folder icon + name */}
        <span className="folder-icon" aria-hidden="true">
          {state.expanded ? "\uD83D\uDCC2" : "\uD83D\uDCC1"}
        </span>
        <span className="folder-name">{name}</span>
      </div>

      {/* Error message */}
      {state.error && (
        <div
          className="folder-error"
          style={{ paddingLeft: `${indent + 24}px` }}
        >
          {state.error}
        </div>
      )}

      {/* Children (subfolders) */}
      {state.expanded && !state.error && (
        <ul role="group" className="folder-children">
          {state.isLoading && state.children.length === 0 && (
            <li
              className="folder-empty"
              style={{ paddingLeft: `${indent + 24}px` }}
            >
              Loading&#8230;
            </li>
          )}
          {!state.isLoading && state.children.length === 0 && (
            <li
              className="folder-empty"
              style={{ paddingLeft: `${indent + 24}px` }}
            >
              (empty)
            </li>
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
                    error: null,
                  }
                }
                folders={folders}
                selectedPath={selectedPath}
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
