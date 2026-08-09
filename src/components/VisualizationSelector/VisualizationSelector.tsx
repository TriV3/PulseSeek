export type VisualizationMode = "waveform" | "logarithmic" | "linear";

interface VisualizationSelectorProps {
  value: VisualizationMode;
  onChange: (value: VisualizationMode) => void;
}

const VISUALIZATIONS: Array<{
  value: VisualizationMode;
  label: string;
}> = [
  { value: "waveform", label: "Waveform" },
  { value: "logarithmic", label: "Log analyzer" },
  { value: "linear", label: "Linear analyzer" },
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
