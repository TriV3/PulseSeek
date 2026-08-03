import { describe, expect, it } from "vitest";
import type { BrowserEntry } from "../FolderTree/folderTreeTypes";
import {
  filterByMark,
  markFiles,
  matchesMarkFilter,
  selectMarkedEntryIds,
  unmarkFiles,
  type SessionMark,
} from "./sessionMarks";

const playable = (id: string): BrowserEntry => ({
  id,
  name: id,
  kind: "playable",
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
