import { useCallback, useEffect, useReducer, useRef } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import {
  cancelEnumeration,
  listBrowserRoots,
  startEnumeration,
} from "../api/commandEnvelope";
import {
  onFileChanged,
  onFolderChunk,
  type FolderChunkPayload,
} from "../api/playbackEvents";
import type {
  BrowserEntry,
  FolderTreeAction,
  FolderTreeState,
} from "../components/FolderTree/folderTreeTypes";
import {
  INITIAL_FOLDER_TREE_STATE,
  getParentPath,
} from "../components/FolderTree/folderTreeTypes";

// ── Reducer ──────────────────────────────────────────────────────────────

export function folderTreeReducer(
  state: FolderTreeState,
  action: FolderTreeAction,
): FolderTreeState {
  switch (action.type) {
    case "ROOTS_LOADED": {
      const rootPath = "computer://";
      const rootEntries = action.roots.map((root) => ({
        id: root.path,
        name: root.name,
        kind: "folder" as const,
        rootKind: root.kind,
      }));
      const rootFolders = Object.fromEntries(
        [...action.roots, ...action.libraries].map((root) => [
          root.path,
          {
            expanded: false,
            children: [],
            isLoading: false,
            hasLoaded: false,
            hasSubfolders: null,
            error: null,
            recursive: false,
          },
        ]),
      );
      return {
        ...state,
        rootPath,
        selectedPath: rootPath,
        libraries: action.libraries.map((library) => ({
          id: library.path,
          name: library.name,
          kind: "folder" as const,
          libraryKind: library.kind,
        })),
        folders: {
          ...rootFolders,
          [rootPath]: {
            expanded: true,
            children: rootEntries,
            isLoading: false,
            hasLoaded: true,
            hasSubfolders: rootEntries.length > 0,
            error: null,
            recursive: false,
          },
        },
        status: "ready",
        errorMessage: null,
      };
    }

    case "ROOTS_ERROR":
      return { ...state, status: "error", errorMessage: action.message };

    case "START_PICKING":
      return { ...state, status: "picking", errorMessage: null };

    case "FOLDER_PICKED":
      return {
        ...state,
        rootPath: action.path,
        selectedPath: action.path,
        folders: {
          [action.path]: {
            expanded: false,
            children: [],
            isLoading: true,
            hasLoaded: false,
            hasSubfolders: null,
            error: null,
            recursive: false,
          },
        },
        status: "loading",
        errorMessage: null,
      };

    case "PICK_CANCELLED":
      return { ...state, status: "idle" };

    case "PICK_ERROR":
      return { ...state, status: "error", errorMessage: action.message };

    case "START_ENUMERATING": {
      const existing = state.folders[action.path];
      return {
        ...state,
        activeSessionId: action.sessionId,
        folders: {
          ...state.folders,
          [action.path]: {
            expanded: existing?.expanded ?? true,
            children: [],
            isLoading: true,
            hasLoaded: existing?.hasLoaded ?? false,
            hasSubfolders: existing?.hasSubfolders ?? null,
            error: null,
            recursive: action.recursive,
          },
        },
        playableEntries: {
          ...state.playableEntries,
          [action.path]: [],
        },
        status: "loading",
      };
    }

    case "ENUMERATION_CHUNK": {
      const current = state.folders[action.path];
      if (!current) return state;

      // Merge new folder entries into the children list.
      const newFolders = action.entries.filter((e) => e.kind === "folder");

      // Accumulate playable entries for the file list.
      const playableById = new Map(
        (state.playableEntries[action.path] ?? []).map((entry) => [
          entry.id,
          entry,
        ]),
      );
      for (const entry of action.entries) {
        if (entry.kind === "playable") {
          playableById.set(entry.id, entry);
        } else if (entry.kind !== "folder") {
          playableById.delete(entry.id);
        }
      }

      // Deduplicate folder names.
      const childrenById = new Map(
        current.children.map((entry) => [entry.id, entry]),
      );
      for (const entry of newFolders) {
        childrenById.set(entry.id, entry);
      }

      const discoveredFolderStates = Object.fromEntries(
        newFolders.map((entry) => [
          entry.id,
          {
            ...(state.folders[entry.id] ?? {
              expanded: false,
              children: [],
              isLoading: false,
              hasLoaded: false,
              error: null,
              recursive: false,
            }),
            hasSubfolders:
              entry.has_subfolders ??
              state.folders[entry.id]?.hasSubfolders ??
              null,
          },
        ]),
      );

      return {
        ...state,
        folders: {
          ...state.folders,
          ...discoveredFolderStates,
          [action.path]: {
            ...current,
            children: [...childrenById.values()].sort((left, right) =>
              left.name.localeCompare(right.name, undefined, {
                sensitivity: "base",
              }),
            ),
            // The directory preview can finish before playable files have
            // been validated. Keep the shared loading state active until the
            // whole enumeration completes so neither pane claims the folder
            // is empty while file scanning is still in progress.
            isLoading: !action.done,
            hasLoaded: action.done ? true : current.hasLoaded,
            hasSubfolders:
              action.done || childrenById.size > 0
                ? childrenById.size > 0
                : current.hasSubfolders,
            expanded: current.expanded,
          },
        },
        playableEntries: {
          ...state.playableEntries,
          [action.path]: [...playableById.values()].sort((left, right) =>
            left.name.localeCompare(right.name, undefined, {
              sensitivity: "base",
            }),
          ),
        },
        status: action.done
          ? state.status === "error"
            ? "error"
            : "ready"
          : "loading",
        activeSessionId: action.done ? null : state.activeSessionId,
      };
    }

    case "ENUMERATION_ERROR": {
      const existing = state.folders[action.path];
      return {
        ...state,
        folders: {
          ...state.folders,
          [action.path]: {
            expanded: existing?.expanded ?? true,
            children: existing?.children ?? [],
            isLoading: false,
            hasLoaded: existing?.hasLoaded ?? false,
            hasSubfolders: existing?.hasSubfolders ?? null,
            error: action.message,
            recursive: existing?.recursive ?? false,
          },
        },
        status: "error",
        errorMessage: action.message,
        activeSessionId: null,
      };
    }

    case "TOGGLE_EXPAND": {
      const current = state.folders[action.path];
      if (!current) return state;
      return {
        ...state,
        folders: {
          ...state.folders,
          [action.path]: { ...current, expanded: !current.expanded },
        },
      };
    }

    case "SELECT_FOLDER":
      return { ...state, selectedPath: action.path };

    case "RESTORE_CONTEXT": {
      const expanded = new Set(action.expandedPaths);
      const placeholders = Object.fromEntries(
        action.expandedPaths.map((path) => [
          path,
          state.folders[path] ?? {
            expanded: false,
            children: [],
            isLoading: false,
            hasLoaded: false,
            hasSubfolders: null,
            error: null,
          },
        ]),
      );
      return {
        ...state,
        selectedPath: action.selectedPath,
        folders: Object.fromEntries(
          Object.entries({ ...state.folders, ...placeholders }).map(
            ([path, folder]) => [
              path,
              { ...folder, expanded: expanded.has(path) },
            ],
          ),
        ),
      };
    }

    case "NAVIGATE_UP": {
      if (!state.selectedPath) return state;
      const parentPath = getParentPath(state.selectedPath);
      if (!parentPath || parentPath === state.selectedPath) return state;
      return {
        ...state,
        selectedPath: parentPath,
        folders: {
          ...state.folders,
          [parentPath]: {
            ...(state.folders[parentPath] ?? {
              expanded: false,
              children: [],
              isLoading: false,
              hasLoaded: false,
              hasSubfolders: null,
              error: null,
            }),
            expanded: false,
          },
        },
      };
    }

    case "CLEAR_ERROR":
      return { ...state, status: "idle", errorMessage: null };

    case "REMOVE_ENTRIES": {
      const existing = state.playableEntries[action.path];
      if (!existing) return state;
      const removeSet = new Set(action.entryIds);
      const remaining = existing.filter((e) => !removeSet.has(e.id));
      if (remaining.length === existing.length) return state;
      return {
        ...state,
        playableEntries: {
          ...state.playableEntries,
          [action.path]: remaining,
        },
      };
    }

    case "RENAME_ENTRY": {
      // A rename keeps the row (same file) but moves it to the new stable
      // entry id, so the visible item and cache identity stay consistent
      // (FR-FM-004, FR-FM-010). Folder children and playable entries both
      // update; metadata carries over unchanged.
      const { path, oldId, newId, newName } = action;
      const folder = state.folders[path];
      const playable = state.playableEntries[path];
      if (!folder && !playable) return state;

      const replaceEntry = (entry: BrowserEntry): BrowserEntry =>
        entry.id === oldId ? { ...entry, id: newId, name: newName } : entry;

      const folderChildren = folder
        ? { [path]: { ...folder, children: folder.children.map(replaceEntry) } }
        : {};
      const playableEntries = playable
        ? {
            ...state.playableEntries,
            [path]: playable.map(replaceEntry),
          }
        : state.playableEntries;

      return {
        ...state,
        folders: { ...state.folders, ...folderChildren },
        playableEntries,
      };
    }

    default:
      return state;
  }
}

// ── Hook ─────────────────────────────────────────────────────────────────

const SESSION_PATHS: Record<string, string> = {};
const SESSION_COMPLETIONS: Record<string, () => void> = {};
// Buffer for folder-chunk events that arrive before their session mapping
// is registered (race between Rust worker emitting and JavaScript setting
// the session→path mapping).
const PENDING_CHUNKS: Record<string, FolderChunkPayload[]> = {};

export function getRestorationPaths(selectedPath: string): string[] {
  const ancestors: string[] = [];
  let current: string | null = selectedPath;
  while (current && current !== "computer://") {
    ancestors.push(current);
    current = getParentPath(current);
  }
  if (selectedPath.startsWith("/") && !ancestors.includes("/")) {
    ancestors.push("/");
  }
  return ancestors.reverse();
}

export interface UseFolderTreeReturn {
  state: FolderTreeState;
  toggleExpand: (path: string) => void;
  selectFolder: (path: string) => void;
  navigateUp: () => void;
  clearError: () => void;
  removeEntries: (path: string, entryIds: string[]) => void;
  /** Replaces `oldId` with its renamed id and name inside `path`. */
  renameEntry: (
    path: string,
    oldId: string,
    newId: string,
    newName: string,
  ) => void;
  restoreContext: (selectedPath: string) => Promise<string>;
  setRecursive: (path: string, recursive: boolean) => void;
  refreshSelected: () => void;
}

export function useFolderTree(showHiddenFolders = false): UseFolderTreeReturn {
  const [state, dispatch] = useReducer(
    folderTreeReducer,
    INITIAL_FOLDER_TREE_STATE,
  );
  const stateRef = useRef(state);
  const showHiddenFoldersRef = useRef(showHiddenFolders);
  const pendingWatcherRefreshPathsRef = useRef(new Set<string>());

  // Keep ref synchronised with latest state after each render.
  useEffect(() => {
    stateRef.current = state;
    showHiddenFoldersRef.current = showHiddenFolders;
  });

  // Set up event listener for folder chunk events.
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    (async () => {
      unlisten = await onFolderChunk((payload) => {
        const path = SESSION_PATHS[payload.session_id];
        if (!path) {
          // Session mapping not yet registered — buffer for replay.
          const buffer = PENDING_CHUNKS[payload.session_id];
          if (buffer) {
            buffer.push(payload);
          } else {
            PENDING_CHUNKS[payload.session_id] = [payload];
          }
          return;
        }
        dispatch({
          type: "ENUMERATION_CHUNK",
          path,
          entries: payload.entries,
          foldersDone: payload.folders_done ?? payload.done,
          done: payload.done,
        });
        if (payload.done) {
          delete SESSION_PATHS[payload.session_id];
          SESSION_COMPLETIONS[payload.session_id]?.();
          delete SESSION_COMPLETIONS[payload.session_id];
        }
      });
    })();

    return () => {
      unlisten?.();
    };
  }, []);

  const enumeratePath = useCallback(async (path: string, recursive = false) => {
    const current = stateRef.current;
    // Cancel any in-flight enumeration.
    if (current.activeSessionId) {
      try {
        await cancelEnumeration(current.activeSessionId);
      } catch {
        // Best-effort cancel.
      }
    }

    try {
      const sessionId = await startEnumeration(
        path,
        undefined,
        recursive,
        showHiddenFoldersRef.current,
      );
      // Register mapping before dispatching START_ENUMERATING so that
      // any chunks buffered during the await are applied to the correct
      // folder state after the reducer resets entries.
      SESSION_PATHS[sessionId] = path;
      dispatch({ type: "START_ENUMERATING", path, sessionId, recursive });
      // Replay any folder-chunk events that arrived before the mapping
      // was registered (race between Rust worker and JavaScript handler).
      // Atomically drain the buffer so new events go directly to dispatch.
      const buffered = PENDING_CHUNKS[sessionId];
      delete PENDING_CHUNKS[sessionId];
      if (buffered) {
        for (const payload of buffered) {
          dispatch({
            type: "ENUMERATION_CHUNK",
            path,
            entries: payload.entries,
            foldersDone: payload.folders_done ?? payload.done,
            done: payload.done,
          });
          if (payload.done) {
            delete SESSION_PATHS[sessionId];
          }
        }
      }
    } catch (err: unknown) {
      const message =
        err instanceof Error ? err.message : "Failed to enumerate folder.";
      dispatch({ type: "ENUMERATION_ERROR", path, message });
    }
  }, []);

  const previousShowHiddenFoldersRef = useRef(showHiddenFolders);
  useEffect(() => {
    if (previousShowHiddenFoldersRef.current === showHiddenFolders) return;
    previousShowHiddenFoldersRef.current = showHiddenFolders;
    const selected = stateRef.current.selectedPath;
    if (!selected || selected === "computer://") return;
    const recursive = stateRef.current.folders[selected]?.recursive ?? false;
    void enumeratePath(selected, recursive);
  }, [enumeratePath, showHiddenFolders]);

  useEffect(() => {
    let active = true;
    try {
      void listBrowserRoots()
        .then(({ roots, libraries }) => {
          if (!active) return;
          dispatch({ type: "ROOTS_LOADED", roots, libraries });
        })
        .catch((error: unknown) => {
          if (!active) return;
          dispatch({
            type: "ROOTS_ERROR",
            message:
              error instanceof Error
                ? error.message
                : "Failed to list disks and network volumes.",
          });
        });
    } catch (error: unknown) {
      dispatch({
        type: "ROOTS_ERROR",
        message:
          error instanceof Error
            ? error.message
            : "Failed to list disks and network volumes.",
      });
    }
    return () => {
      active = false;
    };
  }, [enumeratePath]);

  // Re-read folders whose contents changed externally (FR-BR-008). The file
  // watcher debounces bursts into a single event per watched folder; only
  // folders that are loaded and idle are re-enumerated so an in-flight scan is
  // never restarted. The reducer merges new rows by entry id, so the current
  // selection survives when its stable target remains.
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    void onFileChanged((payload) => {
      const { path } = payload;
      if (stateRef.current.selectedPath !== path) return;
      const folder = stateRef.current.folders[path];
      if (!folder || folder.isLoading || folder.error) return;
      if (pendingWatcherRefreshPathsRef.current.has(path)) return;
      pendingWatcherRefreshPathsRef.current.add(path);
      void enumeratePath(path, folder.recursive).finally(() => {
        pendingWatcherRefreshPathsRef.current.delete(path);
      });
    })
      .then((unlistenFn) => {
        unlisten = unlistenFn;
      })
      .catch(() => {
        // Listening is best-effort; the manual refresh still works.
      });
    return () => {
      unlisten?.();
    };
  }, [enumeratePath]);

  const toggleExpand = useCallback(
    (path: string) => {
      const folder = stateRef.current.folders[path];
      // If the folder has no children yet and is not currently loading, start
      // enumeration.
      if (
        folder &&
        !folder.expanded &&
        !folder.isLoading &&
        folder.children.length === 0 &&
        !folder.error
      ) {
        enumeratePath(path);
      }
      dispatch({ type: "TOGGLE_EXPAND", path });
    },
    [enumeratePath],
  );

  const selectFolder = useCallback(
    (path: string) => {
      const folder = stateRef.current.folders[path];
      dispatch({ type: "SELECT_FOLDER", path });

      // A known leaf has no expand control, so selecting it is the only user
      // action available to start its file scan. Keep expansion and file
      // enumeration independent: leaves remain arrowless while their audio
      // rows are loaded.
      if (
        folder?.hasSubfolders === false &&
        !folder.hasLoaded &&
        !folder.isLoading &&
        !folder.error
      ) {
        void enumeratePath(path, folder.recursive);
      }
    },
    [enumeratePath],
  );

  const navigateUp = useCallback(() => {
    const selected = stateRef.current.selectedPath;
    if (!selected) return;
    const parent = getParentPath(selected);
    if (parent) {
      // Enumerate parent if not already loaded.
      const parentFolder = stateRef.current.folders[parent];
      if (!parentFolder || (!parentFolder.isLoading && !parentFolder.error)) {
        enumeratePath(parent);
      }
    }
    dispatch({ type: "NAVIGATE_UP" });
  }, [enumeratePath]);

  const clearError = useCallback(() => {
    dispatch({ type: "CLEAR_ERROR" });
  }, []);

  const removeEntries = useCallback((path: string, entryIds: string[]) => {
    dispatch({ type: "REMOVE_ENTRIES", path, entryIds });
  }, []);

  const renameEntry = useCallback(
    (path: string, oldId: string, newId: string, newName: string) => {
      dispatch({ type: "RENAME_ENTRY", path, oldId, newId, newName });
    },
    [],
  );

  const setRecursive = useCallback(
    (path: string, recursive: boolean) => {
      void enumeratePath(path, recursive);
    },
    [enumeratePath],
  );

  const refreshSelected = useCallback(() => {
    const selected = stateRef.current.selectedPath;
    if (!selected || selected === "computer://") return;
    const recursive = stateRef.current.folders[selected]?.recursive ?? false;
    void enumeratePath(selected, recursive);
  }, [enumeratePath]);

  const restoreContext = useCallback((selectedPath: string) => {
    const paths = getRestorationPaths(selectedPath);
    dispatch({
      type: "RESTORE_CONTEXT",
      selectedPath,
      expandedPaths: ["computer://", ...paths],
    });
    return (async () => {
      // The saved path is already authoritative. Starting every known level
      // immediately avoids waiting for a full metadata scan of each ancestor
      // before the final folder (and its last-played file) can appear.
      const results = await Promise.all(
        paths.map(async (path) => {
          try {
            const sessionId = await startEnumeration(
              path,
              undefined,
              false,
              showHiddenFoldersRef.current,
            );
            SESSION_PATHS[sessionId] = path;
            dispatch({
              type: "START_ENUMERATING",
              path,
              sessionId,
              recursive: false,
            });
            const buffered = PENDING_CHUNKS[sessionId];
            delete PENDING_CHUNKS[sessionId];
            for (const payload of buffered ?? []) {
              dispatch({
                type: "ENUMERATION_CHUNK",
                path,
                entries: payload.entries,
                foldersDone: payload.folders_done ?? payload.done,
                done: payload.done,
              });
              if (payload.done) delete SESSION_PATHS[sessionId];
            }
            return true;
          } catch {
            return false;
          }
        }),
      );
      const firstFailure = results.findIndex((result) => !result);
      if (firstFailure === -1) return selectedPath;

      const restoredPaths = paths.slice(0, firstFailure);
      const fallback = restoredPaths.at(-1) ?? "computer://";
      dispatch({
        type: "RESTORE_CONTEXT",
        selectedPath: fallback,
        expandedPaths: ["computer://", ...restoredPaths],
      });
      return fallback;
    })();
  }, []);

  return {
    state,
    toggleExpand,
    selectFolder,
    navigateUp,
    clearError,
    removeEntries,
    renameEntry,
    restoreContext,
    setRecursive,
    refreshSelected,
  };
}
