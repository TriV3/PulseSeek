import { describe, expect, it } from "vitest";
import type { BrowserEntry } from "../FolderTree/folderTreeTypes";
import { getFileExtension, sortFileEntries, type FileSort } from "./fileSort";

function entry(
  id: string,
  overrides: Partial<BrowserEntry> = {},
): BrowserEntry {
  return {
    id,
    name: id,
    kind: "playable",
    metadata: null,
    ...overrides,
  };
}

function withMetadata(
  base: BrowserEntry,
  metadata: Partial<NonNullable<BrowserEntry["metadata"]>>,
): BrowserEntry {
  return {
    ...base,
    metadata: {
      duration_ms: null,
      size_bytes: null,
      modified_at_ms: null,
      channels: null,
      sample_rate: null,
      bit_depth: null,
      codec: null,
      ...metadata,
    },
  };
}

describe("getFileExtension", () => {
  it("returns the lowercase extension from a name", () => {
    expect(getFileExtension("song.WAV")).toBe("wav");
  });

  it("returns empty string when there is no extension", () => {
    expect(getFileExtension("README")).toBe("");
  });

  it("handles dotfiles and names with multiple dots", () => {
    expect(getFileExtension(".hidden")).toBe("hidden");
    expect(getFileExtension("archive.tar.gz")).toBe("gz");
  });
});

describe("sortFileEntries — name and path (locale/Unicode)", () => {
  it("sorts names ascending with case-insensitive comparison", () => {
    const entries = [
      entry("b.mp3"),
      entry("A.mp3"),
      entry("a.wav"),
      entry("B.flac"),
    ];
    const sorted = sortFileEntries(entries, {
      field: "name",
      direction: "asc",
    });
    // Case-insensitive (base sensitivity): A/a sort together before b/B;
    // ties fall back to the remaining characters of the name.
    expect(sorted.map((e) => e.id)).toEqual([
      "A.mp3",
      "a.wav",
      "B.flac",
      "b.mp3",
    ]);
  });

  it("sorts names descending", () => {
    const entries = [entry("b.mp3"), entry("A.mp3"), entry("c.wav")];
    const sorted = sortFileEntries(entries, {
      field: "name",
      direction: "desc",
    });
    expect(sorted.map((e) => e.id)).toEqual(["c.wav", "b.mp3", "A.mp3"]);
  });

  it("compares Unicode names using locale rules", () => {
    const entries = [
      entry("éclair.mp3"),
      entry("zebra.mp3"),
      entry("ecole.mp3"),
    ];
    const sorted = sortFileEntries(entries, {
      field: "name",
      direction: "asc",
    });
    // "é" sorts near "e" under base sensitivity; within that group the
    // remaining characters decide ("éclair" < "ecole" because l < o).
    expect(sorted.map((e) => e.id)).toEqual([
      "éclair.mp3",
      "ecole.mp3",
      "zebra.mp3",
    ]);
  });

  it("sorts by full path when field is path", () => {
    const entries = [
      entry("/music/b/song.mp3"),
      entry("/music/a/song.mp3"),
      entry("/music/a/song2.mp3"),
    ];
    const sorted = sortFileEntries(entries, {
      field: "path",
      direction: "asc",
    });
    expect(sorted.map((e) => e.id)).toEqual([
      "/music/a/song.mp3",
      "/music/a/song2.mp3",
      "/music/b/song.mp3",
    ]);
  });

  it("sorts paths descending", () => {
    const entries = [entry("/music/a/song.mp3"), entry("/music/b/song.mp3")];
    const sorted = sortFileEntries(entries, {
      field: "path",
      direction: "desc",
    });
    expect(sorted.map((e) => e.id)).toEqual([
      "/music/b/song.mp3",
      "/music/a/song.mp3",
    ]);
  });
});

describe("sortFileEntries — duration, size, date", () => {
  it("sorts by duration ascending using metadata", () => {
    const entries = [
      withMetadata(entry("long.mp3"), { duration_ms: 300_000 }),
      withMetadata(entry("short.wav"), { duration_ms: 10_000 }),
      withMetadata(entry("medium.flac"), { duration_ms: 90_000 }),
    ];
    const sorted = sortFileEntries(entries, {
      field: "duration",
      direction: "asc",
    });
    expect(sorted.map((e) => e.id)).toEqual([
      "short.wav",
      "medium.flac",
      "long.mp3",
    ]);
  });

  it("sorts by duration descending", () => {
    const entries = [
      withMetadata(entry("long.mp3"), { duration_ms: 300_000 }),
      withMetadata(entry("short.wav"), { duration_ms: 10_000 }),
    ];
    const sorted = sortFileEntries(entries, {
      field: "duration",
      direction: "desc",
    });
    expect(sorted.map((e) => e.id)).toEqual(["long.mp3", "short.wav"]);
  });

  it("sorts by size ascending", () => {
    const entries = [
      withMetadata(entry("big.mp3"), { size_bytes: 9_000 }),
      withMetadata(entry("small.wav"), { size_bytes: 1_000 }),
      withMetadata(entry("tiny.flac"), { size_bytes: 100 }),
    ];
    const sorted = sortFileEntries(entries, {
      field: "size",
      direction: "asc",
    });
    expect(sorted.map((e) => e.id)).toEqual([
      "tiny.flac",
      "small.wav",
      "big.mp3",
    ]);
  });

  it("sorts by modification date ascending", () => {
    const entries = [
      withMetadata(entry("old.mp3"), { modified_at_ms: 1_000 }),
      withMetadata(entry("new.wav"), { modified_at_ms: 3_000 }),
      withMetadata(entry("mid.flac"), { modified_at_ms: 2_000 }),
    ];
    const sorted = sortFileEntries(entries, {
      field: "date",
      direction: "asc",
    });
    expect(sorted.map((e) => e.id)).toEqual(["old.mp3", "mid.flac", "new.wav"]);
  });
});

describe("sortFileEntries — type (extension)", () => {
  it("sorts by extension ascending", () => {
    const entries = [entry("song.wav"), entry("song.mp3"), entry("song.flac")];
    const sorted = sortFileEntries(entries, {
      field: "type",
      direction: "asc",
    });
    expect(sorted.map((e) => e.id)).toEqual([
      "song.flac",
      "song.mp3",
      "song.wav",
    ]);
  });

  it("sorts by extension descending", () => {
    const entries = [entry("song.wav"), entry("song.mp3")];
    const sorted = sortFileEntries(entries, {
      field: "type",
      direction: "desc",
    });
    expect(sorted.map((e) => e.id)).toEqual(["song.wav", "song.mp3"]);
  });

  it("is case-insensitive for extensions", () => {
    const entries = [entry("song.WAV"), entry("song.mp3")];
    const sorted = sortFileEntries(entries, {
      field: "type",
      direction: "asc",
    });
    expect(sorted.map((e) => e.id)).toEqual(["song.mp3", "song.WAV"]);
  });
});

describe("sortFileEntries — missing metadata and stability", () => {
  it("keeps entries with missing metadata last when ascending", () => {
    const entries = [
      withMetadata(entry("known.mp3"), { duration_ms: 50_000 }),
      entry("unknown.wav"),
      withMetadata(entry("other.flac"), { duration_ms: 20_000 }),
    ];
    const sorted = sortFileEntries(entries, {
      field: "duration",
      direction: "asc",
    });
    expect(sorted.map((e) => e.id)).toEqual([
      "other.flac",
      "known.mp3",
      "unknown.wav",
    ]);
  });

  it("keeps entries with missing metadata last when descending", () => {
    const entries = [
      withMetadata(entry("known.mp3"), { duration_ms: 50_000 }),
      entry("unknown.wav"),
      withMetadata(entry("other.flac"), { duration_ms: 20_000 }),
    ];
    const sorted = sortFileEntries(entries, {
      field: "duration",
      direction: "desc",
    });
    expect(sorted.map((e) => e.id)).toEqual([
      "known.mp3",
      "other.flac",
      "unknown.wav",
    ]);
  });

  it("falls back to full path order for equal keys regardless of input order", () => {
    // Input order is deliberately reversed: the result must not depend on it.
    const entries = [
      withMetadata(entry("/m/3.wav"), { duration_ms: 10_000 }),
      withMetadata(entry("/m/1.wav"), { duration_ms: 10_000 }),
      withMetadata(entry("/m/2.wav"), { duration_ms: 10_000 }),
    ];
    const sorted = sortFileEntries(entries, {
      field: "duration",
      direction: "asc",
    });
    expect(sorted.map((e) => e.id)).toEqual([
      "/m/1.wav",
      "/m/2.wav",
      "/m/3.wav",
    ]);
  });

  it("ties on equal names fall back to stable id order", () => {
    const entries = [
      { id: "/m/2.wav", name: "song.wav", kind: "playable" as const },
      { id: "/m/1.wav", name: "song.wav", kind: "playable" as const },
    ];
    const sorted = sortFileEntries(entries, {
      field: "name",
      direction: "asc",
    });
    expect(sorted.map((e) => e.id)).toEqual(["/m/1.wav", "/m/2.wav"]);
  });
});

describe("sortFileEntries — default export shape", () => {
  it("exports a FileSort type usable by callers", () => {
    const sort: FileSort = { field: "name", direction: "asc" };
    expect(sort.field).toBe("name");
    expect(sort.direction).toBe("asc");
  });
});
