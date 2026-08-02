import { describe, expect, it } from "vitest";
import type { BrowserEntry } from "../FolderTree/folderTreeTypes";
import { filterFileEntries } from "./fileSearch";

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

describe("filterFileEntries — empty query", () => {
  it("returns every entry unchanged for an empty query", () => {
    const entries = [entry("a.mp3"), entry("b.wav"), entry("c.flac")];
    const filtered = filterFileEntries(entries, "");
    expect(filtered).toEqual(entries);
    expect(filtered).not.toBe(entries);
  });

  it("returns every entry for a whitespace-only query", () => {
    const entries = [entry("a.mp3")];
    expect(filterFileEntries(entries, "   ")).toEqual(entries);
  });
});

describe("filterFileEntries — case", () => {
  it("matches names case-insensitively", () => {
    const entries = [entry("Drum Loop.mp3"), entry("bass.wav")];
    const filtered = filterFileEntries(entries, "drum");
    expect(filtered.map((e) => e.id)).toEqual(["Drum Loop.mp3"]);
  });

  it("matches query typed in the other case", () => {
    const entries = [entry("kick.wav"), entry("snare.wav")];
    const filtered = filterFileEntries(entries, "KICK");
    expect(filtered.map((e) => e.id)).toEqual(["kick.wav"]);
  });
});

describe("filterFileEntries — Unicode", () => {
  it("matches accented names with an accented query", () => {
    const entries = [entry("éclair.mp3"), entry("ecole.wav")];
    const filtered = filterFileEntries(entries, "éclair");
    expect(filtered.map((e) => e.id)).toEqual(["éclair.mp3"]);
  });

  it("matches accented names ignoring case", () => {
    const entries = [entry("Éclair.mp3"), entry("ecole.wav")];
    const filtered = filterFileEntries(entries, "éclair");
    expect(filtered.map((e) => e.id)).toEqual(["Éclair.mp3"]);
  });

  it("matches non-latin names", () => {
    const entries = [entry("サンプル.wav"), entry("sample.mp3")];
    const filtered = filterFileEntries(entries, "ンプ");
    expect(filtered.map((e) => e.id)).toEqual(["サンプル.wav"]);
  });
});

describe("filterFileEntries — path", () => {
  it("matches the full path in addition to the name", () => {
    // Names deliberately differ from paths so the match must come from the
    // full path, not the file name.
    const entries = [
      entry("/music/kicks/fat.wav", { name: "fat.wav" }),
      entry("/music/snare.wav", { name: "snare.wav" }),
    ];
    const filtered = filterFileEntries(entries, "kicks");
    expect(filtered.map((e) => e.id)).toEqual(["/music/kicks/fat.wav"]);
  });

  it("matches a parent directory path", () => {
    const entries = [
      entry("/samples/loops/beat.mp3", { name: "beat.mp3" }),
      entry("/samples/one-shots/hit.wav", { name: "hit.wav" }),
    ];
    const filtered = filterFileEntries(entries, "/samples/one-shots");
    expect(filtered.map((e) => e.id)).toEqual(["/samples/one-shots/hit.wav"]);
  });
});

describe("filterFileEntries — no results and non-mutation", () => {
  it("returns an empty array when nothing matches", () => {
    const entries = [entry("a.mp3"), entry("b.wav")];
    expect(filterFileEntries(entries, "zzz")).toEqual([]);
  });

  it("does not mutate the input array", () => {
    const entries = [entry("kick.wav"), entry("snare.wav")];
    const snapshot = [...entries];
    filterFileEntries(entries, "kick");
    expect(entries).toEqual(snapshot);
  });
});
