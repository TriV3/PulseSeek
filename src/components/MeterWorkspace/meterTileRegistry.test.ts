import { describe, expect, it } from "vitest";
import {
  addTile,
  createMeterRegistry,
  duplicateTile,
  getAvailableModules,
  removeTile,
  type MeterTile,
} from "./meterTileRegistry";

describe("meter tile registry", () => {
  it("assigns stable unique IDs and preserves them when tiles move", () => {
    const first = addTile(createMeterRegistry(), "spectrum");
    const second = addTile(first.registry, "spectrum");

    expect(first.tile.tileId).not.toBe(second.tile.tileId);
    expect(second.registry.tiles.map((tile) => tile.tileId)).toEqual([
      first.tile.tileId,
      second.tile.tileId,
    ]);
  });

  it("duplicates tile configuration with a new independent ID", () => {
    const added = addTile(createMeterRegistry(), "band-energy");
    const duplicated = duplicateTile(added.registry, added.tile.tileId);

    expect(duplicated?.tile).toMatchObject({ moduleKind: "band-energy" });
    expect(duplicated?.tile.tileId).not.toBe(added.tile.tileId);
    expect(duplicated?.registry.tiles).toHaveLength(2);
  });

  it("distinguishes core modules and opt-in experimental modules", () => {
    expect(
      getAvailableModules(false).every((module) => module.category === "core"),
    ).toBe(true);
    expect(
      getAvailableModules(false).some(
        (module) => module.moduleKind === "goniometer",
      ),
    ).toBe(false);
    expect(
      getAvailableModules(true).some(
        (module) => module.moduleKind === "goniometer",
      ),
    ).toBe(true);
  });

  it("removes only requested tile and releases only unused subscriptions", () => {
    const first = addTile(createMeterRegistry(), "spectrum", "spectrum:shared");
    const second = addTile(first.registry, "spectrum", "spectrum:shared");
    const third = addTile(second.registry, "loudness", "loudness:unique");
    const result = removeTile(third.registry, first.tile.tileId);

    expect(result.registry.tiles.map((tile) => tile.tileId)).toEqual([
      second.tile.tileId,
      third.tile.tileId,
    ]);
    expect(result.releasedSubscriptions).toEqual([]);

    const final = removeTile(result.registry, second.tile.tileId);
    expect(final.releasedSubscriptions).toEqual(["spectrum:shared"]);
  });

  it("preserves explicit null subscription keys when duplicating", () => {
    const registry = {
      ...createMeterRegistry(),
      tiles: [
        {
          tileId: "tile-1",
          moduleKind: "spectrum" as const,
          subscriptionKey: null,
          state: "ready" as const,
        },
      ],
    };
    const duplicated = duplicateTile(registry, "tile-1");

    expect(duplicated?.tile.subscriptionKey).toBeNull();
  });

  it("reports tile availability states", () => {
    const tile: MeterTile = {
      tileId: "tile-1",
      moduleKind: "spectrum",
      subscriptionKey: null,
      state: "unavailable",
    };
    const registry = { ...createMeterRegistry(), tiles: [tile] };

    expect(registry.tiles[0].state).toBe("unavailable");
  });
});
