import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { FileList } from "./FileList";
import type { BrowserEntry } from "../FolderTree/folderTreeTypes";

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
