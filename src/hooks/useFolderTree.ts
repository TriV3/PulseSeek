import { useCallback, useEffect, useReducer, useRef } from "react";
import {
  cancelEnumeration,
  listBrowserRoots,
  startEnumeration,
} from "../api/commandEnvelope";
import { onFolderChunk, type FolderChunkPayload } from "../api/playbackEvents";
import type {
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
      }));
      const rootFolders = Object.fromEntries(
        action.roots.map((root) => [
          root.path,
          {
            expanded: false,
            children: [],
            isLoading: false,
            error: null,
          },
        ]),
      );
      return {
        ...state,
        rootPath,
        selectedPath: rootPath,
        folders: {
          ...rootFolders,
          [rootPath]: {
            expanded: false,
            children: rootEntries,
            isLoading: false,
            error: null,
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
            expanded: true,
            children: [],
            isLoading: true,
            error: null,
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
            error: null,
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
          state.folders[entry.id] ?? {
            expanded: false,
            children: [],
            isLoading: false,
            error: null,
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
            children: [...childrenById.values()],
            isLoading: !(action.foldersDone ?? action.done),
            expanded: true,
          },
        },
        playableEntries: {
          ...state.playableEntries,
          [action.path]: [...playableById.values()],
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
            error: action.message,
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
              error: null,
            }),
            expanded: true,
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

    default:
      return state;
  }
}

// ── Hook ─────────────────────────────────────────────────────────────────

const SESSION_PATHS: Record<string, string> = {};
// Buffer for folder-chunk events that arrive before their session mapping
// is registered (race between Rust worker emitting and JavaScript setting
// the session→path mapping).
const PENDING_CHUNKS: Record<string, FolderChunkPayload[]> = {};

export interface UseFolderTreeReturn {
  state: FolderTreeState;
  toggleExpand: (path: string) => void;
  selectFolder: (path: string) => void;
  navigateUp: () => void;
  clearError: () => void;
  removeEntries: (path: string, entryIds: string[]) => void;
}

export function useFolderTree(): UseFolderTreeReturn {
  const [state, dispatch] = useReducer(
    folderTreeReducer,
    INITIAL_FOLDER_TREE_STATE,
  );
  const stateRef = useRef(state);

  // Keep ref synchronised with latest state after each render.
  useEffect(() => {
    stateRef.current = state;
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
        }
      });
    })();

    return () => {
      unlisten?.();
    };
  }, []);

  const enumeratePath = useCallback(async (path: string) => {
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
      const sessionId = await startEnumeration(path);
      // Register mapping before dispatching START_ENUMERATING so that
      // any chunks buffered during the await are applied to the correct
      // folder state after the reducer resets entries.
      SESSION_PATHS[sessionId] = path;
      dispatch({ type: "START_ENUMERATING", path, sessionId });
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

  useEffect(() => {
    let active = true;
    try {
      void listBrowserRoots()
        .then((roots) => {
          if (!active) return;
          dispatch({ type: "ROOTS_LOADED", roots });
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

  const selectFolder = useCallback((path: string) => {
    dispatch({ type: "SELECT_FOLDER", path });
  }, []);

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

  return {
    state,
    toggleExpand,
    selectFolder,
    navigateUp,
    clearError,
    removeEntries,
  };
}
