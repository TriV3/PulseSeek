import { describe, expect, it } from "vitest";
import type { BrowserEntry } from "../FolderTree/folderTreeTypes";
import { filterByFormat, formatOf } from "./fileFilter";

function playable(
  id: string,
  codec: string | null,
  overrides: Partial<BrowserEntry> = {},
): BrowserEntry {
  return {
    id,
    name: id,
    kind: "playable",
    metadata:
      codec === null
        ? null
        : {
            codec,
            duration_ms: null,
            size_bytes: null,
            modified_at_ms: null,
            channels: null,
            sample_rate: null,
            bit_depth: null,
          },
    ...overrides,
  };
}

function folder(id: string): BrowserEntry {
  return { id, name: id, kind: "folder", metadata: null };
}

describe("formatOf — filename extension mapping", () => {
  it("maps the WAV extension to the pcm format", () => {
    expect(formatOf(playable("kick.wav", "Unknown"))).toBe("pcm");
    expect(formatOf(playable("kick.wave", "Unknown"))).toBe("pcm");
  });

  it("maps the FLAC extension to the flac format", () => {
    expect(formatOf(playable("kit.flac", "Unknown"))).toBe("flac");
  });

  it("maps the MP3 extension to the mp3 format", () => {
    expect(formatOf(playable("loop.mp3", "Unknown"))).toBe("mp3");
  });

  it("matches extensions case-insensitively", () => {
    expect(formatOf(playable("loop.MP3", null))).toBe("mp3");
    expect(formatOf(playable("kit.FlAc", null))).toBe("flac");
  });

  it("returns null for unsupported extensions", () => {
    expect(formatOf(playable("mystery.ogg", "MP3"))).toBeNull();
    expect(formatOf(playable("installer.msi", "MP3"))).toBeNull();
    expect(formatOf(playable("disk.dmg", "MP3"))).toBeNull();
  });

  it("classifies by extension even when metadata claims another codec", () => {
    expect(formatOf(playable("misleading.mp3", "PCM"))).toBe("mp3");
  });

  it("returns null for folders", () => {
    expect(formatOf(folder("/music/kits"))).toBeNull();
  });
});

describe("filterByFormat — no active filter", () => {
  it("returns every entry unchanged for an empty format list", () => {
    const entries = [
      playable("a.mp3", "MP3"),
      playable("b.wav", "PCM"),
      playable("c.flac", "FLAC"),
      folder("/music/kits"),
    ];
    const filtered = filterByFormat(entries, []);
    expect(filtered).toEqual(entries);
    expect(filtered).not.toBe(entries);
  });
});

describe("filterByFormat — single filter", () => {
  it("keeps only playable entries whose decoded format matches", () => {
    const entries = [
      playable("a.mp3", "MP3"),
      playable("b.wav", "PCM"),
      playable("c.flac", "FLAC"),
    ];
    const filtered = filterByFormat(entries, ["mp3"]);
    expect(filtered.map((e) => e.id)).toEqual(["a.mp3"]);
  });

  it("keeps folders visible while filtering playable files", () => {
    const entries = [
      folder("/music/kits"),
      playable("a.mp3", "MP3"),
      playable("b.wav", "PCM"),
    ];
    const filtered = filterByFormat(entries, ["mp3"]);
    expect(filtered.map((e) => e.id)).toEqual(["/music/kits", "a.mp3"]);
  });
});

describe("filterByFormat — multiple filters", () => {
  it("keeps playable entries matching any selected format", () => {
    const entries = [
      playable("a.mp3", "MP3"),
      playable("b.wav", "PCM"),
      playable("c.flac", "FLAC"),
    ];
    const filtered = filterByFormat(entries, ["mp3", "flac"]);
    expect(filtered.map((e) => e.id)).toEqual(["a.mp3", "c.flac"]);
  });
});

describe("filterByFormat — unsupported extensions", () => {
  it("excludes unsupported extensions while a filter is active", () => {
    const entries = [
      playable("a.mp3", "MP3"),
      playable("mystery.ogg", "Unknown"),
      playable("preview.wav", null),
      playable("future.ogg", "ogg"),
    ];
    const filtered = filterByFormat(entries, ["mp3"]);
    expect(filtered.map((e) => e.id)).toEqual(["a.mp3"]);
  });

  it("keeps entries visible when no filter is active", () => {
    const entries = [
      playable("mystery.ogg", "Unknown"),
      playable("preview.wav", null),
    ];
    expect(filterByFormat(entries, [])).toEqual(entries);
  });
});

describe("filterByFormat — extension over decoder metadata", () => {
  it("classifies a .mp3-named file as MP3 for filtering", () => {
    const entries = [
      playable("misleading.mp3", "PCM"),
      playable("real.mp3", "MP3"),
    ];
    const pcm = filterByFormat(entries, ["pcm"]);
    expect(pcm.map((e) => e.id)).toEqual([]);
    const mp3 = filterByFormat(entries, ["mp3"]);
    expect(mp3.map((e) => e.id)).toEqual(["misleading.mp3", "real.mp3"]);
  });
});

describe("filterByFormat — non-mutation", () => {
  it("does not mutate the input array", () => {
    const entries = [playable("a.mp3", "MP3"), playable("b.wav", "PCM")];
    const snapshot = [...entries];
    filterByFormat(entries, ["mp3"]);
    expect(entries).toEqual(snapshot);
  });
});
