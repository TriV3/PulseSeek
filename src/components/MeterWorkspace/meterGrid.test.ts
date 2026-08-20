import { describe, expect, it } from "vitest";
import {
  DEFAULT_TILE_HEIGHT,
  DEFAULT_TILE_WIDTH,
  MAX_TILE_HEIGHT,
  MAX_TILE_WIDTH,
  MIN_TILE_HEIGHT,
  MIN_TILE_WIDTH,
  clampTileSize,
  createMeterGrid,
  maximizeTile,
  moveTile,
  restoreTile,
  resizeTile,
  resizeGridToBounds,
  type MeterGrid,
} from "./meterGrid";

describe("meter grid", () => {
  it("keeps tile sizes within bounds", () => {
    const grid = createMeterGrid(["one"]);
    expect(grid.tiles[0]).toMatchObject({
      width: DEFAULT_TILE_WIDTH,
      height: DEFAULT_TILE_HEIGHT,
    });
    expect(clampTileSize({ width: 1, height: 9999 })).toEqual({
      width: MIN_TILE_WIDTH,
      height: MAX_TILE_HEIGHT,
    });
    expect(resizeTile(grid, "one", 1, 9999).tiles[0]).toMatchObject({
      width: MIN_TILE_WIDTH,
      height: MAX_TILE_HEIGHT,
    });
  });

  it("moves tiles immutably and restores maximized position", () => {
    const grid = createMeterGrid(["one", "two", "three"]);
    const moved = moveTile(grid, "three", -2);
    expect(moved.tiles.map((tile) => tile.tileId)).toEqual([
      "three",
      "one",
      "two",
    ]);
    const maximized = maximizeTile(moved, "one");
    expect(maximized.tiles.find((tile) => tile.tileId === "one")).toMatchObject(
      {
        maximized: true,
      },
    );
    const restored = restoreTile(maximized, "one");
    expect(restored.tiles.map((tile) => tile.tileId)).toEqual([
      "three",
      "one",
      "two",
    ]);
    expect(restored.tiles.find((tile) => tile.tileId === "one")).toMatchObject({
      maximized: false,
    });
  });

  it("clamps grid after container resize", () => {
    const grid = resizeTile(createMeterGrid(["one"]), "one", 720, 640);
    expect(resizeGridToBounds(grid, 300, 180).tiles[0]).toMatchObject({
      width: 300,
      height: 180,
    });
  });

  it("defines usable bounds", () => {
    expect(MIN_TILE_WIDTH).toBeLessThan(DEFAULT_TILE_WIDTH);
    expect(DEFAULT_TILE_WIDTH).toBeLessThan(MAX_TILE_WIDTH);
    expect(MIN_TILE_HEIGHT).toBeLessThan(DEFAULT_TILE_HEIGHT);
    expect(DEFAULT_TILE_HEIGHT).toBeLessThan(MAX_TILE_HEIGHT);
  });
});

export const gridForTest = (): MeterGrid => createMeterGrid([]);
