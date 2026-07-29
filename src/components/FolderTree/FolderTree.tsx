import { useCallback } from "react";
import type { FolderTreeState } from "./folderTreeTypes";
import { FolderNode } from "./FolderNode";
import "./FolderTree.css";

export interface FolderTreeProps {
  state: FolderTreeState;
  openFolder: () => Promise<void>;
  toggleExpand: (path: string) => void;
  selectFolder: (path: string) => void;
  navigateUp: () => void;
  clearError: () => void;
}

export function FolderTree({
  state,
  openFolder,
  toggleExpand,
  selectFolder,
  navigateUp,
  clearError,
}: FolderTreeProps) {
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      const selected = state.selectedPath;
      if (!selected) return;

      const allPaths = Object.keys(state.folders);
      const currentIndex = allPaths.indexOf(selected);

      switch (e.key) {
        case "ArrowDown": {
          e.preventDefault();
          const nextIndex = Math.min(currentIndex + 1, allPaths.length - 1);
          if (nextIndex >= 0) {
            selectFolder(allPaths[nextIndex]);
          }
          break;
        }
        case "ArrowUp": {
          e.preventDefault();
          const prevIndex = Math.max(currentIndex - 1, 0);
          if (prevIndex < allPaths.length) {
            selectFolder(allPaths[prevIndex]);
          }
          break;
        }
        case "ArrowRight": {
          e.preventDefault();
          const folder = state.folders[selected];
          if (folder && !folder.expanded) {
            toggleExpand(selected);
          }
          break;
        }
        case "ArrowLeft": {
          e.preventDefault();
          const folder = state.folders[selected];
          if (folder && folder.expanded) {
            toggleExpand(selected);
          } else {
            navigateUp();
          }
          break;
        }
      }
    },
    [state.selectedPath, state.folders, selectFolder, toggleExpand, navigateUp],
  );

  if (!state.rootPath) {
    return (
      <div className="folder-tree" role="tree" aria-label="Folder browser">
        <div className="folder-tree-toolbar">
          <button
            type="button"
            className="open-folder-btn"
            onClick={openFolder}
            disabled={state.status === "picking"}
          >
            {state.status === "picking" ? "Opening\u2026" : "Open Folder"}
          </button>
        </div>
        {state.errorMessage && (
          <div
            className="folder-tree-banner folder-tree-banner--error"
            role="alert"
          >
            <span>{state.errorMessage}</span>
            <button
              type="button"
              className="banner-close-btn"
              onClick={clearError}
              aria-label="Dismiss error"
            >
              &times;
            </button>
          </div>
        )}
      </div>
    );
  }

  const rootFolder = state.folders[state.rootPath];
  const rootFolderState = rootFolder ?? {
    expanded: true,
    children: [],
    isLoading: false,
    error: null as string | null,
  };

  return (
    <div
      className="folder-tree"
      role="tree"
      aria-label="Folder browser"
      onKeyDown={handleKeyDown}
    >
      {/* Toolbar */}
      <div className="folder-tree-toolbar">
        <button
          type="button"
          className="open-folder-btn"
          onClick={openFolder}
          disabled={state.status === "picking"}
        >
          {state.status === "picking" ? "Opening\u2026" : "Open Folder"}
        </button>
        <button
          type="button"
          className="go-up-btn"
          onClick={navigateUp}
          disabled={!state.selectedPath}
          aria-label="Go to parent folder"
        >
          Go Up
        </button>
        <span className="current-path" title={state.rootPath}>
          {state.rootPath}
        </span>
      </div>

      {/* Error banner */}
      {state.errorMessage && (
        <div
          className="folder-tree-banner folder-tree-banner--error"
          role="alert"
        >
          <span>{state.errorMessage}</span>
          <button
            type="button"
            className="banner-close-btn"
            onClick={clearError}
            aria-label="Dismiss error"
          >
            &times;
          </button>
        </div>
      )}

      {/* Tree */}
      <ul className="folder-tree-root" role="group">
        <FolderNode
          path={state.rootPath}
          name={
            state.rootPath.split("/").filter(Boolean).pop() ?? state.rootPath
          }
          depth={0}
          state={rootFolderState}
          folders={state.folders}
          selectedPath={state.selectedPath}
          onToggle={toggleExpand}
          onSelect={selectFolder}
        />
      </ul>
    </div>
  );
}
