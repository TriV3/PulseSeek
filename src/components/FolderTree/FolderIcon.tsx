import type { BrowserLibraryKind, BrowserRootKind } from "./folderTreeTypes";

export type FolderIconKind =
  BrowserRootKind | BrowserLibraryKind | "computer" | "folder";

interface FolderIconProps {
  kind: FolderIconKind;
  expanded: boolean;
}

export function FolderIcon({ kind, expanded }: FolderIconProps) {
  const common = {
    className: `folder-icon folder-icon--${kind}`,
    "data-folder-icon": kind,
    "aria-hidden": true,
    viewBox: "0 0 20 20",
  } as const;

  if (kind === "computer") {
    return (
      <svg {...common}>
        <rect x="2.5" y="3" width="15" height="10.5" rx="1.5" />
        <path d="M7 17h6M10 13.5V17" />
      </svg>
    );
  }

  if (kind === "home") {
    return (
      <svg {...common}>
        <path d="m3 9 7-6 7 6v8H5V9" />
        <path d="M8 17v-5h4v5" />
      </svg>
    );
  }

  if (kind === "system" || kind === "physical") {
    return (
      <svg {...common}>
        <rect x="3" y="2.5" width="14" height="15" rx="2" />
        <path d="M5.5 12.5h9M6 6.5h8" />
        <circle cx="13.5" cy="15" r="0.8" className="folder-icon-fill" />
      </svg>
    );
  }

  if (kind === "network") {
    return (
      <svg {...common}>
        <rect x="3" y="2.5" width="14" height="10.5" rx="2" />
        <path d="M6 6.5h8M10 13v2M5.5 17h9M6 15v2M14 15v2" />
        <circle cx="10" cy="15" r="0.8" className="folder-icon-fill" />
      </svg>
    );
  }

  if (
    ["documents", "music", "pictures", "videos", "downloads"].includes(kind)
  ) {
    return (
      <svg {...common}>
        <path d="M3 4.5h5l1.5 1.7H17v9.3H3z" />
        {kind === "music" && (
          <path d="M8 13V8.5l5-1V12M6.5 14.2a1.5 1 0 1 0 0-2 1.5 1 0 0 0 2M11.5 13.2a1.5 1 0 1 0 0-2 1.5 1 0 0 0 2" />
        )}
        {kind === "pictures" && <path d="m5 13 3-3 2 2 2-2 3 3M6.5 8.5h.1" />}
        {kind === "videos" && <path d="m8 9 4 2-4 2z" />}
        {kind === "downloads" && <path d="M10 7v5m-2-2 2 2 2-2M7 14h6" />}
        {kind === "documents" && <path d="M7 8h6M7 10.5h6M7 13h4" />}
      </svg>
    );
  }

  return (
    <svg {...common} data-expanded={expanded ? "true" : "false"}>
      <path d="M2.5 5.5h5l1.6 1.7h8.4v8.3H2.5z" />
      {expanded && <path d="M2.5 8h15l-1.8 7.5H4.3z" />}
    </svg>
  );
}
