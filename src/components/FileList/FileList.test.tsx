import {
  render,
  screen,
  fireEvent,
  waitFor,
  within,
  act,
} from "@testing-library/react";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { useMemo, useState } from "react";
import { FileList } from "./FileList";
import type { BrowserEntry } from "../FolderTree/folderTreeTypes";
import { sortFileEntries, type FileSort } from "./fileSort";
import { filterFileEntries } from "./fileSearch";
import {
  FORMAT_OPTIONS,
  filterByFormat,
  type AudioFileFormat,
} from "./fileFilter";
import {
  filterByMark,
  type MarkFilter,
  type SessionMark,
  type SessionMarks,
} from "./sessionMarks";
import { useSessionMarks } from "../../hooks/useSessionMarks";

const mockMoveToTrash = vi.hoisted(() => vi.fn());
const mockRenameFile = vi.hoisted(() => vi.fn());
const mockStartMoveFiles = vi.hoisted(() => vi.fn());
const mockCancelMoveFiles = vi.hoisted(() => vi.fn());
const mockPickFolder = vi.hoisted(() => vi.fn());
const mockRevealFile = vi.hoisted(() => vi.fn());
const mockOpenWith = vi.hoisted(() => vi.fn());
const mockDragOut = vi.hoisted(() => vi.fn());

vi.mock("../../api/commandEnvelope", () => ({
  moveToTrash: mockMoveToTrash,
  renameFile: mockRenameFile,
  startMoveFiles: mockStartMoveFiles,
  cancelMoveFiles: mockCancelMoveFiles,
  pickFolder: mockPickFolder,
  revealFile: mockRevealFile,
  openWith: mockOpenWith,
  dragOut: mockDragOut,
}));

type EventHandler = (event: { payload: unknown }) => void;
const eventHandlers = new Map<string, EventHandler>();

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (event: string, handler: EventHandler) => {
    eventHandlers.set(event, handler);
    return () => {
      eventHandlers.delete(event);
    };
  }),
}));

// Mock TanStack Virtual to render all items synchronously (jsdom has no
// layout, so the virtualizer would measure 0 height and render nothing).
vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: vi.fn(
    (opts: { count: number; estimateSize: () => number }) => {
      const items = Array.from(
        { length: Math.min(opts.count, 20) },
        (_, i) => ({
          key: i,
          index: i,
          start: i * opts.estimateSize(),
          size: opts.estimateSize(),
        }),
      );
      return {
        getVirtualItems: () => items,
        getTotalSize: () => items.length * opts.estimateSize(),
        scrollToIndex: vi.fn(),
      };
    },
  ),
}));

const sampleEntries: BrowserEntry[] = [
  { id: "song1.mp3", name: "song1.mp3", kind: "playable" },
  { id: "song2.wav", name: "song2.wav", kind: "playable" },
  { id: "song3.flac", name: "song3.flac", kind: "playable" },
];

describe("FileList — no folder selected", () => {
  it("shows a placeholder when no folder is selected", () => {
    render(
      <FileList
        entries={[]}
        selectedPath={null}
        isLoading={false}
        error={null}
      />,
    );

    expect(
      screen.getByText("Select a folder to browse files."),
    ).toBeInTheDocument();
  });

  it("has an accessible region with label", () => {
    render(
      <FileList
        entries={[]}
        selectedPath={null}
        isLoading={false}
        error={null}
      />,
    );

    expect(
      screen.getByRole("region", { name: "File list" }),
    ).toBeInTheDocument();
  });
});

describe("FileList — error state", () => {
  it("shows the error message", () => {
    render(
      <FileList
        entries={[]}
        selectedPath="/test/music"
        isLoading={false}
        error="Permission denied."
      />,
    );

    expect(screen.getByText("Permission denied.")).toBeInTheDocument();
  });
});

describe("FileList — loading state", () => {
  it("shows a loading indicator when loading with no entries", () => {
    render(
      <FileList
        entries={[]}
        selectedPath="/test/music"
        isLoading={true}
        error={null}
      />,
    );

    expect(screen.getByText("Loading\u2026")).toBeInTheDocument();
  });

  it("shows entries when loading but entries exist (incremental)", () => {
    render(
      <FileList
        entries={[sampleEntries[0]]}
        selectedPath="/test/music"
        isLoading={true}
        error={null}
      />,
    );

    expect(screen.getByText("song1.mp3")).toBeInTheDocument();
  });
});

describe("FileList — empty folder", () => {
  it("shows empty placeholder", () => {
    render(
      <FileList
        entries={[]}
        selectedPath="/test/music"
        isLoading={false}
        error={null}
      />,
    );

    expect(screen.getByText("(no playable files)")).toBeInTheDocument();
  });
});

describe("FileList — entries display", () => {
  it("renders all playable file names", () => {
    render(
      <FileList
        entries={sampleEntries}
        selectedPath="/test/music"
        isLoading={false}
        error={null}
      />,
    );

    expect(screen.getByText("song1.mp3")).toBeInTheDocument();
    expect(screen.getByText("song2.wav")).toBeInTheDocument();
    expect(screen.getByText("song3.flac")).toBeInTheDocument();
  });

  it("has a Name column header", () => {
    render(
      <FileList
        entries={sampleEntries}
        selectedPath="/test/music"
        isLoading={false}
        error={null}
      />,
    );

    expect(screen.getByText("Name")).toBeInTheDocument();
  });

  it("shows formatted metadata columns", () => {
    render(
      <FileList
        entries={[
          {
            id: "song.wav",
            name: "song.wav",
            kind: "playable",
            metadata: {
              duration_ms: 61_000,
              size_bytes: 1_572_864,
              modified_at_ms: Date.UTC(2026, 0, 2, 3, 4),
              channels: 2,
              sample_rate: 44_100,
              bit_depth: 16,
              codec: "PCM",
            },
          },
        ]}
        selectedPath="/test/music"
        isLoading={false}
        error={null}
      />,
    );

    expect(
      screen.getByRole("columnheader", { name: "Duration" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("columnheader", { name: "Size" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("columnheader", { name: "Modified" }),
    ).toBeInTheDocument();
    expect(screen.getByText("1:01")).toBeInTheDocument();
    const expectedSize = `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 }).format(1.5)} MiB`;
    const expectedSampleRate = `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 3 }).format(44.1)} kHz`;
    expect(screen.getByText(expectedSize)).toBeInTheDocument();
    expect(screen.getByText("Stereo")).toBeInTheDocument();
    expect(screen.getByText(expectedSampleRate)).toBeInTheDocument();
    expect(screen.getByText("16-bit")).toBeInTheDocument();
    expect(screen.getByText("PCM")).toBeInTheDocument();
    expect(
      screen.getByRole("grid", { name: "Playable files" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("row", { name: /song\.wav/ })).toBeInTheDocument();
  });

  it("preserves common fractional sample rates", () => {
    render(
      <FileList
        entries={[
          {
            id: "song.wav",
            name: "song.wav",
            kind: "playable",
            metadata: {
              duration_ms: null,
              size_bytes: null,
              modified_at_ms: null,
              channels: null,
              sample_rate: 22_050,
              bit_depth: null,
              codec: null,
            },
          },
        ]}
        selectedPath="/test/music"
        isLoading={false}
        error={null}
      />,
    );

    const expectedRate = `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 3 }).format(22.05)} kHz`;
    expect(screen.getByText(expectedRate)).toBeInTheDocument();
  });

  it("keeps partially loaded playable rows and marks missing values", () => {
    render(
      <FileList
        entries={[
          {
            id: "song.mp3",
            name: "song.mp3",
            kind: "playable",
            metadata: {
              duration_ms: 61_000,
              size_bytes: null,
              modified_at_ms: null,
              channels: 2,
              sample_rate: 44_100,
              bit_depth: null,
              codec: "MP3",
            },
          },
        ]}
        selectedPath="/test/music"
        isLoading={false}
        error={null}
      />,
    );

    expect(screen.getByText("song.mp3")).toBeInTheDocument();
    expect(screen.getAllByText("—")).toHaveLength(3);
  });

  it("shows loading placeholders while playable metadata is unavailable", () => {
    render(
      <FileList
        entries={[sampleEntries[0]]}
        selectedPath="/test/music"
        isLoading={true}
        error={null}
      />,
    );

    expect(screen.getByText("song1.mp3")).toBeInTheDocument();
    expect(screen.getAllByLabelText("Metadata loading")).toHaveLength(7);
  });
});

describe("FileList — file selection", () => {
  it("calls onFileSelect when a row is clicked", () => {
    const onSelect = vi.fn();

    render(
      <FileList
        entries={[sampleEntries[0]]}
        selectedPath="/test/music"
        isLoading={false}
        error={null}
        onFileSelect={onSelect}
      />,
    );

    fireEvent.click(screen.getByText("song1.mp3"));

    expect(onSelect).toHaveBeenCalledOnce();
    expect(onSelect).toHaveBeenCalledWith(sampleEntries[0]);
    expect(screen.getByRole("row", { name: /song1\.mp3/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  it("selects a row with the keyboard", () => {
    const onSelect = vi.fn();

    render(
      <FileList
        entries={[sampleEntries[0]]}
        selectedPath="/test/music"
        isLoading={false}
        error={null}
        onFileSelect={onSelect}
      />,
    );

    const row = screen.getByRole("row", { name: /song1\.mp3/ });
    fireEvent.keyDown(row, { key: "Enter" });

    expect(onSelect).toHaveBeenCalledWith(sampleEntries[0]);
    expect(row).toHaveAttribute("aria-selected", "true");
  });

  it("moves grid focus with arrow keys using one tab stop", () => {
    render(
      <FileList
        entries={sampleEntries}
        selectedPath="/test/music"
        isLoading={false}
        error={null}
      />,
    );

    const first = screen.getByRole("row", { name: /song1\.mp3/ });
    const second = screen.getByRole("row", { name: /song2\.wav/ });
    expect(first).toHaveAttribute("tabindex", "0");
    expect(second).toHaveAttribute("tabindex", "-1");
    fireEvent.keyDown(first, { key: "ArrowDown" });
    expect(second).toHaveAttribute("tabindex", "0");
  });

  it("starts playback from one left click and exposes row status", () => {
    const onSelect = vi.fn();

    render(
      <FileList
        entries={[sampleEntries[0]]}
        selectedPath="/test/music"
        isLoading={false}
        error={null}
        onFileSelect={onSelect}
        playbackEntryId={sampleEntries[0].id}
        playbackStatus="loading"
      />,
    );

    fireEvent.click(screen.getByText("song1.mp3"));

    expect(onSelect).toHaveBeenCalledWith(sampleEntries[0]);
    expect(screen.getByText("Loading")).toBeInTheDocument();
    expect(
      screen.getByRole("row", { name: /song1\.mp3.*Loading/i }),
    ).toHaveAttribute("aria-selected", "true");
  });

  it("moves the visible selection when transport changes track", () => {
    const { rerender } = render(
      <FileList
        entries={sampleEntries}
        selectedPath="/test/music"
        isLoading={false}
        error={null}
        playbackEntryId={sampleEntries[0].id}
        playbackStatus="playing"
      />,
    );

    rerender(
      <FileList
        entries={sampleEntries}
        selectedPath="/test/music"
        isLoading={false}
        error={null}
        playbackEntryId={sampleEntries[1].id}
        playbackStatus="playing"
      />,
    );

    expect(screen.getByRole("row", { name: /song2\.wav/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  it("shows playback failure without removing the selected row", () => {
    const onSelect = vi.fn();
    render(
      <FileList
        entries={[sampleEntries[0]]}
        selectedPath="/test/music"
        isLoading={false}
        error={null}
        onFileSelect={onSelect}
        playbackEntryId={sampleEntries[0].id}
        playbackStatus="failed"
        playbackError="Unable to play file."
      />,
    );

    fireEvent.click(screen.getByText("song1.mp3"));

    expect(screen.getByText("Unable to play file.")).toBeInTheDocument();
    expect(screen.getByText("song1.mp3")).toBeInTheDocument();
    expect(
      screen.getByRole("row", { name: /song1\.mp3.*Failed/i }),
    ).toHaveAttribute("aria-selected", "true");
  });
});

describe("FileList — stable row identity", () => {
  it("exposes each backend entry id on its row", () => {
    const entries = [
      { id: "a.mp3", name: "a.mp3", kind: "playable" as const },
      { id: "b.mp3", name: "b.mp3", kind: "playable" as const },
    ];

    render(
      <FileList
        entries={entries}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );

    expect(screen.getByRole("row", { name: /a\.mp3/ })).toHaveAttribute(
      "data-row-id",
      "a.mp3",
    );
    expect(screen.getByRole("row", { name: /b\.mp3/ })).toHaveAttribute(
      "data-row-id",
      "b.mp3",
    );
  });
});

describe("FileList — move to Trash", () => {
  it("opens confirmation for the selected row", () => {
    render(
      <FileList
        entries={[sampleEntries[0]]}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );

    fireEvent.click(screen.getByText("song1.mp3"));
    fireEvent.click(screen.getByRole("button", { name: "Move to Trash" }));

    expect(screen.getByRole("alertdialog")).toHaveTextContent("song1.mp3");
  });

  it("cancels without calling the trash command", () => {
    render(
      <FileList
        entries={[sampleEntries[0]]}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );

    fireEvent.click(screen.getByText("song1.mp3"));
    fireEvent.click(screen.getByRole("button", { name: "Move to Trash" }));
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

    expect(mockMoveToTrash).not.toHaveBeenCalled();
    expect(screen.getByText("song1.mp3")).toBeInTheDocument();
  });

  it("moves confirmed row to Trash and refreshes the list", async () => {
    mockMoveToTrash.mockResolvedValueOnce([{ path: "song1.mp3", ok: true }]);
    function Harness() {
      const [entries, setEntries] = useState(sampleEntries);
      return (
        <FileList
          entries={entries}
          selectedPath="/music"
          isLoading={false}
          error={null}
          onEntriesTrashed={(ids) =>
            setEntries((current) =>
              current.filter((entry) => !ids.includes(entry.id)),
            )
          }
        />
      );
    }

    render(<Harness />);
    fireEvent.click(screen.getByText("song1.mp3"));
    fireEvent.click(screen.getByRole("button", { name: "Move to Trash" }));
    fireEvent.click(
      screen
        .getByRole("alertdialog")
        .querySelector(
          "button.confirm-dialog-button--confirm",
        ) as HTMLButtonElement,
    );

    await waitFor(() => {
      expect(screen.queryByText("song1.mp3")).not.toBeInTheDocument();
    });
    expect(screen.getByText("song2.wav")).toBeInTheDocument();
  });

  it("keeps row and shows error after partial failure", async () => {
    mockMoveToTrash.mockResolvedValueOnce([
      {
        path: "song1.mp3",
        ok: false,
        message: "Could not move file.",
      },
    ]);
    render(
      <FileList
        entries={[sampleEntries[0]]}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );
    fireEvent.click(screen.getByText("song1.mp3"));
    fireEvent.click(screen.getByRole("button", { name: "Move to Trash" }));
    fireEvent.click(
      screen
        .getByRole("alertdialog")
        .querySelector(
          "button.confirm-dialog-button--confirm",
        ) as HTMLButtonElement,
    );

    await waitFor(() => {
      expect(screen.getByRole("alert")).toHaveTextContent(
        "Could not move file.",
      );
    });
    expect(screen.getByText("song1.mp3")).toBeInTheDocument();
  });

  it("opens confirmation with Delete keyboard shortcut", () => {
    render(
      <FileList
        entries={[sampleEntries[0]]}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );
    fireEvent.keyDown(screen.getByRole("row", { name: /song1\.mp3/ }), {
      key: "Delete",
    });

    expect(screen.getByRole("alertdialog")).toBeInTheDocument();
  });

  it("opens confirmation for the selected row with global Delete", () => {
    render(
      <FileList
        entries={[sampleEntries[0]]}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );
    fireEvent.click(screen.getByText("song1.mp3"));
    fireEvent.keyDown(window, { key: "Delete" });

    expect(screen.getByRole("alertdialog")).toHaveTextContent("song1.mp3");
  });
});

describe("FileList — large collections", () => {
  it("renders only virtual rows for 100,000 entries", () => {
    const entries: BrowserEntry[] = Array.from(
      { length: 100_000 },
      (_, index) => ({
        id: `/music/sample-${index}.wav`,
        name: `sample-${index}.wav`,
        kind: "playable",
      }),
    );

    render(
      <FileList
        entries={entries}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );

    expect(screen.getAllByRole("row")).toHaveLength(21);
  });
});

describe("FileList — file sorting", () => {
  const baseEntries: BrowserEntry[] = [
    {
      id: "/m/200.flac",
      name: "zulu.flac",
      kind: "playable",
      metadata: {
        duration_ms: 200_000,
        size_bytes: 9_000,
        modified_at_ms: 3_000,
        channels: null,
        sample_rate: null,
        bit_depth: null,
        codec: null,
      },
    },
    {
      id: "/m/100.wav",
      name: "alpha.wav",
      kind: "playable",
      metadata: {
        duration_ms: 10_000,
        size_bytes: 1_000,
        modified_at_ms: 1_000,
        channels: null,
        sample_rate: null,
        bit_depth: null,
        codec: null,
      },
    },
    {
      id: "/m/150.mp3",
      name: "bravo.mp3",
      kind: "playable",
      metadata: {
        duration_ms: 50_000,
        size_bytes: 4_000,
        modified_at_ms: 2_000,
        channels: null,
        sample_rate: null,
        bit_depth: null,
        codec: null,
      },
    },
  ];

  /** Mirrors how App sorts before rendering rows. */
  function SortHarness({
    onSortChange,
  }: {
    onSortChange?: (sort: FileSort) => void;
  }) {
    const [sort, setSort] = useState<FileSort>({
      field: "name",
      direction: "asc",
    });
    const entries = useMemo(() => sortFileEntries(baseEntries, sort), [sort]);
    return (
      <FileList
        entries={entries}
        selectedPath="/music"
        isLoading={false}
        error={null}
        sort={sort}
        onSortChange={(next) => {
          setSort(next);
          onSortChange?.(next);
        }}
      />
    );
  }

  it("clicking a column header requests ascending sort for that field", () => {
    const onSortChange = vi.fn();
    render(
      <SortHarness
        onSortChange={(next) => {
          if (next.field !== "name" || next.direction !== "asc") {
            onSortChange(next);
          }
        }}
      />,
    );

    fireEvent.click(screen.getByRole("columnheader", { name: "Duration" }));

    expect(onSortChange).toHaveBeenCalledWith({
      field: "duration",
      direction: "asc",
    });
  });

  it("clicking the active header toggles the direction", () => {
    const onSortChange = vi.fn();
    render(
      <SortHarness
        onSortChange={(next) => {
          if (next.field !== "name" || next.direction !== "asc") {
            onSortChange(next);
          }
        }}
      />,
    );

    fireEvent.click(screen.getByRole("columnheader", { name: "Duration" }));
    fireEvent.click(screen.getByRole("columnheader", { name: "Duration" }));

    expect(onSortChange).toHaveBeenCalledWith({
      field: "duration",
      direction: "desc",
    });
  });

  it("reflects the active column and direction with aria-sort", () => {
    render(
      <FileList
        entries={sortFileEntries(baseEntries, {
          field: "size",
          direction: "desc",
        })}
        selectedPath="/music"
        isLoading={false}
        error={null}
        sort={{ field: "size", direction: "desc" }}
        onSortChange={() => undefined}
      />,
    );

    expect(screen.getByRole("columnheader", { name: "Size" })).toHaveAttribute(
      "aria-sort",
      "descending",
    );
    expect(
      screen.getByRole("columnheader", { name: "Duration" }),
    ).not.toHaveAttribute("aria-sort");
  });

  it("orders rows according to the requested sort", () => {
    render(
      <FileList
        entries={sortFileEntries(baseEntries, {
          field: "duration",
          direction: "asc",
        })}
        selectedPath="/music"
        isLoading={false}
        error={null}
        sort={{ field: "duration", direction: "asc" }}
        onSortChange={() => undefined}
      />,
    );

    const rows = screen.getAllByRole("row");
    expect(rows[0]).toHaveTextContent("Name");
    expect(rows[1]).toHaveTextContent("alpha.wav");
    expect(rows[2]).toHaveTextContent("bravo.mp3");
    expect(rows[3]).toHaveTextContent("zulu.flac");
  });

  it("keeps the selection when the sort changes", () => {
    render(<SortHarness />);

    fireEvent.click(screen.getByText("zulu.flac"));
    expect(screen.getByRole("row", { name: /zulu\.flac/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );

    // Toggle Duration ascending → descending; rows reorder, selection stays.
    fireEvent.click(screen.getByRole("columnheader", { name: "Duration" }));
    fireEvent.click(screen.getByRole("columnheader", { name: "Duration" }));

    expect(screen.getByRole("row", { name: /zulu\.flac/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  it("keeps sort controls keyboard accessible", () => {
    render(<SortHarness />);

    expect(screen.getByRole("columnheader", { name: "Duration" }).tagName).toBe(
      "BUTTON",
    );
  });
});

describe("FileList — folder search", () => {
  const searchEntries: BrowserEntry[] = [
    {
      id: "/music/kicks/fat-kick.wav",
      name: "fat-kick.wav",
      kind: "playable",
      metadata: null,
    },
    {
      id: "/music/snares/acoustic.wav",
      name: "acoustic.wav",
      kind: "playable",
      metadata: null,
    },
    {
      id: "/music/loops/drum-loop.mp3",
      name: "drum-loop.mp3",
      kind: "playable",
      metadata: null,
    },
  ];

  it("shows an accessible search box in the file list", () => {
    render(
      <FileList
        entries={searchEntries}
        selectedPath="/music"
        isLoading={false}
        error={null}
        searchQuery=""
        onSearchQueryChange={() => undefined}
      />,
    );

    expect(
      screen.getByRole("searchbox", { name: /search files/i }),
    ).toBeInTheDocument();
  });

  it("emits the typed query through onSearchQueryChange", () => {
    const onSearchQueryChange = vi.fn();
    render(
      <FileList
        entries={searchEntries}
        selectedPath="/music"
        isLoading={false}
        error={null}
        searchQuery=""
        onSearchQueryChange={onSearchQueryChange}
      />,
    );

    fireEvent.change(screen.getByRole("searchbox", { name: /search files/i }), {
      target: { value: "kick" },
    });

    expect(onSearchQueryChange).toHaveBeenCalledWith("kick");
  });

  it("keeps the typed query visible in the input", () => {
    render(
      <FileList
        entries={searchEntries}
        selectedPath="/music"
        isLoading={false}
        error={null}
        searchQuery="kick"
        onSearchQueryChange={() => undefined}
      />,
    );

    expect(
      screen.getByRole("searchbox", { name: /search files/i }),
    ).toHaveValue("kick");
  });

  it("shows a distinct message when the search matches nothing", () => {
    // App filters entries before FileList; an empty filtered list plus a
    // non-empty query renders the no-match message.
    render(
      <FileList
        entries={[]}
        selectedPath="/music"
        isLoading={false}
        error={null}
        searchQuery="zzz"
        onSearchQueryChange={() => undefined}
      />,
    );

    expect(screen.getByText(/no files match/)).toBeInTheDocument();
  });

  it("keeps the search box visible when the search matches nothing", () => {
    render(
      <FileList
        entries={[]}
        selectedPath="/music"
        isLoading={false}
        error={null}
        searchQuery="zzz"
        onSearchQueryChange={() => undefined}
      />,
    );

    expect(
      screen.getByRole("searchbox", { name: /search files/i }),
    ).toBeInTheDocument();
    expect(screen.getByText(/no files match/)).toBeInTheDocument();
  });

  it("renders matched folders as navigable rows", () => {
    const entries: BrowserEntry[] = [
      {
        id: "/music/drum-kits",
        name: "drum-kits",
        kind: "folder",
        metadata: null,
      },
      ...searchEntries,
    ];
    const onSelectFolder = vi.fn();

    render(
      <FileList
        entries={entries}
        selectedPath="/music"
        isLoading={false}
        error={null}
        searchQuery="drum"
        onSearchQueryChange={() => undefined}
        onSelectFolder={onSelectFolder}
      />,
    );

    const folderRow = screen.getByRole("row", { name: /drum-kits/ });
    expect(folderRow).toBeInTheDocument();

    fireEvent.click(folderRow);
    expect(onSelectFolder).toHaveBeenCalledWith("/music/drum-kits");
  });

  it("navigates into a folder with the keyboard", () => {
    const entries: BrowserEntry[] = [
      {
        id: "/music/drum-kits",
        name: "drum-kits",
        kind: "folder",
        metadata: null,
      },
      ...searchEntries,
    ];
    const onSelectFolder = vi.fn();

    render(
      <FileList
        entries={entries}
        selectedPath="/music"
        isLoading={false}
        error={null}
        searchQuery="drum"
        onSearchQueryChange={() => undefined}
        onSelectFolder={onSelectFolder}
      />,
    );

    const folderRow = screen.getByRole("row", { name: /drum-kits/ });
    fireEvent.keyDown(folderRow, { key: "Enter" });

    expect(onSelectFolder).toHaveBeenCalledWith("/music/drum-kits");
  });

  it("does not treat a folder row as a playable selection", () => {
    const onFileSelect = vi.fn();
    const entries: BrowserEntry[] = [
      {
        id: "/music/drum-kits",
        name: "drum-kits",
        kind: "folder",
        metadata: null,
      },
      ...searchEntries,
    ];

    render(
      <FileList
        entries={entries}
        selectedPath="/music"
        isLoading={false}
        error={null}
        searchQuery="drum"
        onSearchQueryChange={() => undefined}
        onFileSelect={onFileSelect}
      />,
    );

    fireEvent.click(screen.getByRole("row", { name: /drum-kits/ }));

    expect(onFileSelect).not.toHaveBeenCalled();
  });

  it("keeps the selection when the search query changes", () => {
    function SearchHarness() {
      const [query, setQuery] = useState("");
      const entries = useMemo(
        () => filterFileEntries(searchEntries, query),
        [query],
      );
      return (
        <FileList
          entries={entries}
          selectedPath="/music"
          isLoading={false}
          error={null}
          searchQuery={query}
          onSearchQueryChange={setQuery}
        />
      );
    }

    render(<SearchHarness />);

    fireEvent.click(screen.getByText("acoustic.wav"));
    expect(screen.getByRole("row", { name: /acoustic\.wav/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );

    fireEvent.change(screen.getByRole("searchbox", { name: /search files/i }), {
      target: { value: "acoustic" },
    });
    expect(screen.getByRole("row", { name: /acoustic\.wav/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );

    fireEvent.change(screen.getByRole("searchbox", { name: /search files/i }), {
      target: { value: "" },
    });
    expect(screen.getByRole("row", { name: /acoustic\.wav/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  it("handles rapid successive query changes", () => {
    function RapidHarness() {
      const [query, setQuery] = useState("");
      const entries = useMemo(
        () => filterFileEntries(searchEntries, query),
        [query],
      );
      return (
        <FileList
          entries={entries}
          selectedPath="/music"
          isLoading={false}
          error={null}
          searchQuery={query}
          onSearchQueryChange={setQuery}
        />
      );
    }

    render(<RapidHarness />);
    const input = screen.getByRole("searchbox", { name: /search files/i });

    fireEvent.change(input, { target: { value: "d" } });
    fireEvent.change(input, { target: { value: "dr" } });
    fireEvent.change(input, { target: { value: "drum" } });

    expect(screen.getByText("drum-loop.mp3")).toBeInTheDocument();
    expect(screen.queryByText("fat-kick.wav")).not.toBeInTheDocument();
  });
});

describe("FileList — format filter", () => {
  const formatEntries: BrowserEntry[] = [
    {
      id: "loop.mp3",
      name: "loop.mp3",
      kind: "playable",
      metadata: {
        duration_ms: null,
        size_bytes: null,
        modified_at_ms: null,
        channels: null,
        sample_rate: null,
        bit_depth: null,
        codec: "MP3",
      },
    },
    {
      id: "kick.wav",
      name: "kick.wav",
      kind: "playable",
      metadata: {
        duration_ms: null,
        size_bytes: null,
        modified_at_ms: null,
        channels: null,
        sample_rate: null,
        bit_depth: null,
        codec: "PCM",
      },
    },
    {
      id: "kit.flac",
      name: "kit.flac",
      kind: "playable",
      metadata: {
        duration_ms: null,
        size_bytes: null,
        modified_at_ms: null,
        channels: null,
        sample_rate: null,
        bit_depth: null,
        codec: "FLAC",
      },
    },
  ];

  it("renders an accessible format filter with every format option", () => {
    render(
      <FileList
        entries={formatEntries}
        selectedPath="/music"
        isLoading={false}
        error={null}
        formatFilter={[]}
        onFormatFilterChange={() => undefined}
      />,
    );

    for (const option of FORMAT_OPTIONS) {
      expect(
        screen.getByRole("checkbox", { name: option.label }),
      ).toBeInTheDocument();
    }
    expect(
      screen.getByRole("button", { name: "Reset format filter" }),
    ).toBeInTheDocument();
  });

  it("emits a format selection through onFormatFilterChange", () => {
    const onFormatFilterChange = vi.fn();
    render(
      <FileList
        entries={formatEntries}
        selectedPath="/music"
        isLoading={false}
        error={null}
        formatFilter={[]}
        onFormatFilterChange={onFormatFilterChange}
      />,
    );

    fireEvent.click(screen.getByRole("checkbox", { name: "MP3" }));

    expect(onFormatFilterChange).toHaveBeenCalledWith(["mp3"]);
  });

  it("unselecting a checked format removes it from the selection", () => {
    const onFormatFilterChange = vi.fn();
    render(
      <FileList
        entries={formatEntries}
        selectedPath="/music"
        isLoading={false}
        error={null}
        formatFilter={["mp3", "flac"]}
        onFormatFilterChange={onFormatFilterChange}
      />,
    );

    fireEvent.click(screen.getByRole("checkbox", { name: "MP3" }));

    expect(onFormatFilterChange).toHaveBeenCalledWith(["flac"]);
  });

  it("reset emits an empty format list", () => {
    const onFormatFilterChange = vi.fn();
    render(
      <FileList
        entries={formatEntries}
        selectedPath="/music"
        isLoading={false}
        error={null}
        formatFilter={["mp3", "flac"]}
        onFormatFilterChange={onFormatFilterChange}
      />,
    );

    fireEvent.click(
      screen.getByRole("button", { name: "Reset format filter" }),
    );

    expect(onFormatFilterChange).toHaveBeenCalledWith([]);
  });

  it("reset is disabled while no format filter is active", () => {
    render(
      <FileList
        entries={formatEntries}
        selectedPath="/music"
        isLoading={false}
        error={null}
        formatFilter={[]}
        onFormatFilterChange={() => undefined}
      />,
    );

    expect(
      screen.getByRole("button", { name: "Reset format filter" }),
    ).toBeDisabled();
  });

  it("shows a distinct message when the format filter matches nothing", () => {
    render(
      <FileList
        entries={[]}
        selectedPath="/music"
        isLoading={false}
        error={null}
        formatFilter={["mp3"]}
        onFormatFilterChange={() => undefined}
      />,
    );

    expect(
      screen.getByText("(no files match the format filter)"),
    ).toBeInTheDocument();
  });

  it("keeps the filter controls visible when the format filter matches nothing", () => {
    render(
      <FileList
        entries={[]}
        selectedPath="/music"
        isLoading={false}
        error={null}
        formatFilter={["mp3"]}
        onFormatFilterChange={() => undefined}
      />,
    );

    expect(screen.getByRole("checkbox", { name: "MP3" })).toBeInTheDocument();
    expect(
      screen.getByText("(no files match the format filter)"),
    ).toBeInTheDocument();
  });

  it("keeps the selection when the format filter changes", () => {
    function FormatHarness() {
      const [formats, setFormats] = useState<AudioFileFormat[]>([]);
      const entries = useMemo(
        () => filterByFormat(formatEntries, formats),
        [formats],
      );
      return (
        <FileList
          entries={entries}
          selectedPath="/music"
          isLoading={false}
          error={null}
          formatFilter={formats}
          onFormatFilterChange={setFormats}
        />
      );
    }

    render(<FormatHarness />);

    fireEvent.click(screen.getByText("kick.wav"));
    expect(screen.getByRole("row", { name: /kick\.wav/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );

    // Filter to PCM only: the selected WAV row stays visible and selected.
    fireEvent.click(screen.getByRole("checkbox", { name: "WAV/PCM" }));
    expect(screen.getByRole("row", { name: /kick\.wav/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.queryByText("loop.mp3")).not.toBeInTheDocument();

    // Clear the filter again: selection is retained.
    fireEvent.click(
      screen.getByRole("button", { name: "Reset format filter" }),
    );
    expect(screen.getByRole("row", { name: /kick\.wav/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  it("keeps folder rows visible while the format filter is active", () => {
    const entries: BrowserEntry[] = [
      {
        id: "/music/drum-kits",
        name: "drum-kits",
        kind: "folder",
        metadata: null,
      },
      ...formatEntries,
    ];

    render(
      <FileList
        entries={filterByFormat(entries, ["mp3"])}
        selectedPath="/music"
        isLoading={false}
        error={null}
        formatFilter={["mp3"]}
        onFormatFilterChange={() => undefined}
      />,
    );

    expect(screen.getByRole("row", { name: /drum-kits/ })).toBeInTheDocument();
    expect(screen.getByText("loop.mp3")).toBeInTheDocument();
    expect(screen.queryByText("kick.wav")).not.toBeInTheDocument();
  });
});

describe("FileList — hides non-playable entries", () => {
  it("never renders unsupported or inaccessible entries", () => {
    const entries: BrowserEntry[] = [
      { id: "song.wav", name: "song.wav", kind: "playable" },
      {
        id: "/music/kits",
        name: "kits",
        kind: "folder",
        metadata: null,
      },
      {
        id: "setup.msi",
        name: "setup.msi",
        kind: "unsupported",
        metadata: null,
      },
      {
        id: "broken.log",
        name: "broken.log",
        kind: "inaccessible",
        metadata: null,
      },
    ];

    render(
      <FileList
        entries={entries}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );

    expect(screen.getByText("song.wav")).toBeInTheDocument();
    expect(screen.getByRole("row", { name: /kits/ })).toBeInTheDocument();
    expect(screen.queryByText("setup.msi")).not.toBeInTheDocument();
    expect(screen.queryByText("broken.log")).not.toBeInTheDocument();
  });

  it("excludes hidden entries from the accessible row count", () => {
    const entries: BrowserEntry[] = [
      { id: "song.wav", name: "song.wav", kind: "playable" },
      {
        id: "setup.msi",
        name: "setup.msi",
        kind: "unsupported",
        metadata: null,
      },
    ];

    render(
      <FileList
        entries={entries}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );

    const grid = screen.getByRole("grid", { name: "Playable files" });
    expect(grid).toHaveAttribute("aria-rowcount", "2");
  });
});

describe("FileList — multiple selection", () => {
  it("toggles a row with ctrl+click without starting playback", () => {
    const onSelect = vi.fn();

    render(
      <FileList
        entries={sampleEntries}
        selectedPath="/music"
        isLoading={false}
        error={null}
        onFileSelect={onSelect}
      />,
    );

    const song1 = screen.getByRole("row", { name: /song1\.mp3/ });
    const song2 = screen.getByRole("row", { name: /song2\.wav/ });

    fireEvent.click(song1);
    expect(song1).toHaveAttribute("aria-selected", "true");

    fireEvent.click(song2, { ctrlKey: true });

    expect(song1).toHaveAttribute("aria-selected", "true");
    expect(song2).toHaveAttribute("aria-selected", "true");
    expect(onSelect).toHaveBeenCalledOnce();
    expect(onSelect).toHaveBeenCalledWith(sampleEntries[0]);

    fireEvent.click(song1, { ctrlKey: true });

    expect(song1).toHaveAttribute("aria-selected", "false");
    expect(song2).toHaveAttribute("aria-selected", "true");
    expect(onSelect).toHaveBeenCalledOnce();
  });

  it("toggles a row with meta+click too", () => {
    const onSelect = vi.fn();

    render(
      <FileList
        entries={sampleEntries}
        selectedPath="/music"
        isLoading={false}
        error={null}
        onFileSelect={onSelect}
      />,
    );

    fireEvent.click(screen.getByRole("row", { name: /song1\.mp3/ }));
    fireEvent.click(screen.getByRole("row", { name: /song2\.wav/ }), {
      metaKey: true,
    });

    expect(screen.getByRole("row", { name: /song1\.mp3/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByRole("row", { name: /song2\.wav/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(onSelect).toHaveBeenCalledOnce();
  });

  it("selects a range with shift+click without starting playback", () => {
    const onSelect = vi.fn();

    render(
      <FileList
        entries={sampleEntries}
        selectedPath="/music"
        isLoading={false}
        error={null}
        onFileSelect={onSelect}
      />,
    );

    fireEvent.click(screen.getByRole("row", { name: /song1\.mp3/ }));
    fireEvent.click(screen.getByRole("row", { name: /song3\.flac/ }), {
      shiftKey: true,
    });

    for (const name of [/song1\.mp3/, /song2\.wav/, /song3\.flac/]) {
      expect(screen.getByRole("row", { name })).toHaveAttribute(
        "aria-selected",
        "true",
      );
    }
    expect(onSelect).toHaveBeenCalledOnce();
    expect(onSelect).toHaveBeenCalledWith(sampleEntries[0]);
  });

  it("selects all playable rows with Ctrl/Cmd+A and skips folders", () => {
    const entries: BrowserEntry[] = [
      { id: "/music/kits", name: "kits", kind: "folder", metadata: null },
      ...sampleEntries,
    ];

    render(
      <FileList
        entries={entries}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );

    fireEvent.keyDown(screen.getByRole("row", { name: /kits/ }), {
      key: "a",
      ctrlKey: true,
    });

    for (const name of [/song1\.mp3/, /song2\.wav/, /song3\.flac/]) {
      expect(screen.getByRole("row", { name })).toHaveAttribute(
        "aria-selected",
        "true",
      );
    }
    expect(screen.getByRole("row", { name: /kits/ })).toHaveAttribute(
      "aria-selected",
      "false",
    );
  });

  it("extends the selection with shift+arrow keys", () => {
    render(
      <FileList
        entries={sampleEntries}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );

    const song1 = screen.getByRole("row", { name: /song1\.mp3/ });
    fireEvent.click(song1);
    fireEvent.keyDown(song1, { key: "ArrowDown", shiftKey: true });

    expect(screen.getByRole("row", { name: /song1\.mp3/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByRole("row", { name: /song2\.wav/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByRole("row", { name: /song3\.flac/ })).toHaveAttribute(
      "aria-selected",
      "false",
    );
  });

  it("re-selects the playing row when returning to its folder", () => {
    const { rerender } = render(
      <FileList
        entries={sampleEntries}
        selectedPath="/music"
        isLoading={false}
        error={null}
        playbackEntryId={sampleEntries[0].id}
        playbackStatus="playing"
      />,
    );

    expect(screen.getByRole("row", { name: /song1\.mp3/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );

    // Navigate away: selection clears while playback keeps running.
    rerender(
      <FileList
        entries={[]}
        selectedPath="/other"
        isLoading={false}
        error={null}
        playbackEntryId={sampleEntries[0].id}
        playbackStatus="playing"
      />,
    );
    expect(screen.queryByRole("grid", { name: "Playable files" })).toBeNull();

    // Return: the still-playing row is selected again.
    rerender(
      <FileList
        entries={sampleEntries}
        selectedPath="/music"
        isLoading={false}
        error={null}
        playbackEntryId={sampleEntries[0].id}
        playbackStatus="playing"
      />,
    );
    expect(screen.getByRole("row", { name: /song1\.mp3/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  it("keeps the selection when more rows stream in", () => {
    function StreamHarness() {
      const [entries, setEntries] = useState<BrowserEntry[]>([
        sampleEntries[0],
      ]);
      return (
        <div>
          <button type="button" onClick={() => setEntries(sampleEntries)}>
            Stream more
          </button>
          <FileList
            entries={entries}
            selectedPath="/music"
            isLoading={false}
            error={null}
          />
        </div>
      );
    }

    render(<StreamHarness />);
    fireEvent.click(screen.getByRole("row", { name: /song1\.mp3/ }));
    expect(screen.getByRole("row", { name: /song1\.mp3/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );

    fireEvent.click(screen.getByRole("button", { name: "Stream more" }));

    expect(screen.getByRole("row", { name: /song1\.mp3/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByRole("row", { name: /song2\.wav/ })).toHaveAttribute(
      "aria-selected",
      "false",
    );
    expect(screen.getByRole("row", { name: /song3\.flac/ })).toHaveAttribute(
      "aria-selected",
      "false",
    );
  });

  it("prunes a trashed row from the selection and keeps survivors", async () => {
    mockMoveToTrash.mockResolvedValueOnce([{ path: "song2.wav", ok: true }]);

    function TrashHarness() {
      const [entries, setEntries] = useState(sampleEntries);
      return (
        <FileList
          entries={entries}
          selectedPath="/music"
          isLoading={false}
          error={null}
          onEntriesTrashed={(ids) =>
            setEntries((current) =>
              current.filter((entry) => !ids.includes(entry.id)),
            )
          }
        />
      );
    }

    render(<TrashHarness />);
    fireEvent.click(screen.getByRole("row", { name: /song1\.mp3/ }));
    fireEvent.click(screen.getByRole("row", { name: /song2\.wav/ }), {
      ctrlKey: true,
    });
    expect(screen.getByRole("row", { name: /song1\.mp3/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByRole("row", { name: /song2\.wav/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );

    fireEvent.click(screen.getByRole("button", { name: "Move to Trash" }));
    fireEvent.click(
      screen
        .getByRole("alertdialog")
        .querySelector(
          "button.confirm-dialog-button--confirm",
        ) as HTMLButtonElement,
    );

    await waitFor(() => {
      expect(screen.queryByText("song2.wav")).not.toBeInTheDocument();
    });
    expect(screen.getByRole("row", { name: /song1\.mp3/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByRole("row", { name: /song3\.flac/ })).toHaveAttribute(
      "aria-selected",
      "false",
    );
  });

  it("clears the selection when the last selected row is removed", async () => {
    mockMoveToTrash.mockResolvedValueOnce([{ path: "song1.mp3", ok: true }]);

    function SingleHarness() {
      const [entries, setEntries] = useState([sampleEntries[0]]);
      return (
        <FileList
          entries={entries}
          selectedPath="/music"
          isLoading={false}
          error={null}
          onEntriesTrashed={(ids) =>
            setEntries((current) =>
              current.filter((entry) => !ids.includes(entry.id)),
            )
          }
        />
      );
    }

    render(<SingleHarness />);
    fireEvent.click(screen.getByRole("row", { name: /song1\.mp3/ }));
    fireEvent.click(screen.getByRole("button", { name: "Move to Trash" }));
    fireEvent.click(
      screen
        .getByRole("alertdialog")
        .querySelector(
          "button.confirm-dialog-button--confirm",
        ) as HTMLButtonElement,
    );

    await waitFor(() => {
      expect(screen.queryByText("song1.mp3")).not.toBeInTheDocument();
    });
    expect(
      screen.getByRole("button", { name: "Move to Trash" }),
    ).toBeDisabled();
  });
});

describe("FileList — session marks", () => {
  const markedEntries: BrowserEntry[] = [
    { id: "song1.mp3", name: "song1.mp3", kind: "playable" },
    { id: "song2.wav", name: "song2.wav", kind: "playable" },
    { id: "song3.flac", name: "song3.flac", kind: "playable" },
  ];

  function MarkHarness({
    marks = {},
    markFilter = "all",
    onMarkChange = vi.fn(),
    onMarkFilterChange = vi.fn(),
  }: {
    marks?: SessionMarks;
    markFilter?: MarkFilter;
    onMarkChange?: (ids: string[], mark: SessionMark | null) => void;
    onMarkFilterChange?: (filter: MarkFilter) => void;
  }) {
    return (
      <FileList
        entries={markedEntries}
        selectedPath="/music"
        isLoading={false}
        error={null}
        marks={marks}
        onMarkChange={onMarkChange}
        markFilter={markFilter}
        onMarkFilterChange={onMarkFilterChange}
      />
    );
  }

  it("renders mark controls in the actions bar", () => {
    render(<MarkHarness />);

    expect(
      screen.getByRole("button", { name: "Mark Keep" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Mark Maybe" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Mark Reject" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Mark Favorite" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Select marked" }),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Filter by mark")).toBeInTheDocument();
  });

  it("disables mark buttons without a playable selection", () => {
    render(<MarkHarness />);

    expect(screen.getByRole("button", { name: "Mark Keep" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Clear mark" })).toBeDisabled();
  });

  it("applies a mark to the single selected file", () => {
    const onMarkChange = vi.fn();
    render(<MarkHarness onMarkChange={onMarkChange} />);

    fireEvent.click(screen.getByText("song1.mp3"));
    fireEvent.click(screen.getByRole("button", { name: "Mark Keep" }));

    expect(onMarkChange).toHaveBeenCalledWith(["song1.mp3"], "keep");
  });

  it("applies a mark to every selected file", () => {
    const onMarkChange = vi.fn();
    render(<MarkHarness onMarkChange={onMarkChange} />);

    fireEvent.click(screen.getByText("song1.mp3"));
    fireEvent.click(screen.getByText("song2.wav"), { ctrlKey: true });
    fireEvent.click(screen.getByRole("button", { name: "Mark Maybe" }));

    expect(onMarkChange).toHaveBeenCalledWith(
      ["song1.mp3", "song2.wav"],
      "maybe",
    );
  });

  it("clears the mark of the selection", () => {
    const onMarkChange = vi.fn();
    render(
      <MarkHarness
        marks={{ "song1.mp3": "keep" }}
        onMarkChange={onMarkChange}
      />,
    );

    fireEvent.click(screen.getByText("song1.mp3"));
    fireEvent.click(screen.getByRole("button", { name: "Clear mark" }));

    expect(onMarkChange).toHaveBeenCalledWith(["song1.mp3"], null);
  });

  it("shows the session mark in the Mark column", () => {
    render(
      <MarkHarness marks={{ "song1.mp3": "keep", "song2.wav": "favorite" }} />,
    );

    expect(
      within(screen.getByRole("row", { name: /song1\.mp3/ })).getByText("Keep"),
    ).toBeInTheDocument();
    expect(
      within(screen.getByRole("row", { name: /song2\.wav/ })).getByText(
        "Favorite",
      ),
    ).toBeInTheDocument();
    expect(
      within(screen.getByRole("row", { name: /song3\.flac/ })).queryByText(
        "Keep",
      ),
    ).not.toBeInTheDocument();
  });

  it("shows colored pastilles in the name column", () => {
    render(
      <MarkHarness
        marks={{
          "song1.mp3": "keep",
          "song2.wav": "reject",
          "song3.flac": "favorite",
        }}
      />,
    );

    expect(
      screen
        .getByRole("row", { name: /song1\.mp3/ })
        .querySelector(".file-list-mark-dot--keep"),
    ).not.toBeNull();
    expect(
      screen
        .getByRole("row", { name: /song2\.wav/ })
        .querySelector(".file-list-mark-dot--reject"),
    ).not.toBeNull();
    const favoriteRow = screen.getByRole("row", { name: /song3\.flac/ });
    expect(
      favoriteRow.querySelector(".file-list-mark-dot--favorite"),
    ).not.toBeNull();
    expect(within(favoriteRow).getByText("★")).toBeInTheDocument();
  });

  it("keeps marks when the folder changes", () => {
    const { rerender } = render(
      <FileList
        entries={markedEntries}
        selectedPath="/a"
        isLoading={false}
        error={null}
        marks={{ "song1.mp3": "keep" }}
      />,
    );

    expect(
      within(screen.getByRole("row", { name: /song1\.mp3/ })).getByText("Keep"),
    ).toBeInTheDocument();

    rerender(
      <FileList
        entries={markedEntries}
        selectedPath="/b"
        isLoading={false}
        error={null}
        marks={{ "song1.mp3": "keep" }}
      />,
    );

    expect(
      within(screen.getByRole("row", { name: /song1\.mp3/ })).getByText("Keep"),
    ).toBeInTheDocument();
  });

  it("changes the mark filter through the select", () => {
    const onMarkFilterChange = vi.fn();
    render(<MarkHarness onMarkFilterChange={onMarkFilterChange} />);

    fireEvent.change(screen.getByLabelText("Filter by mark"), {
      target: { value: "keep" },
    });

    expect(onMarkFilterChange).toHaveBeenCalledWith("keep");
  });

  it("disables Select marked when nothing is marked", () => {
    render(<MarkHarness />);
    expect(
      screen.getByRole("button", { name: "Select marked" }),
    ).toBeDisabled();
  });

  it("batch-selects every marked file", () => {
    render(
      <MarkHarness marks={{ "song1.mp3": "keep", "song2.wav": "maybe" }} />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Select marked" }));

    expect(screen.getByRole("row", { name: /song1\.mp3/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByRole("row", { name: /song2\.wav/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByRole("row", { name: /song3\.flac/ })).toHaveAttribute(
      "aria-selected",
      "false",
    );
  });

  it("applies a mark to the selection via keyboard shortcut", () => {
    Object.defineProperty(navigator, "platform", {
      configurable: true,
      value: "MacIntel",
    });
    const onMarkChange = vi.fn();
    render(<MarkHarness onMarkChange={onMarkChange} />);

    fireEvent.click(screen.getByText("song1.mp3"));
    fireEvent.keyDown(window, { key: "k", metaKey: true, shiftKey: true });

    expect(onMarkChange).toHaveBeenCalledWith(["song1.mp3"], "keep");
  });
});

describe("FileList — session marks end to end", () => {
  const entries: BrowserEntry[] = [
    { id: "song1.mp3", name: "song1.mp3", kind: "playable" },
    { id: "song2.wav", name: "song2.wav", kind: "playable" },
    { id: "song3.flac", name: "song3.flac", kind: "playable" },
  ];

  beforeEach(() => {
    mockMoveToTrash.mockClear();
  });

  function SessionMarksHarness() {
    const session = useSessionMarks();
    const [filter, setFilter] = useState<MarkFilter>("all");
    const visible = useMemo(
      () => filterByMark(entries, session.marks, filter),
      [session.marks, filter],
    );
    return (
      <FileList
        entries={visible}
        selectedPath="/music"
        isLoading={false}
        error={null}
        marks={session.marks}
        onMarkChange={(ids, mark) => {
          if (mark === null) session.unmark(ids);
          else session.setMark(ids, mark);
        }}
        markFilter={filter}
        onMarkFilterChange={setFilter}
      />
    );
  }

  it("marks a file, filters to its mark, and batch-selects marked files", () => {
    render(<SessionMarksHarness />);

    fireEvent.click(screen.getByText("song1.mp3"));
    fireEvent.click(screen.getByRole("button", { name: "Mark Keep" }));
    expect(
      within(screen.getByRole("row", { name: /song1\.mp3/ })).getByText("Keep"),
    ).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Filter by mark"), {
      target: { value: "keep" },
    });
    expect(screen.queryByText("song2.wav")).not.toBeInTheDocument();
    expect(screen.getByText("song1.mp3")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Select marked" }));
    expect(screen.getByRole("row", { name: /song1\.mp3/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  it("marking never touches the backend (FR-LS-007)", () => {
    function MarkFlowHarness() {
      const session = useSessionMarks();
      return (
        <FileList
          entries={entries}
          selectedPath="/music"
          isLoading={false}
          error={null}
          marks={session.marks}
          onMarkChange={(ids, mark) => {
            if (mark === null) session.unmark(ids);
            else session.setMark(ids, mark);
          }}
        />
      );
    }

    render(<MarkFlowHarness />);
    fireEvent.click(screen.getByText("song1.mp3"));
    fireEvent.click(screen.getByRole("button", { name: "Mark Keep" }));
    expect(
      within(screen.getByRole("row", { name: /song1\.mp3/ })).getByText("Keep"),
    ).toBeInTheDocument();

    // Session marks must never create library items (FR-LS-007): the only
    // backend call this component can make is Move to Trash, and marking
    // must not trigger it.
    expect(mockMoveToTrash).not.toHaveBeenCalled();
  });
});

describe("FileList — recursive view", () => {
  it("renders a toggle that reports changes and shows the pressed state", () => {
    const onRecursiveChange = vi.fn();
    render(
      <FileList
        entries={[]}
        selectedPath="/music"
        isLoading={false}
        error={null}
        recursive={false}
        onRecursiveChange={onRecursiveChange}
      />,
    );

    const toggle = screen.getByRole("button", { name: "Recursive view" });
    expect(toggle).toHaveAttribute("aria-pressed", "false");
    fireEvent.click(toggle);
    expect(onRecursiveChange).toHaveBeenCalledWith(true);
  });

  it("shows relative paths instead of bare names in recursive mode", () => {
    render(
      <FileList
        entries={[
          {
            id: "/music/album/one.wav",
            name: "one.wav",
            kind: "playable",
          },
          {
            id: "/music/album/sub/two.wav",
            name: "two.wav",
            kind: "playable",
          },
        ]}
        selectedPath="/music/album"
        isLoading={false}
        error={null}
        recursive
      />,
    );

    expect(screen.getByText("one.wav")).toBeInTheDocument();
    expect(screen.getByText("sub/two.wav")).toBeInTheDocument();
    expect(screen.queryByText("two.wav")).not.toBeInTheDocument();
  });

  it("keeps bare names when recursive mode is off", () => {
    render(
      <FileList
        entries={[
          {
            id: "/music/album/one.wav",
            name: "one.wav",
            kind: "playable",
          },
        ]}
        selectedPath="/music/album"
        isLoading={false}
        error={null}
        recursive={false}
      />,
    );

    expect(screen.getByText("one.wav")).toBeInTheDocument();
  });
});

describe("FileList — rename", () => {
  beforeEach(() => {
    mockRenameFile.mockClear();
  });

  it("disables rename for folder rows", () => {
    const entries: BrowserEntry[] = [
      {
        id: "/music/drum-kits",
        name: "drum-kits",
        kind: "folder",
        metadata: null,
      },
    ];

    render(
      <FileList
        entries={entries}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );

    fireEvent.click(screen.getByRole("row", { name: /drum-kits/ }));
    expect(screen.getByRole("button", { name: "Rename" })).toBeDisabled();
  });

  it("opens the rename dialog for the selected row", () => {
    render(
      <FileList
        entries={[sampleEntries[0]]}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );

    fireEvent.click(screen.getByText("song1.mp3"));
    fireEvent.click(screen.getByRole("button", { name: "Rename" }));

    expect(screen.getByRole("alertdialog")).toHaveTextContent("song1.mp3");
    expect(screen.getByLabelText("New file name")).toHaveValue("song1.mp3");
  });

  it("cancels without calling the rename command", () => {
    render(
      <FileList
        entries={[sampleEntries[0]]}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );

    fireEvent.click(screen.getByText("song1.mp3"));
    fireEvent.click(screen.getByRole("button", { name: "Rename" }));
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

    expect(mockRenameFile).not.toHaveBeenCalled();
  });

  it("renames the confirmed row and reports the new identity", async () => {
    mockRenameFile.mockResolvedValueOnce({
      old_path: "song1.mp3",
      new_path: "renamed.mp3",
      was_playing: false,
    });
    const onEntryRenamed = vi.fn();
    function Harness() {
      const [entries, setEntries] = useState(sampleEntries);
      return (
        <FileList
          entries={entries}
          selectedPath="/music"
          isLoading={false}
          error={null}
          onEntryRenamed={(oldId, newId, newName) => {
            onEntryRenamed(oldId, newId, newName);
            setEntries((current) =>
              current.map((entry) =>
                entry.id === oldId
                  ? { ...entry, id: newId, name: newName }
                  : entry,
              ),
            );
          }}
        />
      );
    }

    render(<Harness />);
    fireEvent.click(screen.getByText("song1.mp3"));
    fireEvent.click(screen.getByRole("button", { name: "Rename" }));
    fireEvent.change(screen.getByLabelText("New file name"), {
      target: { value: "renamed.mp3" },
    });
    fireEvent.click(
      within(screen.getByRole("alertdialog")).getByRole("button", {
        name: "Rename",
      }),
    );

    await waitFor(() => {
      expect(onEntryRenamed).toHaveBeenCalledWith(
        "song1.mp3",
        "renamed.mp3",
        "renamed.mp3",
      );
    });
    expect(screen.queryByText("song1.mp3")).not.toBeInTheDocument();
    expect(screen.getByText("renamed.mp3")).toBeInTheDocument();
  });

  it("keeps the row and shows the error after a failed rename", async () => {
    mockRenameFile.mockRejectedValueOnce(
      new Error("That name is already taken."),
    );
    render(
      <FileList
        entries={[sampleEntries[0]]}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );

    fireEvent.click(screen.getByText("song1.mp3"));
    fireEvent.click(screen.getByRole("button", { name: "Rename" }));
    fireEvent.change(screen.getByLabelText("New file name"), {
      target: { value: "existing.mp3" },
    });
    fireEvent.click(
      within(screen.getByRole("alertdialog")).getByRole("button", {
        name: "Rename",
      }),
    );

    await waitFor(() => {
      expect(screen.getByRole("alert")).toHaveTextContent(
        "That name is already taken.",
      );
    });
    expect(screen.getByText("song1.mp3")).toBeInTheDocument();
  });
});

describe("FileList — move", () => {
  beforeEach(() => {
    mockStartMoveFiles.mockClear();
    mockCancelMoveFiles.mockReset();
    mockCancelMoveFiles.mockResolvedValue(undefined);
    mockPickFolder.mockClear();
    eventHandlers.clear();
  });

  function emitMoveProgress(payload: unknown) {
    const handler = eventHandlers.get("browser:move-progress");
    if (!handler) throw new Error("move-progress listener not registered");
    act(() => {
      handler({ payload });
    });
  }

  function selectSongs() {
    fireEvent.click(screen.getByRole("row", { name: /song1\.mp3/ }));
    fireEvent.click(screen.getByRole("row", { name: /song2\.wav/ }), {
      ctrlKey: true,
    });
  }

  it("disables Move… without a playable selection", () => {
    render(
      <FileList
        entries={[sampleEntries[0]]}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );

    expect(screen.getByRole("button", { name: "Move…" })).toBeDisabled();
  });

  it("opens the dialog and requires a target folder before confirming", async () => {
    mockPickFolder.mockResolvedValueOnce("/library");
    render(
      <FileList
        entries={sampleEntries}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );

    selectSongs();
    fireEvent.click(screen.getByRole("button", { name: "Move…" }));

    const dialog = screen.getByRole("alertdialog");
    expect(dialog).toHaveTextContent("Move 2 files into a folder.");
    expect(within(dialog).getByRole("button", { name: "Move" })).toBeDisabled();

    fireEvent.click(
      within(dialog).getByRole("button", { name: "Choose folder…" }),
    );
    await waitFor(() => {
      expect(within(dialog).getByText("/library")).toBeInTheDocument();
    });
    expect(
      within(dialog).getByRole("button", { name: "Move" }),
    ).not.toBeDisabled();
  });

  it("starts the move with the selected ids and target", async () => {
    mockPickFolder.mockResolvedValueOnce("/library");
    mockStartMoveFiles.mockResolvedValueOnce("move-1");
    render(
      <FileList
        entries={sampleEntries}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );

    selectSongs();
    fireEvent.click(screen.getByRole("button", { name: "Move…" }));
    fireEvent.click(
      within(screen.getByRole("alertdialog")).getByRole("button", {
        name: "Choose folder…",
      }),
    );
    await waitFor(() => {
      expect(
        within(screen.getByRole("alertdialog")).getByRole("button", {
          name: "Move",
        }),
      ).not.toBeDisabled();
    });
    fireEvent.click(
      within(screen.getByRole("alertdialog")).getByRole("button", {
        name: "Move",
      }),
    );

    await waitFor(() => {
      expect(mockStartMoveFiles).toHaveBeenCalledWith(
        ["song1.mp3", "song2.wav"],
        "/library",
      );
    });
    expect(screen.getByText("Moving file 0 of 2…")).toBeInTheDocument();
  });

  it("reports success and failure separately and drops moved rows", async () => {
    mockPickFolder.mockResolvedValueOnce("/library");
    mockStartMoveFiles.mockResolvedValueOnce("move-1");
    const onEntriesMoved = vi.fn();
    function Harness() {
      const [entries, setEntries] = useState(sampleEntries);
      return (
        <FileList
          entries={entries}
          selectedPath="/music"
          isLoading={false}
          error={null}
          onEntriesMoved={(moved) => {
            onEntriesMoved(moved);
            setEntries((current) =>
              current.filter(
                (entry) => !moved.some((item) => item.oldId === entry.id),
              ),
            );
          }}
        />
      );
    }

    render(<Harness />);
    selectSongs();
    fireEvent.click(screen.getByRole("button", { name: "Move…" }));
    fireEvent.click(
      within(screen.getByRole("alertdialog")).getByRole("button", {
        name: "Choose folder…",
      }),
    );
    await waitFor(() => {
      expect(mockPickFolder).toHaveBeenCalled();
    });
    fireEvent.click(
      within(screen.getByRole("alertdialog")).getByRole("button", {
        name: "Move",
      }),
    );
    await waitFor(() => {
      expect(mockStartMoveFiles).toHaveBeenCalled();
    });
    await waitFor(() => {
      expect(eventHandlers.has("browser:move-progress")).toBe(true);
    });

    emitMoveProgress({
      session_id: "move-1",
      completed: 1,
      total: 2,
      done: false,
      results: [
        { path: "song1.mp3", new_path: "/library/song1.mp3", ok: true },
      ],
    });
    emitMoveProgress({
      session_id: "move-1",
      completed: 2,
      total: 2,
      done: true,
      results: [
        { path: "song1.mp3", new_path: "/library/song1.mp3", ok: true },
        {
          path: "song2.wav",
          ok: false,
          category: "Conflict",
          message: "PulseSeek could not apply that change.",
          diagnostic_code: "file.operation",
        },
      ],
    });

    await waitFor(() => {
      expect(onEntriesMoved).toHaveBeenCalledWith([
        { oldId: "song1.mp3", newId: "/library/song1.mp3" },
      ]);
    });
    const dialog = screen.getByRole("alertdialog");
    expect(within(dialog).getByText("1 file moved.")).toBeInTheDocument();
    expect(
      within(dialog).getByText("1 file could not be moved:"),
    ).toBeInTheDocument();
    expect(screen.queryByText("song1.mp3")).not.toBeInTheDocument();
    expect(screen.getByText("song2.wav")).toBeInTheDocument();
  });

  it("completes when the done event races the start reply", async () => {
    mockPickFolder.mockResolvedValueOnce("/library");
    let resolveStart: (sessionId: string) => void = () => {};
    mockStartMoveFiles.mockImplementationOnce(
      () =>
        new Promise<string>((resolve) => {
          resolveStart = resolve;
        }),
    );
    const onEntriesMoved = vi.fn();
    function Harness() {
      const [entries, setEntries] = useState(sampleEntries);
      return (
        <FileList
          entries={entries}
          selectedPath="/music"
          isLoading={false}
          error={null}
          onEntriesMoved={(moved) => {
            onEntriesMoved(moved);
            setEntries((current) =>
              current.filter(
                (entry) => !moved.some((item) => item.oldId === entry.id),
              ),
            );
          }}
        />
      );
    }

    render(<Harness />);
    selectSongs();
    fireEvent.click(screen.getByRole("button", { name: "Move…" }));
    fireEvent.click(
      within(screen.getByRole("alertdialog")).getByRole("button", {
        name: "Choose folder…",
      }),
    );
    await waitFor(() => {
      expect(mockPickFolder).toHaveBeenCalled();
    });
    fireEvent.click(
      within(screen.getByRole("alertdialog")).getByRole("button", {
        name: "Move",
      }),
    );
    await waitFor(() => {
      expect(eventHandlers.has("browser:move-progress")).toBe(true);
    });

    // The worker can finish before the start IPC reply reaches the UI; the
    // buffered done event must still produce the summary (no stuck dialog).
    emitMoveProgress({
      session_id: "move-1",
      completed: 2,
      total: 2,
      done: true,
      results: [
        { path: "song1.mp3", new_path: "/library/song1.mp3", ok: true },
        { path: "song2.wav", new_path: "/library/song2.wav", ok: true },
      ],
    });
    act(() => {
      resolveStart("move-1");
    });

    await waitFor(() => {
      expect(
        within(screen.getByRole("alertdialog")).getByText("2 files moved."),
      ).toBeInTheDocument();
    });
    expect(onEntriesMoved).toHaveBeenCalledWith([
      { oldId: "song1.mp3", newId: "/library/song1.mp3" },
      { oldId: "song2.wav", newId: "/library/song2.wav" },
    ]);
    expect(screen.queryByText("song1.mp3")).not.toBeInTheDocument();
    expect(screen.queryByText("song2.wav")).not.toBeInTheDocument();
  });

  it("cancels a running batch through the backend", async () => {
    mockPickFolder.mockResolvedValueOnce("/library");
    mockStartMoveFiles.mockResolvedValueOnce("move-1");
    render(
      <FileList
        entries={sampleEntries}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );

    selectSongs();
    fireEvent.click(screen.getByRole("button", { name: "Move…" }));
    fireEvent.click(
      within(screen.getByRole("alertdialog")).getByRole("button", {
        name: "Choose folder…",
      }),
    );
    await waitFor(() => {
      expect(mockPickFolder).toHaveBeenCalled();
    });
    fireEvent.click(
      within(screen.getByRole("alertdialog")).getByRole("button", {
        name: "Move",
      }),
    );
    await waitFor(() => {
      expect(mockStartMoveFiles).toHaveBeenCalled();
    });

    fireEvent.click(
      within(screen.getByRole("alertdialog")).getByRole("button", {
        name: "Cancel",
      }),
    );
    expect(mockCancelMoveFiles).toHaveBeenCalledWith("move-1");
  });

  it("keeps the dialog open and shows the error when the start fails", async () => {
    mockPickFolder.mockResolvedValueOnce("/library");
    mockStartMoveFiles.mockRejectedValueOnce(
      new Error("That folder is unavailable."),
    );
    render(
      <FileList
        entries={sampleEntries}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );

    selectSongs();
    fireEvent.click(screen.getByRole("button", { name: "Move…" }));
    fireEvent.click(
      within(screen.getByRole("alertdialog")).getByRole("button", {
        name: "Choose folder…",
      }),
    );
    await waitFor(() => {
      expect(mockPickFolder).toHaveBeenCalled();
    });
    fireEvent.click(
      within(screen.getByRole("alertdialog")).getByRole("button", {
        name: "Move",
      }),
    );

    await waitFor(() => {
      expect(screen.getByRole("alert")).toHaveTextContent(
        "That folder is unavailable.",
      );
    });
    expect(screen.getByRole("alertdialog")).toBeInTheDocument();
  });
});

describe("FileList — reveal and open-with", () => {
  it("reveals the primary selected file", async () => {
    mockRevealFile.mockResolvedValueOnce(undefined);
    render(
      <FileList
        entries={[sampleEntries[0]]}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );

    fireEvent.click(screen.getByText("song1.mp3"));
    fireEvent.click(screen.getByRole("button", { name: "Reveal" }));

    await waitFor(() => {
      expect(mockRevealFile).toHaveBeenCalledWith("song1.mp3");
    });
  });

  it("opens the primary selected file with the default application", async () => {
    mockOpenWith.mockResolvedValueOnce(undefined);
    render(
      <FileList
        entries={[sampleEntries[0]]}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );

    fireEvent.click(screen.getByText("song1.mp3"));
    fireEvent.click(screen.getByRole("button", { name: "Open With…" }));

    await waitFor(() => {
      expect(mockOpenWith).toHaveBeenCalledWith("song1.mp3");
    });
  });

  it("shows the error when reveal fails", async () => {
    mockRevealFile.mockRejectedValueOnce(new Error("File is missing."));
    render(
      <FileList
        entries={[sampleEntries[0]]}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );

    fireEvent.click(screen.getByText("song1.mp3"));
    fireEvent.click(screen.getByRole("button", { name: "Reveal" }));

    await waitFor(() => {
      expect(screen.getByRole("alert")).toHaveTextContent("File is missing.");
    });
  });

  it("shows an error when open-with fails", async () => {
    mockOpenWith.mockRejectedValueOnce(new Error("No default application."));
    render(
      <FileList
        entries={[sampleEntries[0]]}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );

    fireEvent.click(screen.getByText("song1.mp3"));
    fireEvent.click(screen.getByRole("button", { name: "Open With…" }));

    await waitFor(() => {
      expect(screen.getByRole("alert")).toHaveTextContent(
        "No default application.",
      );
    });
  });

  it("disables the actions without a primary selection", () => {
    render(
      <FileList
        entries={[sampleEntries[0]]}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );

    expect(screen.getByRole("button", { name: "Reveal" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Open With…" })).toBeDisabled();
  });

  it("clears a stale error when the primary selection changes", async () => {
    mockRevealFile.mockRejectedValueOnce(new Error("File is missing."));
    render(
      <FileList
        entries={[sampleEntries[0], sampleEntries[1]]}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );

    fireEvent.click(screen.getByText("song1.mp3"));
    fireEvent.click(screen.getByRole("button", { name: "Reveal" }));

    await waitFor(() => {
      expect(screen.getByRole("alert")).toHaveTextContent("File is missing.");
    });

    fireEvent.click(screen.getByText("song2.wav"));

    await waitFor(() => {
      expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    });
  });
});

describe("FileList — drag-out", () => {
  beforeEach(() => {
    mockDragOut.mockReset();
    vi.unstubAllEnvs();
  });

  function makeDataTransfer() {
    const data: Record<string, string> = {};
    return {
      setData: vi.fn((type: string, value: string) => {
        data[type] = value;
      }),
      getData: (type: string) => data[type] ?? "",
      effectAllowed: "",
    };
  }

  function rowFor(name: string): Element {
    const cell = screen.getByText(name);
    const row = cell.closest('[role="row"]');
    if (!row) throw new Error(`row for ${name} not found`);
    return row;
  }

  function startNativeMouseDrag(name: string): void {
    const row = rowFor(name);
    fireEvent.mouseDown(row, {
      button: 0,
      buttons: 1,
      clientX: 10,
      clientY: 10,
    });
    fireEvent.mouseMove(row, {
      buttons: 0,
      clientX: 18,
      clientY: 10,
    });
  }

  function withPlatform(
    platform: string,
    run: () => Promise<void> | void,
  ): Promise<void> {
    const original = Object.getOwnPropertyDescriptor(navigator, "platform");
    Object.defineProperty(navigator, "platform", {
      value: platform,
      configurable: true,
    });
    return Promise.resolve()
      .then(run)
      .finally(() => {
        if (original) Object.defineProperty(navigator, "platform", original);
        else delete (navigator as { platform?: string }).platform;
      });
  }

  it("sets a text/uri-list payload for the dragged row", () => {
    return withPlatform("Linux x86_64", () => {
      render(
        <FileList
          entries={[sampleEntries[0]]}
          selectedPath="/music"
          isLoading={false}
          error={null}
        />,
      );
      const dt = makeDataTransfer();
      fireEvent.dragStart(rowFor("song1.mp3"), { dataTransfer: dt });
      expect(dt.setData).toHaveBeenCalledWith(
        "text/uri-list",
        "file://song1.mp3",
      );
      expect(mockDragOut).not.toHaveBeenCalled();
    });
  });

  it("converts an absolute Windows path to a valid file URI", () => {
    return withPlatform("Win32", () => {
      render(
        <FileList
          entries={[
            {
              id: "C:\\Music\\a b#1.wav",
              name: "a b#1.wav",
              kind: "playable",
            },
          ]}
          selectedPath="C:\\Music"
          isLoading={false}
          error={null}
        />,
      );
      const dt = makeDataTransfer();
      fireEvent.dragStart(rowFor("a b#1.wav"), { dataTransfer: dt });
      expect(dt.setData).toHaveBeenCalledWith(
        "text/uri-list",
        "file:///C:/Music/a%20b%231.wav",
      );
    });
  });

  it("converts a Windows UNC path to a valid file URI", () => {
    return withPlatform("Win32", () => {
      render(
        <FileList
          entries={[
            {
              id: "\\\\server\\share\\track.wav",
              name: "track.wav",
              kind: "playable",
            },
          ]}
          selectedPath="\\\\server\\share"
          isLoading={false}
          error={null}
        />,
      );
      const dt = makeDataTransfer();
      fireEvent.dragStart(rowFor("track.wav"), { dataTransfer: dt });
      expect(dt.setData).toHaveBeenCalledWith(
        "text/uri-list",
        "file://server/share/track.wav",
      );
    });
  });

  it("percent-encodes file URIs in the uri-list payload", () => {
    return withPlatform("Linux x86_64", () => {
      render(
        <FileList
          entries={[
            { id: "/music/a b#1.wav", name: "a b#1.wav", kind: "playable" },
          ]}
          selectedPath="/music"
          isLoading={false}
          error={null}
        />,
      );
      const dt = makeDataTransfer();
      fireEvent.dragStart(rowFor("a b#1.wav"), { dataTransfer: dt });
      expect(dt.setData).toHaveBeenCalledWith(
        "text/uri-list",
        "file:///music/a%20b%231.wav",
      );
    });
  });

  it("drags the whole selection when the dragged row is selected", () => {
    return withPlatform("Linux x86_64", () => {
      render(
        <FileList
          entries={sampleEntries}
          selectedPath="/music"
          isLoading={false}
          error={null}
        />,
      );
      fireEvent.click(screen.getByText("song1.mp3"));
      fireEvent.click(screen.getByText("song2.wav"), { ctrlKey: true });
      const dt = makeDataTransfer();
      fireEvent.dragStart(rowFor("song1.mp3"), { dataTransfer: dt });
      expect(dt.setData).toHaveBeenCalledWith(
        "text/uri-list",
        "file://song1.mp3\nfile://song2.wav",
      );
    });
  });

  it("drags only the dragged row when it is not part of the selection", () => {
    return withPlatform("Linux x86_64", () => {
      render(
        <FileList
          entries={sampleEntries}
          selectedPath="/music"
          isLoading={false}
          error={null}
        />,
      );
      fireEvent.click(screen.getByText("song1.mp3"));
      const dt = makeDataTransfer();
      fireEvent.dragStart(rowFor("song2.wav"), { dataTransfer: dt });
      expect(dt.setData).toHaveBeenCalledWith(
        "text/uri-list",
        "file://song2.wav",
      );
    });
  });

  it("invokes native drag-out on macOS without enabling HTML5 drag", async () => {
    mockDragOut.mockResolvedValueOnce(undefined);
    await withPlatform("MacIntel", () => {
      render(
        <FileList
          entries={[sampleEntries[0]]}
          selectedPath="/music"
          isLoading={false}
          error={null}
        />,
      );
      expect(rowFor("song1.mp3")).toHaveAttribute("draggable", "false");
      startNativeMouseDrag("song1.mp3");
      return waitFor(() => {
        expect(mockDragOut).toHaveBeenCalledWith(["song1.mp3"]);
      });
    });
  });

  it("uses the Tauri build platform when the webview hides macOS", async () => {
    vi.stubEnv("TAURI_ENV_PLATFORM", "darwin");
    mockDragOut.mockResolvedValueOnce(undefined);
    await withPlatform("Linux x86_64", () => {
      render(
        <FileList
          entries={[sampleEntries[0]]}
          selectedPath="/music"
          isLoading={false}
          error={null}
        />,
      );
      expect(rowFor("song1.mp3")).toHaveAttribute("draggable", "false");
      startNativeMouseDrag("song1.mp3");
      return waitFor(() => {
        expect(mockDragOut).toHaveBeenCalledWith(["song1.mp3"]);
      });
    });
  });

  it("starts macOS drag-out from mouse movement without an HTML drag", async () => {
    vi.stubEnv("TAURI_ENV_PLATFORM", "darwin");
    mockDragOut.mockResolvedValueOnce(undefined);
    render(
      <FileList
        entries={[sampleEntries[0]]}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );

    const row = rowFor("song1.mp3");
    expect(row).toHaveAttribute("draggable", "false");
    startNativeMouseDrag("song1.mp3");

    await waitFor(() => {
      expect(mockDragOut).toHaveBeenCalledWith(["song1.mp3"]);
    });
  });

  it("keeps a macOS click below the drag threshold as a normal click", () => {
    vi.stubEnv("TAURI_ENV_PLATFORM", "darwin");
    render(
      <FileList
        entries={[sampleEntries[0]]}
        selectedPath="/music"
        isLoading={false}
        error={null}
      />,
    );

    const row = rowFor("song1.mp3");
    fireEvent.mouseDown(row, {
      button: 0,
      buttons: 1,
      clientX: 10,
      clientY: 10,
    });
    fireEvent.mouseMove(row, { buttons: 0, clientX: 13, clientY: 10 });
    fireEvent.mouseUp(row, { button: 0, clientX: 13, clientY: 10 });

    expect(mockDragOut).not.toHaveBeenCalled();
  });

  it("passes the whole selection to the native macOS drag session", async () => {
    mockDragOut.mockResolvedValueOnce(undefined);
    await withPlatform("MacIntel", () => {
      render(
        <FileList
          entries={sampleEntries}
          selectedPath="/music"
          isLoading={false}
          error={null}
        />,
      );
      fireEvent.click(screen.getByText("song1.mp3"));
      fireEvent.click(screen.getByText("song2.wav"), { ctrlKey: true });
      startNativeMouseDrag("song1.mp3");
      return waitFor(() => {
        expect(mockDragOut).toHaveBeenCalledWith(["song1.mp3", "song2.wav"]);
      });
    });
  });

  it("clears the busy flag after the native drag-out resolves", async () => {
    mockDragOut.mockResolvedValueOnce(undefined);
    await withPlatform("MacIntel", () => {
      render(
        <FileList
          entries={[sampleEntries[0]]}
          selectedPath="/music"
          isLoading={false}
          error={null}
        />,
      );
      fireEvent.click(screen.getByText("song1.mp3"));
      startNativeMouseDrag("song1.mp3");
      return waitFor(() => {
        expect(mockDragOut).toHaveBeenCalled();
        expect(screen.getByRole("button", { name: "Reveal" })).toBeEnabled();
      });
    });
  });

  it("allows another drag immediately after a cancelled native drag", async () => {
    mockDragOut.mockRejectedValueOnce(new Error("Drag cancelled."));
    mockDragOut.mockResolvedValueOnce(undefined);
    await withPlatform("MacIntel", () => {
      render(
        <FileList
          entries={[sampleEntries[0]]}
          selectedPath="/music"
          isLoading={false}
          error={null}
        />,
      );
      startNativeMouseDrag("song1.mp3");
      return waitFor(() => {
        expect(screen.getByRole("alert")).toHaveTextContent("Drag cancelled.");
      }).then(async () => {
        startNativeMouseDrag("song1.mp3");
        await waitFor(() => {
          expect(mockDragOut).toHaveBeenCalledTimes(2);
        });
      });
    });
  });

  it("shows an error when the native drag-out fails", async () => {
    mockDragOut.mockRejectedValueOnce(new Error("File is missing."));
    await withPlatform("MacIntel", () => {
      render(
        <FileList
          entries={[sampleEntries[0]]}
          selectedPath="/music"
          isLoading={false}
          error={null}
        />,
      );
      startNativeMouseDrag("song1.mp3");
      return waitFor(() => {
        expect(screen.getByRole("alert")).toHaveTextContent("File is missing.");
      });
    });
  });
});
