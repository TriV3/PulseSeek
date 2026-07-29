/** Entry kind as returned by the Rust backend. */
export type BrowserEntryKind =
  "folder" | "playable" | "unsupported" | "inaccessible";

/** A single entry from a folder enumeration chunk. */
export interface BrowserEntry {
  id: string;
  name: string;
  kind: BrowserEntryKind;
}

/** Per-folder state tracked in the tree. */
export interface FolderState {
  expanded: boolean;
  /** Subfolder entries inside this folder. */
  children: BrowserEntry[];
  /** Whether a folder enumeration is in progress for this path. */
  isLoading: boolean;
  /** Error message when enumeration failed, or null. */
  error: string | null;
}

/** Top-level tree state managed by the reducer. */
export interface FolderTreeState {
  /** The root folder the user picked. Null before any pick. */
  rootPath: string | null;
  /** Map of folder path → folder state. */
  folders: Record<string, FolderState>;
  /** Playable file entries per folder path (for file list). */
  playableEntries: Record<string, BrowserEntry[]>;
  /** Full path of the currently selected (highlighted) folder. */
  selectedPath: string | null;
  /** Current enumeration session id, or null if idle. */
  activeSessionId: string | null;
  /** High-level status of the tree. */
  status: "idle" | "picking" | "loading" | "ready" | "error";
  /** User-facing error message. */
  errorMessage: string | null;
}

export type FolderTreeAction =
  | { type: "START_PICKING" }
  | { type: "FOLDER_PICKED"; path: string }
  | { type: "PICK_CANCELLED" }
  | { type: "PICK_ERROR"; message: string }
  | { type: "START_ENUMERATING"; path: string; sessionId: string }
  | {
      type: "ENUMERATION_CHUNK";
      path: string;
      entries: BrowserEntry[];
      done: boolean;
    }
  | { type: "ENUMERATION_ERROR"; path: string; message: string }
  | { type: "TOGGLE_EXPAND"; path: string }
  | { type: "SELECT_FOLDER"; path: string }
  | { type: "NAVIGATE_UP" }
  | { type: "CLEAR_ERROR" };

export const INITIAL_FOLDER_TREE_STATE: FolderTreeState = {
  rootPath: null,
  folders: {},
  playableEntries: {},
  selectedPath: null,
  activeSessionId: null,
  status: "idle",
  errorMessage: null,
};

/** Returns the parent directory path, or null at the filesystem root. */
export function getParentPath(path: string): string | null {
  const normalized = path.replace(/\/+$/, "");
  const lastSlash = normalized.lastIndexOf("/");
  if (lastSlash <= 0) return null;
  return normalized.substring(0, lastSlash);
}
