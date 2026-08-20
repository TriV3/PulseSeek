import { useEffect, useRef, useState } from "react";
import {
  addTile,
  createMeterRegistry,
  duplicateTile,
  getAvailableModules,
  removeTile,
  updateTileState,
  type MeterModuleKind,
  type MeterRegistry,
  type TileState,
} from "./meterTileRegistry";
import {
  createMeterGrid,
  maximizeTile,
  moveTile,
  resizeGridToBounds,
  resizeTile,
  restoreTile,
  type MeterGrid,
} from "./meterGrid";

const stateLabels: Record<TileState, string> = {
  loading: "Loading",
  ready: "Ready",
  unavailable: "Unavailable",
  incomplete: "Incomplete",
  degraded: "Degraded",
  error: "Error",
};

type MeterWorkspaceProps = {
  onSubscribe?: (subscriptionKey: string) => void | (() => void);
};

export function MeterWorkspace({ onSubscribe }: MeterWorkspaceProps) {
  const [registry, setRegistry] = useState<MeterRegistry>(createMeterRegistry);
  const [grid, setGrid] = useState<MeterGrid>(() => createMeterGrid([]));
  const [experimentalEnabled, setExperimentalEnabled] = useState(false);
  const [focusedTileId, setFocusedTileId] = useState<string | null>(null);
  const subscriptions = useRef(
    new Map<string, { count: number; cleanup: (() => void) | undefined }>(),
  );
  const gridElement = useRef<HTMLDivElement>(null);
  const tileRefs = useRef(new Map<string, HTMLElement>());
  const keyboardFocus = useRef<string | null>(null);
  const resizeObserver = useRef<Pick<ResizeObserver, "disconnect"> | null>(
    null,
  );
  const subscribeRef = useRef(onSubscribe);
  useEffect(() => {
    subscribeRef.current = onSubscribe;
  }, [onSubscribe]);
  const modules = getAvailableModules(experimentalEnabled);

  useEffect(() => {
    const resize = () => {
      const bounds = gridElement.current?.getBoundingClientRect();
      if (bounds)
        setGrid((current) =>
          resizeGridToBounds(current, bounds.width, bounds.height),
        );
    };
    window.addEventListener("resize", resize);
    const observer =
      typeof ResizeObserver === "undefined" ? null : new ResizeObserver(resize);
    resizeObserver.current = observer;
    if (observer && gridElement.current) observer.observe(gridElement.current);
    resize();
    return () => {
      window.removeEventListener("resize", resize);
      resizeObserver.current?.disconnect();
      resizeObserver.current = null;
    };
  }, [registry.tiles.length]);

  useEffect(
    () => () => {
      subscriptions.current.forEach((subscription) => subscription.cleanup?.());
      subscriptions.current.clear();
    },
    [],
  );

  const addModule = (moduleKind: MeterModuleKind) => {
    const result = addTile(registry, moduleKind);
    setRegistry(result.registry);
    setGrid((currentGrid) => ({
      tiles: [
        ...currentGrid.tiles,
        ...createMeterGrid([result.tile.tileId]).tiles,
      ],
    }));
    const key = result.tile.subscriptionKey ?? moduleKind;
    const existing = subscriptions.current.get(key);
    if (existing) existing.count += 1;
    else
      subscriptions.current.set(key, {
        count: 1,
        cleanup: subscribeRef.current?.(key) ?? undefined,
      });
  };

  const remove = (tileId: string) => {
    const result = removeTile(registry, tileId);
    const key = registry.tiles.find(
      (tile) => tile.tileId === tileId,
    )?.subscriptionKey;
    for (const releasedKey of result.releasedSubscriptions) {
      subscriptions.current.get(releasedKey)?.cleanup?.();
      subscriptions.current.delete(releasedKey);
    }
    if (key && result.releasedSubscriptions.length === 0) {
      const subscription = subscriptions.current.get(key);
      if (subscription) subscription.count -= 1;
    }
    setRegistry(result.registry);
    setGrid((current) => ({
      tiles: current.tiles.filter((tile) => tile.tileId !== tileId),
    }));
    setFocusedTileId(null);
  };

  return (
    <div className="meter-workspace-content">
      <header className="meter-workspace-header">
        <div>
          <h2>Meters</h2>
          <p>Choose measurement tiles for current playback session.</p>
        </div>
        <label className="meter-experimental-toggle">
          <input
            type="checkbox"
            checked={experimentalEnabled}
            onChange={(event) => setExperimentalEnabled(event.target.checked)}
          />
          Show experimental modules
        </label>
      </header>
      <section
        className="meter-module-catalogue"
        aria-label="Meter module catalogue"
      >
        <h3>Add meter tile</h3>
        <div className="meter-module-list">
          {modules.map((module) => (
            <button
              key={module.moduleKind}
              type="button"
              onClick={() => addModule(module.moduleKind)}
              title={
                module.category === "experimental"
                  ? "Experimental module"
                  : undefined
              }
            >
              {module.title}
              {module.category === "experimental" ? " (Experimental)" : ""}
            </button>
          ))}
        </div>
      </section>
      {registry.tiles.length === 0 ? (
        <div className="meters-empty-state">
          <h2>No meter tiles</h2>
          <p>Add module above to start building Meters workspace.</p>
        </div>
      ) : (
        <section
          ref={gridElement}
          className="meter-tile-list"
          aria-label="Meter tiles"
        >
          {grid.tiles.flatMap((layoutTile) => {
            const tile = registry.tiles.find(
              (item) => item.tileId === layoutTile.tileId,
            );
            if (!tile) return [];
            const module = modules.find(
              (item) => item.moduleKind === tile.moduleKind,
            );
            const layout = grid.tiles.find(
              (item) => item.tileId === tile.tileId,
            );
            const title = module?.title ?? tile.moduleKind;
            return [
              <article
                className="meter-tile"
                ref={(element) => {
                  if (element) tileRefs.current.set(tile.tileId, element);
                  else tileRefs.current.delete(tile.tileId);
                }}
                key={tile.tileId}
                aria-label={title}
                tabIndex={
                  focusedTileId === null || focusedTileId === tile.tileId
                    ? 0
                    : -1
                }
                onFocus={() => {
                  keyboardFocus.current = tile.tileId;
                  setFocusedTileId(tile.tileId);
                }}
                onKeyDown={(event) => {
                  if (event.target !== event.currentTarget) return;
                  const tiles = grid.tiles.flatMap((item) => {
                    const element = tileRefs.current.get(item.tileId);
                    return element ? [element] : [];
                  });
                  const index = tiles.indexOf(event.currentTarget);
                  const next =
                    event.key === "ArrowRight" || event.key === "ArrowDown"
                      ? index + 1
                      : event.key === "ArrowLeft" || event.key === "ArrowUp"
                        ? index - 1
                        : index;
                  if (next !== index) {
                    if (event.shiftKey) {
                      setGrid((current) =>
                        moveTile(current, tile.tileId, next - index),
                      );
                    } else {
                      tiles[next]?.focus();
                    }
                    event.preventDefault();
                  }
                }}
                data-maximized={layout?.maximized ?? false}
                data-width={layout?.width}
                style={{
                  width: layout?.maximized ? "100%" : layout?.width,
                  minHeight: layout?.height,
                }}
              >
                <header>
                  <h3>{title}</h3>
                  <span role="status">{stateLabels[tile.state]}</span>
                </header>
                <p>Tile {tile.tileId}</p>
                <label>
                  State
                  <select
                    aria-label={`${title} state`}
                    value={tile.state}
                    onChange={(event) =>
                      setRegistry((current) =>
                        updateTileState(
                          current,
                          tile.tileId,
                          event.target.value as TileState,
                        ),
                      )
                    }
                  >
                    {Object.keys(stateLabels).map((state) => (
                      <option key={state} value={state}>
                        {stateLabels[state as TileState]}
                      </option>
                    ))}
                  </select>
                </label>
                <div>
                  <button
                    type="button"
                    aria-label={`Reorder ${title}`}
                    onClick={() =>
                      setGrid((current) => moveTile(current, tile.tileId, 1))
                    }
                  >
                    Reorder
                  </button>
                  <button
                    type="button"
                    aria-label={`Decrease ${title} width`}
                    onClick={() =>
                      setGrid((current) =>
                        resizeTile(
                          current,
                          tile.tileId,
                          (layout?.width ?? 320) - 20,
                          layout?.height ?? 240,
                        ),
                      )
                    }
                  >
                    Width -
                  </button>
                  <button
                    type="button"
                    aria-label={`Decrease ${title} height`}
                    onClick={() =>
                      setGrid((current) =>
                        resizeTile(
                          current,
                          tile.tileId,
                          layout?.width ?? 320,
                          (layout?.height ?? 240) - 20,
                        ),
                      )
                    }
                  >
                    Height -
                  </button>
                  <button
                    type="button"
                    aria-label={`Increase ${title} height`}
                    onClick={() =>
                      setGrid((current) =>
                        resizeTile(
                          current,
                          tile.tileId,
                          layout?.width ?? 320,
                          (layout?.height ?? 240) + 20,
                        ),
                      )
                    }
                  >
                    Height +
                  </button>
                  <button
                    type="button"
                    aria-label={`Increase ${title} width`}
                    onClick={() =>
                      setGrid((current) =>
                        resizeTile(
                          current,
                          tile.tileId,
                          (layout?.width ?? 320) + 40,
                          layout?.height ?? 240,
                        ),
                      )
                    }
                  >
                    Width +
                  </button>
                  <button
                    type="button"
                    aria-label={
                      layout?.maximized
                        ? `Restore ${title}`
                        : `Maximize ${title}`
                    }
                    onClick={() =>
                      setGrid((current) =>
                        layout?.maximized
                          ? restoreTile(current, tile.tileId)
                          : maximizeTile(current, tile.tileId),
                      )
                    }
                  >
                    {layout?.maximized ? "Restore" : "Maximize"}
                  </button>
                  <button
                    type="button"
                    aria-label={`Duplicate ${title}`}
                    onClick={() => {
                      const result = duplicateTile(registry, tile.tileId);
                      if (!result) return;
                      setRegistry(result.registry);
                      setGrid((current) => ({
                        tiles: [
                          ...current.tiles,
                          ...createMeterGrid([result.tile.tileId]).tiles,
                        ],
                      }));
                      const key =
                        result.tile.subscriptionKey ?? tile.moduleKind;
                      const subscription = subscriptions.current.get(key);
                      if (subscription) subscription.count += 1;
                      else
                        subscriptions.current.set(key, {
                          count: 1,
                          cleanup: subscribeRef.current?.(key) ?? undefined,
                        });
                    }}
                  >
                    Duplicate
                  </button>
                  <button
                    type="button"
                    aria-label={`Remove ${title}`}
                    onClick={() => remove(tile.tileId)}
                  >
                    Remove
                  </button>
                </div>
              </article>,
            ];
          })}
        </section>
      )}
    </div>
  );
}
