import { useState } from "react";
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

const stateLabels: Record<TileState, string> = {
  loading: "Loading",
  ready: "Ready",
  unavailable: "Unavailable",
  incomplete: "Incomplete",
  degraded: "Degraded",
  error: "Error",
};

export function MeterWorkspace() {
  const [registry, setRegistry] = useState<MeterRegistry>(createMeterRegistry);
  const [experimentalEnabled, setExperimentalEnabled] = useState(false);
  const modules = getAvailableModules(experimentalEnabled);

  const addModule = (moduleKind: MeterModuleKind) => {
    setRegistry((current) => addTile(current, moduleKind).registry);
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
        <section className="meter-tile-list" aria-label="Meter tiles">
          {registry.tiles.map((tile) => {
            const module = modules.find(
              (item) => item.moduleKind === tile.moduleKind,
            );
            return (
              <article className="meter-tile" key={tile.tileId}>
                <header>
                  <h3>{module?.title ?? tile.moduleKind}</h3>
                  <span role="status">{stateLabels[tile.state]}</span>
                </header>
                <p>Tile {tile.tileId}</p>
                <label>
                  State
                  <select
                    aria-label={`${module?.title ?? tile.moduleKind} state`}
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
                    aria-label={`Duplicate ${module?.title ?? tile.moduleKind}`}
                    onClick={() =>
                      setRegistry(
                        (current) =>
                          duplicateTile(current, tile.tileId)?.registry ??
                          current,
                      )
                    }
                  >
                    Duplicate
                  </button>
                  <button
                    type="button"
                    aria-label={`Remove ${module?.title ?? tile.moduleKind}`}
                    onClick={() =>
                      setRegistry(
                        (current) => removeTile(current, tile.tileId).registry,
                      )
                    }
                  >
                    Remove
                  </button>
                </div>
              </article>
            );
          })}
        </section>
      )}
    </div>
  );
}
