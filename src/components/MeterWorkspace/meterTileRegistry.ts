export type MeterModuleKind =
  | "spectrum"
  | "band-energy"
  | "colored-waveform"
  | "spectrogram"
  | "loudness"
  | "true-peak"
  | "stereo"
  | "diagnostics"
  | "goniometer";

export type TileState =
  "loading" | "ready" | "unavailable" | "incomplete" | "degraded" | "error";

export type MeterModule = {
  moduleKind: MeterModuleKind;
  title: string;
  category: "core" | "experimental";
};

export type MeterTile = {
  tileId: string;
  moduleKind: MeterModuleKind;
  subscriptionKey: string | null;
  state: TileState;
};

export type MeterRegistry = {
  tiles: MeterTile[];
  nextTileNumber: number;
};

export const METER_MODULES: readonly MeterModule[] = [
  { moduleKind: "spectrum", title: "Spectrum Analyzer", category: "core" },
  { moduleKind: "band-energy", title: "Band Energy", category: "core" },
  {
    moduleKind: "colored-waveform",
    title: "Colored Waveform",
    category: "core",
  },
  { moduleKind: "spectrogram", title: "Spectrogram", category: "core" },
  { moduleKind: "loudness", title: "Loudness", category: "core" },
  { moduleKind: "true-peak", title: "True Peak", category: "core" },
  { moduleKind: "stereo", title: "Stereo", category: "core" },
  { moduleKind: "diagnostics", title: "Diagnostics", category: "core" },
  { moduleKind: "goniometer", title: "Goniometer", category: "experimental" },
];

export const createMeterRegistry = (): MeterRegistry => ({
  tiles: [],
  nextTileNumber: 1,
});

export const getAvailableModules = (
  experimentalEnabled: boolean,
): MeterModule[] =>
  METER_MODULES.filter(
    (module) => experimentalEnabled || module.category === "core",
  );

const defaultSubscriptionKey = (moduleKind: MeterModuleKind) => moduleKind;

export const addTile = (
  registry: MeterRegistry,
  moduleKind: MeterModuleKind,
  subscriptionKey?: string | null,
): { registry: MeterRegistry; tile: MeterTile } => {
  const resolvedSubscriptionKey =
    subscriptionKey === undefined
      ? defaultSubscriptionKey(moduleKind)
      : subscriptionKey;
  const tile: MeterTile = {
    tileId: `meter-tile-${registry.nextTileNumber}`,
    moduleKind,
    subscriptionKey: resolvedSubscriptionKey,
    state: "loading",
  };
  return {
    tile,
    registry: {
      tiles: [...registry.tiles, tile],
      nextTileNumber: registry.nextTileNumber + 1,
    },
  };
};

export const duplicateTile = (
  registry: MeterRegistry,
  tileId: string,
): { registry: MeterRegistry; tile: MeterTile } | null => {
  const source = registry.tiles.find((tile) => tile.tileId === tileId);
  if (!source) return null;
  return addTile(registry, source.moduleKind, source.subscriptionKey);
};

export const updateTileState = (
  registry: MeterRegistry,
  tileId: string,
  state: TileState,
): MeterRegistry => ({
  ...registry,
  tiles: registry.tiles.map((tile) =>
    tile.tileId === tileId ? { ...tile, state } : tile,
  ),
});

export const removeTile = (
  registry: MeterRegistry,
  tileId: string,
): { registry: MeterRegistry; releasedSubscriptions: string[] } => {
  const removed = registry.tiles.find((tile) => tile.tileId === tileId);
  if (!removed) return { registry, releasedSubscriptions: [] };
  const tiles = registry.tiles.filter((tile) => tile.tileId !== tileId);
  const stillUsed = new Set(tiles.map((tile) => tile.subscriptionKey));
  const releasedSubscriptions =
    removed.subscriptionKey && !stillUsed.has(removed.subscriptionKey)
      ? [removed.subscriptionKey]
      : [];
  return {
    registry: { ...registry, tiles },
    releasedSubscriptions,
  };
};
