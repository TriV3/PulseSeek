import type { WaveformStyle } from "../../api/commandEnvelope";

interface WaveformStyleSelectorProps {
  style: WaveformStyle;
  disabled?: boolean;
  onChange: (style: WaveformStyle) => void | Promise<void>;
}

const STYLES: Array<{ value: WaveformStyle; label: string }> = [
  { value: "solid", label: "Solid" },
  { value: "gradient", label: "Gradient" },
  { value: "outline", label: "Outline" },
];

export function WaveformStyleSelector({
  style,
  disabled = false,
  onChange,
}: WaveformStyleSelectorProps) {
  return (
    <div className="waveform-style-selector">
      <label htmlFor="waveform-style">Waveform style</label>
      <select
        id="waveform-style"
        value={style}
        disabled={disabled}
        onChange={(event) => void onChange(event.target.value as WaveformStyle)}
      >
        {STYLES.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </div>
  );
}
