/** Entry kind as returned by the Rust backend. */
export type BrowserEntryKind =
  "folder" | "playable" | "unsupported" | "inaccessible";

export type BrowserRootKind = "system" | "home" | "physical" | "network";
export type BrowserLibraryKind =
  "documents" | "music" | "pictures" | "videos" | "downloads";

/** A single entry from a folder enumeration chunk. */
export interface BrowserEntry {
  id: string;
  name: string;
  kind: BrowserEntryKind;
  /** Present only for filesystem roots supplied by the operating system. */
  rootKind?: BrowserRootKind;
  libraryKind?: BrowserLibraryKind;
  has_subfolders?: boolean | null;
  metadata?: PlayableFileMetadata | null;
}

/** Available filesystem and audio-stream metadata for a playable entry. */
export interface PlayableFileMetadata {
  duration_ms: number | null;
  size_bytes: number | null;
  modified_at_ms: number | null;
  channels: number | null;
  sample_rate: number | null;
  bit_depth: number | null;
  codec: string | null;
}

/** Per-folder state tracked in the tree. */
export interface FolderState {
  expanded: boolean;
  /** Subfolder entries inside this folder. */
  children: BrowserEntry[];
  /** Whether a folder enumeration is in progress for this path. */
  isLoading: boolean;
  /** Whether this folder has completed at least one enumeration. */
  hasLoaded?: boolean;
  /** Whether a shallow parent scan found at least one direct subfolder. */
  hasSubfolders?: boolean | null;
  /** Error message when enumeration failed, or null. */
  error: string | null;
  /** Whether the folder was enumerated in recursive file-view mode. */
  recursive: boolean;
}

/** Top-level tree state managed by the reducer. */
export interface FolderTreeState {
  /** The root folder the user picked. Null before any pick. */
  rootPath: string | null;
  /** Map of folder path → folder state. */
  folders: Record<string, FolderState>;
  libraries: BrowserEntry[];
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
  | {
      type: "ROOTS_LOADED";
      roots: Array<{ path: string; name: string; kind: BrowserRootKind }>;
      libraries: Array<{
        path: string;
        name: string;
        kind: BrowserLibraryKind;
      }>;
    }
  | { type: "ROOTS_ERROR"; message: string }
  | { type: "START_PICKING" }
  | { type: "FOLDER_PICKED"; path: string }
  | { type: "PICK_CANCELLED" }
  | { type: "PICK_ERROR"; message: string }
  | {
      type: "START_ENUMERATING";
      path: string;
      sessionId: string;
      recursive: boolean;
    }
  | {
      type: "ENUMERATION_CHUNK";
      path: string;
      entries: BrowserEntry[];
      done: boolean;
      foldersDone?: boolean;
    }
  | { type: "ENUMERATION_ERROR"; path: string; message: string }
  | { type: "TOGGLE_EXPAND"; path: string }
  | { type: "SELECT_FOLDER"; path: string }
  | {
      type: "RESTORE_CONTEXT";
      selectedPath: string;
      expandedPaths: string[];
    }
  | { type: "NAVIGATE_UP" }
  | { type: "CLEAR_ERROR" }
  | { type: "REMOVE_ENTRIES"; path: string; entryIds: string[] }
  | {
      type: "RENAME_ENTRY";
      path: string;
      oldId: string;
      newId: string;
      newName: string;
    };

export const INITIAL_FOLDER_TREE_STATE: FolderTreeState = {
  rootPath: null,
  folders: {},
  libraries: [],
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
  if (lastSlash < 0) return null;
  if (lastSlash === 0) return normalized.length > 1 ? "/" : null;
  return normalized.substring(0, lastSlash);
}

/**
 * Returns a file's path relative to the folder it was listed under, falling
 * back to the bare file name when the entry is outside that folder. Recursive
 * file views use this so files from different subfolders stay distinct even
 * when they share a name.
 */
export function relativeEntryPath(
  entryId: string,
  selectedPath: string,
): string {
  if (selectedPath && entryId.startsWith(`${selectedPath}/`)) {
    return entryId.slice(selectedPath.length + 1);
  }
  return entryId.split("/").filter(Boolean).at(-1) ?? entryId;
}

/**
 * Collects every folder currently visible in the browser tree, deduplicated by
 * path. Used as the search base for folder search so a query is not limited to
 * the children of the currently selected folder (FR-LS-004).
 */
export function collectFolderEntries(
  folders: Record<string, FolderState>,
): BrowserEntry[] {
  const byPath = new Map<string, BrowserEntry>();
  for (const folder of Object.values(folders)) {
    for (const entry of folder.children) {
      if (entry.kind === "folder" && !byPath.has(entry.id)) {
        byPath.set(entry.id, entry);
      }
    }
  }
  return [...byPath.values()];
}
