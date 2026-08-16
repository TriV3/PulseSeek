import { render, screen, fireEvent, within } from "@testing-library/react";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { FolderTree } from "./FolderTree";
import type { FolderTreeState } from "./folderTreeTypes";
import { INITIAL_FOLDER_TREE_STATE } from "./folderTreeTypes";

// ── Test helpers ───────────────────────────────────────────────────────

function createMockFolderTreeProps(overrides?: Partial<FolderTreeState>) {
  const state: FolderTreeState = {
    ...INITIAL_FOLDER_TREE_STATE,
    ...overrides,
  };

  return {
    state,
    toggleExpand: vi.fn(),
    selectFolder: vi.fn(),
    navigateUp: vi.fn(),
    clearError: vi.fn(),
  };
}

function folder(name: string) {
  return {
    id: `/test/music/${name}`,
    name,
    kind: "folder" as const,
  };
}

// ── Tests ──────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks();
});

describe("FolderTree — initial state", () => {
  it("shows a loading state without an Open Folder button", () => {
    render(<FolderTree {...createMockFolderTreeProps()} />);
    expect(screen.getByText("Loading disks…")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Open Folder" })).toBeNull();
  });

  it("renders a tree with accessible label", () => {
    render(<FolderTree {...createMockFolderTreeProps()} />);
    expect(
      screen.getByRole("tree", { name: "Folder browser" }),
    ).toBeInTheDocument();
  });
});

describe("FolderTree — volume icons", () => {
  it("renders home, physical, and network roots with different semantic icons", () => {
    const props = createMockFolderTreeProps({
      rootPath: "computer://",
      selectedPath: "computer://",
      folders: {
        "computer://": {
          expanded: true,
          children: [
            {
              id: "/Users/test",
              name: "Home",
              kind: "folder",
              rootKind: "home",
            },
            {
              id: "/Volumes/Portable",
              name: "Portable",
              kind: "folder",
              rootKind: "physical",
            },
            {
              id: "/Volumes/Studio",
              name: "Studio",
              kind: "folder",
              rootKind: "network",
            },
          ],
          isLoading: false,
          hasLoaded: true,
          hasSubfolders: true,
          error: null,
          recursive: false,
        },
        "/Volumes/Portable": {
          expanded: false,
          children: [],
          isLoading: false,
          error: null,
          recursive: false,
        },
        "/Users/test": {
          expanded: false,
          children: [],
          isLoading: false,
          error: null,
          recursive: false,
        },
        "/Volumes/Studio": {
          expanded: false,
          children: [],
          isLoading: false,
          error: null,
          recursive: false,
        },
      },
    });

    render(<FolderTree {...props} />);

    expect(
      screen
        .getByText("Home")
        .parentElement?.querySelector("[data-folder-icon='home']"),
    ).not.toBeNull();
    expect(
      screen
        .getByText("Portable")
        .parentElement?.querySelector("[data-folder-icon='physical']"),
    ).not.toBeNull();
    expect(
      screen
        .getByText("Studio")
        .parentElement?.querySelector("[data-folder-icon='network']"),
    ).not.toBeNull();
  });
});

describe("FolderTree — browser sections", () => {
  it("shows drives and libraries and collapses each section independently", () => {
    const props = createMockFolderTreeProps({
      rootPath: "computer://",
      selectedPath: "computer://",
      libraries: [
        {
          id: "/Users/test/Music",
          name: "Music",
          kind: "folder",
          libraryKind: "music",
        },
      ],
      folders: {
        "computer://": {
          expanded: true,
          children: [
            { id: "/", name: "System", kind: "folder", rootKind: "system" },
          ],
          isLoading: false,
          hasLoaded: true,
          error: null,
          recursive: false,
        },
        "/": {
          expanded: false,
          children: [],
          isLoading: false,
          error: null,
          recursive: false,
        },
        "/Users/test/Music": {
          expanded: false,
          children: [],
          isLoading: false,
          error: null,
          recursive: false,
        },
      },
    });

    render(<FolderTree {...props} />);
    const drives = screen.getByRole("button", { name: "Drives" });
    const libraries = screen.getByRole("button", { name: "Libraries" });
    expect(screen.getByText("System")).toBeVisible();
    expect(screen.getByText("Music")).toBeVisible();
    expect(
      screen
        .getByText("Music")
        .parentElement?.querySelector("[data-folder-icon='music']"),
    ).not.toBeNull();

    fireEvent.click(drives);
    expect(drives).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByText("System")).toBeNull();
    expect(screen.getByText("Music")).toBeVisible();

    fireEvent.click(libraries);
    expect(screen.queryByText("Music")).toBeNull();
  });
});

describe("FolderTree — bookmarks", () => {
  it("marks every bookmarked folder independently of the current selection", () => {
    const props = createMockFolderTreeProps({
      rootPath: "computer://",
      selectedPath: "computer://",
      folders: {
        "computer://": {
          expanded: true,
          children: [
            {
              id: "/music",
              name: "Music",
              kind: "folder",
              rootKind: "physical",
            },
          ],
          isLoading: false,
          hasLoaded: true,
          error: null,
          recursive: false,
        },
        "/music": {
          expanded: false,
          children: [],
          isLoading: false,
          hasLoaded: true,
          error: null,
          recursive: false,
        },
      },
    });

    render(
      <FolderTree {...props} isPathBookmarked={(path) => path === "/music"} />,
    );

    expect(screen.getByText("Music").closest("[role='treeitem']")).toHaveClass(
      "folder-node--bookmarked",
    );
  });

  it("replaces the native menu with folder actions", () => {
    const props = createMockFolderTreeProps({
      rootPath: "/music",
      selectedPath: "/music",
      folders: {
        "/music": {
          expanded: false,
          children: [],
          isLoading: false,
          hasLoaded: true,
          hasSubfolders: true,
          error: null,
          recursive: false,
        },
      },
    });
    const toggleBookmark = vi.fn();
    render(<FolderTree {...props} toggleBookmark={toggleBookmark} />);

    const folder = screen.getByRole("treeitem");
    expect(fireEvent.contextMenu(folder, { clientX: 20, clientY: 30 })).toBe(
      false,
    );
    const menu = screen.getByRole("menu", {
      name: "Folder actions for music",
    });
    expect(within(menu).getByRole("menuitem", { name: "Open" })).toBeVisible();
    expect(
      within(menu).getByRole("menuitem", { name: "Expand folder" }),
    ).toBeVisible();

    fireEvent.click(
      within(menu).getByRole("menuitem", { name: "Bookmark folder" }),
    );
    expect(toggleBookmark).toHaveBeenCalledWith("/music");
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("offers to remove an existing folder bookmark", () => {
    const props = createMockFolderTreeProps({
      rootPath: "/music",
      selectedPath: "/music",
      folders: {
        "/music": {
          expanded: false,
          children: [],
          isLoading: false,
          hasLoaded: true,
          error: null,
          recursive: false,
        },
      },
    });
    render(
      <FolderTree
        {...props}
        isPathBookmarked={(path) => path === "/music"}
        toggleBookmark={vi.fn()}
      />,
    );

    fireEvent.contextMenu(screen.getByRole("treeitem"));
    expect(
      screen.getByRole("menuitem", { name: "Remove folder bookmark" }),
    ).toBeVisible();
  });
});

describe("FolderTree — selected path display", () => {
  it("shows the root path when a folder is selected", () => {
    const props = createMockFolderTreeProps({
      rootPath: "/test/music",
      selectedPath: "/test/music",
      folders: {
        "/test/music": {
          expanded: true,
          children: [],
          isLoading: false,
          error: null,
          recursive: false,
        },
      },
    });

    render(<FolderTree {...props} />);
    expect(screen.getByText("/test/music")).toBeInTheDocument();
  });
});

describe("FolderTree — selected folder visibility", () => {
  it("scrolls the selected folder into the browser viewport", () => {
    const scrollIntoView = vi.fn();
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      value: scrollIntoView,
    });
    const props = createMockFolderTreeProps({
      selectedPath: "/test/music",
      folders: {
        "/test": {
          expanded: true,
          children: [{ id: "/test/music", name: "music", kind: "folder" }],
          isLoading: false,
          error: null,
          recursive: false,
        },
        "/test/music": {
          expanded: false,
          children: [],
          isLoading: false,
          error: null,
          recursive: false,
        },
      },
      rootPath: "/test",
    });

    render(<FolderTree {...props} />);

    expect(scrollIntoView).toHaveBeenCalledWith({ block: "nearest" });
  });
});

describe("FolderTree — selected audio path", () => {
  it("highlights every ancestor folder without highlighting a prefix sibling", () => {
    const props = createMockFolderTreeProps({
      rootPath: "/test",
      selectedPath: "/test/one/two",
      folders: {
        "/test": {
          expanded: true,
          children: [
            { id: "/test/one", name: "one", kind: "folder" },
            { id: "/test/ones", name: "ones", kind: "folder" },
          ],
          isLoading: false,
          error: null,
          recursive: false,
        },
        "/test/one": {
          expanded: true,
          children: [
            {
              id: "/test/one/two",
              name: "two",
              kind: "folder",
            },
          ],
          isLoading: false,
          error: null,
          recursive: false,
        },
        "/test/one/two": {
          expanded: false,
          children: [],
          isLoading: false,
          error: null,
          recursive: false,
        },
        "/test/ones": {
          expanded: false,
          children: [],
          isLoading: false,
          error: null,
          recursive: false,
        },
      },
    });

    render(<FolderTree {...props} activeFilePath="/test/one/two/song.flac" />);

    for (const name of ["test", "one", "two"]) {
      expect(screen.getByText(name).closest(".folder-node")).toHaveClass(
        "folder-node--audio-path",
      );
    }
    expect(screen.getByText("ones").closest(".folder-node")).not.toHaveClass(
      "folder-node--audio-path",
    );
  });
});

describe("FolderTree — indentation", () => {
  it("uses one fixed indentation step per nested list", () => {
    const props = createMockFolderTreeProps({
      rootPath: "/test",
      selectedPath: "/test/one/two",
      folders: {
        "/test": {
          expanded: true,
          children: [{ id: "/test/one", name: "one", kind: "folder" }],
          isLoading: false,
          error: null,
          recursive: false,
        },
        "/test/one": {
          expanded: true,
          children: [{ id: "/test/one/two", name: "two", kind: "folder" }],
          isLoading: false,
          error: null,
          recursive: false,
        },
        "/test/one/two": {
          expanded: false,
          children: [],
          isLoading: false,
          error: null,
          recursive: false,
        },
      },
    });

    render(<FolderTree {...props} />);

    for (const item of screen.getAllByRole("treeitem")) {
      expect(item).not.toHaveStyle({ paddingLeft: "24px" });
      expect(item).not.toHaveStyle({ paddingLeft: "40px" });
    }
  });
});

describe("FolderTree — audio-only folders", () => {
  it("does not label a folder empty when it contains playable files", () => {
    const props = createMockFolderTreeProps({
      rootPath: "/music",
      selectedPath: "/music",
      folders: {
        "/music": {
          expanded: true,
          children: [],
          isLoading: false,
          error: null,
          recursive: false,
        },
      },
      playableEntries: {
        "/music": [
          { id: "/music/song.wav", name: "song.wav", kind: "playable" },
        ],
      },
    });

    render(<FolderTree {...props} />);

    expect(screen.queryByText("(empty)")).not.toBeInTheDocument();
  });
});

describe("FolderTree — folder expansion", () => {
  it("does not render an expand control for a leaf while its files load", () => {
    const props = createMockFolderTreeProps({
      rootPath: "/test/empty",
      selectedPath: "/test/empty",
      folders: {
        "/test/empty": {
          expanded: true,
          children: [],
          isLoading: true,
          hasLoaded: false,
          hasSubfolders: false,
          error: null,
          recursive: false,
        },
      },
    });

    render(<FolderTree {...props} />);

    expect(
      screen.queryByRole("button", { name: /^(expand|collapse) folder$/i }),
    ).toBeNull();
    expect(screen.getByRole("treeitem")).not.toHaveAttribute("aria-expanded");
    expect(screen.queryByText("(empty)")).toBeNull();
  });

  it("shows subfolder children", () => {
    const props = createMockFolderTreeProps({
      rootPath: "/test/music",
      selectedPath: "/test/music",
      folders: {
        "/test/music": {
          expanded: true,
          children: [folder("Sub1"), folder("Sub2")],
          isLoading: false,
          error: null,
          recursive: false,
        },
      },
    });

    render(<FolderTree {...props} />);
    expect(screen.getByText("Sub1")).toBeInTheDocument();
    expect(screen.getByText("Sub2")).toBeInTheDocument();
  });

  it("calls toggleExpand when the arrow is clicked", () => {
    const props = createMockFolderTreeProps({
      rootPath: "/test/music",
      selectedPath: "/test/music",
      folders: {
        "/test/music": {
          expanded: true,
          children: [folder("Sub1")],
          isLoading: false,
          error: null,
          recursive: false,
        },
      },
    });

    render(<FolderTree {...props} />);
    const collapseBtn = screen.getByRole("button", { name: "Collapse folder" });
    fireEvent.click(collapseBtn);

    expect(props.toggleExpand).toHaveBeenCalledWith("/test/music");
  });

  it("renders loaded descendant state using backend entry ids", () => {
    const props = createMockFolderTreeProps({
      rootPath: "/test/music",
      selectedPath: "/test/music/Sub1",
      folders: {
        "/test/music": {
          expanded: true,
          children: [folder("Sub1")],
          isLoading: false,
          error: null,
          recursive: false,
        },
        "/test/music/Sub1": {
          expanded: true,
          children: [
            {
              id: "/test/music/Sub1/Nested",
              name: "Nested",
              kind: "folder",
            },
          ],
          isLoading: false,
          error: null,
          recursive: false,
        },
      },
    });

    render(<FolderTree {...props} />);

    expect(screen.getByText("Nested")).toBeInTheDocument();
    expect(
      screen.getByText("Sub1").closest('[role="treeitem"]'),
    ).toHaveAttribute("aria-expanded", "true");
  });
});

describe("FolderTree — selection", () => {
  it("calls selectFolder when a folder name is clicked", () => {
    const props = createMockFolderTreeProps({
      rootPath: "/test/music",
      selectedPath: "/test/music",
      folders: {
        "/test/music": {
          expanded: true,
          children: [folder("Sub1")],
          isLoading: false,
          error: null,
          recursive: false,
        },
      },
    });

    render(<FolderTree {...props} />);
    fireEvent.click(screen.getByText("Sub1"));

    expect(props.selectFolder).toHaveBeenCalledWith("/test/music/Sub1");
    expect(props.toggleExpand).toHaveBeenCalledWith("/test/music/Sub1");
  });
});

describe("FolderTree — keyboard navigation", () => {
  it("calls selectFolder on ArrowDown", () => {
    const props = createMockFolderTreeProps({
      rootPath: "/test/music",
      selectedPath: "/test/music",
      folders: {
        "/test/music": {
          expanded: true,
          children: [folder("Sub1"), folder("Sub2")],
          isLoading: false,
          error: null,
          recursive: false,
        },
        "/test/music/Sub1": {
          expanded: false,
          children: [],
          isLoading: false,
          error: null,
          recursive: false,
        },
        "/test/music/Sub2": {
          expanded: false,
          children: [],
          isLoading: false,
          error: null,
          recursive: false,
        },
      },
    });

    render(<FolderTree {...props} />);
    const tree = screen.getByRole("tree");
    tree.focus();
    fireEvent.keyDown(tree, { key: "ArrowDown" });

    // The selected path exists so selectFolder can be called.
    expect(props.selectFolder).toHaveBeenCalled();
  });
});

describe("FolderTree — error state", () => {
  it("displays an error banner when errorMessage is set", () => {
    const props = createMockFolderTreeProps({
      status: "error",
      errorMessage: "Cannot read folder.",
    });

    render(<FolderTree {...props} />);
    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(screen.getByText("Cannot read folder.")).toBeInTheDocument();
  });

  it("calls clearError when dismiss button is clicked", () => {
    const props = createMockFolderTreeProps({
      status: "error",
      errorMessage: "Cannot read folder.",
    });

    render(<FolderTree {...props} />);
    fireEvent.click(screen.getByLabelText("Dismiss error"));

    expect(props.clearError).toHaveBeenCalledOnce();
  });
});

describe("FolderTree — Go Up button", () => {
  it("renders a Go Up button when a folder is selected", () => {
    const props = createMockFolderTreeProps({
      rootPath: "/test/music",
      selectedPath: "/test/music",
      folders: {
        "/test/music": {
          expanded: true,
          children: [],
          isLoading: false,
          error: null,
          recursive: false,
        },
      },
    });

    render(<FolderTree {...props} />);
    expect(
      screen.getByRole("button", { name: "Go to parent folder" }),
    ).toBeInTheDocument();
  });
});
