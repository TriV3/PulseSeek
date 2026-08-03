import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
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

const mockMoveToTrash = vi.hoisted(() => vi.fn());

vi.mock("../../api/commandEnvelope", () => ({
  moveToTrash: mockMoveToTrash,
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
