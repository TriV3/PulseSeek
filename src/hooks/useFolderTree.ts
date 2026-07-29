import { useCallback, useEffect, useReducer, useRef } from "react";
import {
  cancelEnumeration,
  pickFolder,
  startEnumeration,
} from "../api/commandEnvelope";
import { onFolderChunk } from "../api/playbackEvents";
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
      const newPlayable = action.entries.filter((e) => e.kind === "playable");

      // Deduplicate folder names.
      const childrenById = new Map(
        current.children.map((entry) => [entry.id, entry]),
      );
      for (const entry of newFolders) {
        childrenById.set(entry.id, entry);
      }

      return {
        ...state,
        folders: {
          ...state.folders,
          [action.path]: {
            ...current,
            children: [...childrenById.values()],
            isLoading: !action.done,
            expanded: true,
          },
        },
        playableEntries: {
          ...state.playableEntries,
          [action.path]: [
            ...(state.playableEntries[action.path] ?? []),
            ...newPlayable,
          ],
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

    default:
      return state;
  }
}

// ── Hook ─────────────────────────────────────────────────────────────────

const SESSION_PATHS: Record<string, string> = {};

export interface UseFolderTreeReturn {
  state: FolderTreeState;
  openFolder: () => Promise<void>;
  toggleExpand: (path: string) => void;
  selectFolder: (path: string) => void;
  navigateUp: () => void;
  clearError: () => void;
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
        if (!path) return; // stale / unknown session
        dispatch({
          type: "ENUMERATION_CHUNK",
          path,
          entries: payload.entries,
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
      SESSION_PATHS[sessionId] = path;
      dispatch({ type: "START_ENUMERATING", path, sessionId });
    } catch (err: unknown) {
      const message =
        err instanceof Error ? err.message : "Failed to enumerate folder.";
      dispatch({ type: "ENUMERATION_ERROR", path, message });
    }
  }, []);

  const openFolder = useCallback(async () => {
    dispatch({ type: "START_PICKING" });
    try {
      const path = await pickFolder();
      if (path === null) {
        dispatch({ type: "PICK_CANCELLED" });
        return;
      }
      dispatch({ type: "FOLDER_PICKED", path });
      // Enumerate the root folder immediately.
      await enumeratePath(path);
    } catch (err: unknown) {
      const message =
        err instanceof Error ? err.message : "Failed to pick folder.";
      dispatch({ type: "PICK_ERROR", message });
    }
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

  return {
    state,
    openFolder,
    toggleExpand,
    selectFolder,
    navigateUp,
    clearError,
  };
}
