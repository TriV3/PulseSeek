import type {
  VisualizationMode,
  VisualizationQuality,
} from "../../api/commandEnvelope";

export type { VisualizationMode, VisualizationQuality };

interface VisualizationSelectorProps {
  value: VisualizationMode;
  onChange: (value: VisualizationMode) => void;
}

interface VisualizationSettingsControlsProps {
  enabled: boolean;
  quality: VisualizationQuality;
  onEnabledChange: (enabled: boolean) => void;
  onQualityChange: (quality: VisualizationQuality) => void;
  reducedMotion?: boolean;
}

const VISUALIZATIONS: Array<{
  value: VisualizationMode;
  label: string;
}> = [
  { value: "waveform", label: "Waveform" },
  { value: "logarithmic", label: "Log analyzer" },
  { value: "linear", label: "Linear analyzer" },
  { value: "musical", label: "Musical spectrum" },
];

export function VisualizationSelector({
  value,
  onChange,
}: VisualizationSelectorProps) {
  return (
    <div className="visualization-selector">
      <label htmlFor="visualization-mode">Visualization</label>
      <select
        id="visualization-mode"
        value={value}
        onChange={(event) =>
          onChange(event.currentTarget.value as VisualizationMode)
        }
      >
        {VISUALIZATIONS.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </div>
  );
}

export function VisualizationSettingsControls({
  enabled,
  quality,
  onEnabledChange,
  onQualityChange,
  reducedMotion = false,
}: VisualizationSettingsControlsProps) {
  return (
    <section
      className="visualization-settings-controls"
      aria-label="Visualization settings"
    >
      <label className="visualization-settings-row">
        <span>Real-time visualizations</span>
        <input
          type="checkbox"
          checked={enabled}
          onChange={(event) => onEnabledChange(event.currentTarget.checked)}
        />
      </label>
      <label className="visualization-settings-row">
        <span>Visualization quality</span>
        <select
          value={quality}
          disabled={!enabled || reducedMotion}
          onChange={(event) =>
            onQualityChange(event.currentTarget.value as VisualizationQuality)
          }
        >
          <option value="low">Low (15 FPS)</option>
          <option value="balanced">Balanced (30 FPS)</option>
          <option value="high">High (60 FPS)</option>
        </select>
      </label>
      {reducedMotion && (
        <span className="visualization-motion-note" role="status">
          Reduced motion is active; the waveform is shown.
        </span>
      )}
    </section>
  );
}
