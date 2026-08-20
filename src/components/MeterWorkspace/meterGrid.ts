export const MIN_TILE_WIDTH = 220;
export const MAX_TILE_WIDTH = 720;
export const DEFAULT_TILE_WIDTH = 320;
export const MIN_TILE_HEIGHT = 160;
export const MAX_TILE_HEIGHT = 640;
export const DEFAULT_TILE_HEIGHT = 240;

export type GridTile = {
  tileId: string;
  width: number;
  height: number;
  maximized: boolean;
  previousIndex: number | null;
};

export type MeterGrid = { tiles: GridTile[] };

export const clampTileSize = ({
  width,
  height,
}: {
  width: number;
  height: number;
}) => ({
  width: Math.min(MAX_TILE_WIDTH, Math.max(MIN_TILE_WIDTH, width)),
  height: Math.min(MAX_TILE_HEIGHT, Math.max(MIN_TILE_HEIGHT, height)),
});

export const createMeterGrid = (tileIds: string[]): MeterGrid => ({
  tiles: tileIds.map((tileId) => ({
    tileId,
    width: DEFAULT_TILE_WIDTH,
    height: DEFAULT_TILE_HEIGHT,
    maximized: false,
    previousIndex: null,
  })),
});

const withTile = (
  grid: MeterGrid,
  tileId: string,
  update: (tile: GridTile, index: number) => GridTile,
): MeterGrid => ({
  tiles: grid.tiles.map((tile, index) =>
    tile.tileId === tileId ? update(tile, index) : tile,
  ),
});

export const resizeGridToBounds = (
  grid: MeterGrid,
  maxWidth: number,
  maxHeight: number,
): MeterGrid => ({
  tiles: grid.tiles.map((tile) => ({
    ...tile,
    width: Math.min(tile.width, Math.max(MIN_TILE_WIDTH, maxWidth)),
    height: Math.min(tile.height, Math.max(MIN_TILE_HEIGHT, maxHeight)),
  })),
});

export const resizeTile = (
  grid: MeterGrid,
  tileId: string,
  width: number,
  height: number,
): MeterGrid => {
  const size = clampTileSize({ width, height });
  return withTile(grid, tileId, (tile) => ({ ...tile, ...size }));
};

export const moveTile = (
  grid: MeterGrid,
  tileId: string,
  offset: number,
): MeterGrid => {
  const index = grid.tiles.findIndex((tile) => tile.tileId === tileId);
  if (index < 0) return grid;
  const target = Math.min(grid.tiles.length - 1, Math.max(0, index + offset));
  if (target === index) return grid;
  const tiles = [...grid.tiles];
  const [tile] = tiles.splice(index, 1);
  tiles.splice(target, 0, tile);
  return { tiles };
};

export const maximizeTile = (grid: MeterGrid, tileId: string): MeterGrid =>
  withTile(grid, tileId, (tile, index) => ({
    ...tile,
    maximized: true,
    previousIndex: tile.maximized ? tile.previousIndex : index,
  }));

export const restoreTile = (grid: MeterGrid, tileId: string): MeterGrid => {
  const tile = grid.tiles.find((item) => item.tileId === tileId);
  if (!tile || !tile.maximized) return grid;
  const restored = withTile(grid, tileId, (item) => ({
    ...item,
    maximized: false,
    previousIndex: null,
  }));
  if (tile.previousIndex === null) return restored;
  const currentIndex = restored.tiles.findIndex(
    (item) => item.tileId === tileId,
  );
  const target = Math.min(restored.tiles.length - 1, tile.previousIndex);
  const tiles = [...restored.tiles];
  const [item] = tiles.splice(currentIndex, 1);
  tiles.splice(target, 0, item);
  return { tiles };
};
