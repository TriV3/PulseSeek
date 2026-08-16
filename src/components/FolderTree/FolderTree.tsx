import { useCallback, useEffect, useRef, useState } from "react";
import type { FolderState, FolderTreeState } from "./folderTreeTypes";
import { FolderNode } from "./FolderNode";
import { ContextMenu } from "../ContextMenu/ContextMenu";
import "./FolderTree.css";

export interface FolderTreeProps {
  state: FolderTreeState;
  toggleExpand: (path: string) => void;
  selectFolder: (path: string) => void;
  navigateUp: () => void;
  clearError: () => void;
  /** Full path of the selected audio file, if any. */
  activeFilePath?: string | null;
  isBookmarked?: boolean;
  isPathBookmarked?: (path: string) => boolean;
  toggleBookmark?: (path: string) => void;
}

export function FolderTree({
  state,
  toggleExpand,
  selectFolder,
  navigateUp,
  clearError,
  activeFilePath = null,
  isBookmarked = false,
  isPathBookmarked,
  toggleBookmark,
}: FolderTreeProps) {
  const treeRef = useRef<HTMLDivElement | null>(null);
  const [drivesExpanded, setDrivesExpanded] = useState(true);
  const [librariesExpanded, setLibrariesExpanded] = useState(true);
  const [contextTarget, setContextTarget] = useState<{
    path: string;
    name: string;
    expanded: boolean;
    canExpand: boolean;
    x: number;
    y: number;
    anchor: HTMLElement;
  } | null>(null);

  const openFolderContextMenu = useCallback(
    (
      path: string,
      name: string,
      folder: FolderState,
      canExpand: boolean,
      x: number,
      y: number,
      anchor: HTMLElement,
    ) => {
      selectFolder(path);
      setContextTarget({
        path,
        name,
        expanded: folder.expanded,
        canExpand,
        x,
        y,
        anchor,
      });
    },
    [selectFolder],
  );

  useEffect(() => {
    const selectedPath = state.selectedPath;
    if (!selectedPath) return;
    const target = Array.from(
      treeRef.current?.querySelectorAll<HTMLElement>("[data-folder-path]") ??
        [],
    ).find((element) => element.dataset.folderPath === selectedPath);
    if (typeof target?.scrollIntoView === "function") {
      target.scrollIntoView({ block: "nearest" });
    }
  }, [state.folders, state.selectedPath]);

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
          <span className="folder-tree-loading">Loading disks…</span>
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
      ref={treeRef}
      role="tree"
      aria-label="Folder browser"
      onKeyDown={handleKeyDown}
    >
      {/* Toolbar */}
      <div className="folder-tree-toolbar">
        <button
          type="button"
          className="go-up-btn"
          onClick={navigateUp}
          disabled={!state.selectedPath}
          aria-label="Go to parent folder"
        >
          Go Up
        </button>
        <button
          type="button"
          className="bookmark-folder-btn"
          disabled={!state.selectedPath || state.selectedPath === "computer://"}
          aria-label={
            isBookmarked ? "Remove folder bookmark" : "Bookmark folder"
          }
          aria-pressed={isBookmarked}
          onClick={() =>
            state.selectedPath && toggleBookmark?.(state.selectedPath)
          }
        >
          {isBookmarked ? "★" : "☆"}
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
      {state.rootPath === "computer://" ? (
        <div className="folder-tree-sections">
          <BrowserSection
            label="Drives"
            expanded={drivesExpanded}
            onToggle={() => setDrivesExpanded((value) => !value)}
          >
            {rootFolderState.children.map((entry) => (
              <FolderNode
                key={entry.id}
                path={entry.id}
                name={entry.name}
                depth={0}
                state={state.folders[entry.id]}
                folders={state.folders}
                playableEntries={state.playableEntries}
                selectedPath={state.selectedPath}
                activeFilePath={activeFilePath}
                isPathBookmarked={isPathBookmarked}
                rootKind={entry.rootKind}
                onToggle={toggleExpand}
                onSelect={selectFolder}
                onContextMenu={openFolderContextMenu}
              />
            ))}
          </BrowserSection>
          <BrowserSection
            label="Libraries"
            expanded={librariesExpanded}
            onToggle={() => setLibrariesExpanded((value) => !value)}
          >
            {state.libraries.map((entry) => (
              <FolderNode
                key={entry.id}
                path={entry.id}
                name={entry.name}
                depth={0}
                state={state.folders[entry.id]}
                folders={state.folders}
                playableEntries={state.playableEntries}
                selectedPath={state.selectedPath}
                activeFilePath={activeFilePath}
                isPathBookmarked={isPathBookmarked}
                libraryKind={entry.libraryKind}
                onToggle={toggleExpand}
                onSelect={selectFolder}
                onContextMenu={openFolderContextMenu}
              />
            ))}
          </BrowserSection>
        </div>
      ) : (
        <ul className="folder-tree-root" role="group">
          <FolderNode
            path={state.rootPath}
            name={
              state.rootPath === "computer://"
                ? "Computer"
                : (state.rootPath.split("/").filter(Boolean).pop() ??
                  state.rootPath)
            }
            depth={0}
            state={rootFolderState}
            folders={state.folders}
            playableEntries={state.playableEntries}
            selectedPath={state.selectedPath}
            activeFilePath={activeFilePath}
            isPathBookmarked={isPathBookmarked}
            rootKind={state.rootPath === "computer://" ? "computer" : undefined}
            onToggle={toggleExpand}
            onSelect={selectFolder}
            onContextMenu={openFolderContextMenu}
          />
        </ul>
      )}
      {contextTarget ? (
        <ContextMenu
          label={`Folder actions for ${contextTarget.name}`}
          x={contextTarget.x}
          y={contextTarget.y}
          returnFocus={contextTarget.anchor}
          onClose={() => setContextTarget(null)}
        >
          <button
            type="button"
            role="menuitem"
            onClick={() => selectFolder(contextTarget.path)}
          >
            Open
          </button>
          {contextTarget.canExpand ? (
            <button
              type="button"
              role="menuitem"
              onClick={() => toggleExpand(contextTarget.path)}
            >
              {contextTarget.expanded ? "Collapse folder" : "Expand folder"}
            </button>
          ) : null}
          <div className="context-menu-separator" role="separator" />
          <button
            type="button"
            role="menuitem"
            disabled={contextTarget.path === "computer://" || !toggleBookmark}
            onClick={() => toggleBookmark?.(contextTarget.path)}
          >
            {(isPathBookmarked?.(contextTarget.path) ??
            (contextTarget.path === state.selectedPath && isBookmarked))
              ? "Remove folder bookmark"
              : "Bookmark folder"}
          </button>
        </ContextMenu>
      ) : null}
    </div>
  );
}

function BrowserSection({
  label,
  expanded,
  onToggle,
  children,
}: {
  label: string;
  expanded: boolean;
  onToggle: () => void;
  children: React.ReactNode;
}) {
  return (
    <section className="browser-section">
      <button
        type="button"
        className="browser-section-header"
        aria-expanded={expanded}
        onClick={onToggle}
      >
        <span
          className={`folder-arrow${expanded ? " expanded" : ""}`}
          aria-hidden="true"
        >
          ▶
        </span>
        {label}
      </button>
      {expanded && (
        <ul className="folder-tree-root" role="group">
          {children}
        </ul>
      )}
    </section>
  );
}
