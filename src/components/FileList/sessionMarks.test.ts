import { describe, expect, it } from "vitest";
import type { BrowserEntry } from "../FolderTree/folderTreeTypes";
import {
  filterByMark,
  markFiles,
  matchesMarkFilter,
  selectMarkedEntryIds,
  transferMarksByMetadata,
  unmarkFiles,
  type SessionMark,
} from "./sessionMarks";

const playable = (id: string): BrowserEntry => ({
  id,
  name: id,
  kind: "playable",
});

const metadata = (sizeBytes: number, modifiedAtMs: number) => ({
  duration_ms: null,
  size_bytes: sizeBytes,
  modified_at_ms: modifiedAtMs,
  channels: null,
  sample_rate: null,
  bit_depth: null,
  codec: null,
});

const folder = (id: string): BrowserEntry => ({
  id,
  name: id,
  kind: "folder",
});

describe("markFiles", () => {
  it("marks a single file", () => {
    const next = markFiles({}, ["a.mp3"], "keep");
    expect(next).toEqual({ "a.mp3": "keep" });
  });

  it("replaces an existing mark (marks are mutually exclusive)", () => {
    const next = markFiles({ "a.mp3": "keep" }, ["a.mp3"], "reject");
    expect(next).toEqual({ "a.mp3": "reject" });
  });

  it("marks every id in the selection", () => {
    const next = markFiles({ "a.mp3": "keep" }, ["b.wav", "c.flac"], "maybe");
    expect(next).toEqual({
      "a.mp3": "keep",
      "b.wav": "maybe",
      "c.flac": "maybe",
    });
  });

  it("does not mutate the input marks", () => {
    const input: Record<string, SessionMark> = { "a.mp3": "keep" };
    markFiles(input, ["a.mp3"], "maybe");
    expect(input).toEqual({ "a.mp3": "keep" });
  });
});

describe("unmarkFiles", () => {
  it("removes the mark from every requested id", () => {
    const next = unmarkFiles({ "a.mp3": "keep", "b.wav": "maybe" }, [
      "a.mp3",
      "missing.flac",
    ]);
    expect(next).toEqual({ "b.wav": "maybe" });
  });

  it("does not mutate the input marks", () => {
    const input: Record<string, SessionMark> = { "a.mp3": "keep" };
    unmarkFiles(input, ["a.mp3"]);
    expect(input).toEqual({ "a.mp3": "keep" });
  });
});

describe("matchesMarkFilter", () => {
  it("matches every entry under the all filter", () => {
    expect(matchesMarkFilter(undefined, "all")).toBe(true);
    expect(matchesMarkFilter("keep", "all")).toBe(true);
  });

  it("matches any marked entry under the marked filter", () => {
    expect(matchesMarkFilter("favorite", "marked")).toBe(true);
    expect(matchesMarkFilter(undefined, "marked")).toBe(false);
  });

  it("matches only the requested mark", () => {
    expect(matchesMarkFilter("keep", "keep")).toBe(true);
    expect(matchesMarkFilter("maybe", "keep")).toBe(false);
    expect(matchesMarkFilter(undefined, "keep")).toBe(false);
  });
});

describe("filterByMark", () => {
  const entries = [
    folder("folder-1"),
    playable("a.mp3"),
    playable("b.wav"),
    playable("c.flac"),
  ];
  const marks = { "a.mp3": "keep", "c.flac": "reject" } as const;

  it("keeps every entry under the all filter", () => {
    expect(filterByMark(entries, marks, "all")).toEqual(entries);
  });

  it("keeps folders plus playable entries with the requested mark", () => {
    expect(filterByMark(entries, marks, "keep")).toEqual([
      folder("folder-1"),
      playable("a.mp3"),
    ]);
  });

  it("keeps folders plus every marked playable entry under marked", () => {
    expect(filterByMark(entries, marks, "marked")).toEqual([
      folder("folder-1"),
      playable("a.mp3"),
      playable("c.flac"),
    ]);
  });

  it("keeps folders only when no playable entry matches", () => {
    expect(filterByMark(entries, marks, "favorite")).toEqual([
      folder("folder-1"),
    ]);
  });
});

describe("selectMarkedEntryIds", () => {
  it("returns only playable ids that carry a mark", () => {
    const entries = [folder("folder-1"), playable("a.mp3"), playable("b.wav")];
    const marks = { "a.mp3": "keep" } as const;
    expect(selectMarkedEntryIds(entries, marks)).toEqual(["a.mp3"]);
  });

  it("returns an empty list when nothing is marked", () => {
    expect(selectMarkedEntryIds([playable("a.mp3")], {})).toEqual([]);
  });
});

describe("transferMarksByMetadata", () => {
  it("transfers a mark to the unique renamed match in the same directory", () => {
    const previous = [
      {
        ...playable("/music/a.mp3"),
        metadata: metadata(2048, 1_700_000_000_000),
      },
    ];
    const next = [
      {
        ...playable("/music/b.mp3"),
        metadata: metadata(2048, 1_700_000_000_000),
      },
    ];
    const marks = { "/music/a.mp3": "keep" } as const;

    expect(transferMarksByMetadata(marks, previous, next)).toEqual({
      "/music/b.mp3": "keep",
    });
  });

  it("keeps the mark untouched when the entry still exists", () => {
    const entry = {
      ...playable("/music/a.mp3"),
      metadata: metadata(2048, 1_700_000_000_000),
    };
    const marks = { "/music/a.mp3": "keep" } as const;

    expect(transferMarksByMetadata(marks, [entry], [entry])).toBe(marks);
  });

  it("does not transfer when the renamed entry is missing metadata", () => {
    const previous = [
      {
        ...playable("/music/a.mp3"),
        metadata: metadata(2048, 1_700_000_000_000),
      },
    ];
    const next = [playable("/music/b.mp3")];
    const marks = { "/music/a.mp3": "keep" } as const;

    expect(transferMarksByMetadata(marks, previous, next)).toBe(marks);
  });

  it("does not transfer across directories", () => {
    const previous = [
      {
        ...playable("/music/a.mp3"),
        metadata: metadata(2048, 1_700_000_000_000),
      },
    ];
    const next = [
      {
        ...playable("/music/sub/b.mp3"),
        metadata: metadata(2048, 1_700_000_000_000),
      },
    ];
    const marks = { "/music/a.mp3": "keep" } as const;

    expect(transferMarksByMetadata(marks, previous, next)).toBe(marks);
  });

  it("does not transfer when several entries share the identity", () => {
    const previous = [
      {
        ...playable("/music/a.mp3"),
        metadata: metadata(2048, 1_700_000_000_000),
      },
    ];
    const next = [
      {
        ...playable("/music/b.mp3"),
        metadata: metadata(2048, 1_700_000_000_000),
      },
      {
        ...playable("/music/c.mp3"),
        metadata: metadata(2048, 1_700_000_000_000),
      },
    ];
    const marks = { "/music/a.mp3": "keep" } as const;

    expect(transferMarksByMetadata(marks, previous, next)).toBe(marks);
  });

  it("does not transfer to an entry that already carries a mark", () => {
    const previous = [
      {
        ...playable("/music/a.mp3"),
        metadata: metadata(2048, 1_700_000_000_000),
      },
    ];
    const next = [
      {
        ...playable("/music/b.mp3"),
        metadata: metadata(2048, 1_700_000_000_000),
      },
    ];
    const marks = {
      "/music/a.mp3": "keep",
      "/music/b.mp3": "favorite",
    } as const;

    expect(transferMarksByMetadata(marks, previous, next)).toBe(marks);
  });

  it("leaves marks for unrelated changes alone", () => {
    const previous = [
      {
        ...playable("/music/a.mp3"),
        metadata: metadata(2048, 1_700_000_000_000),
      },
    ];
    const next = [
      {
        ...playable("/music/a.mp3"),
        metadata: metadata(2048, 1_700_000_000_000),
      },
      {
        ...playable("/music/new.wav"),
        metadata: metadata(4096, 1_700_000_000_001),
      },
    ];
    const marks = { "/music/a.mp3": "maybe" } as const;

    expect(transferMarksByMetadata(marks, previous, next)).toBe(marks);
  });
});
