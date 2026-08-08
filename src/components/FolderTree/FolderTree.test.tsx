import { render, screen, fireEvent } from "@testing-library/react";
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
