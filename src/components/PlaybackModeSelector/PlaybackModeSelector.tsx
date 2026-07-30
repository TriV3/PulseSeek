import type { PlaybackMode } from "../../api/commandEnvelope";

interface PlaybackModeSelectorProps {
  mode: PlaybackMode;
  disabled?: boolean;
  error?: string | null;
  onChange: (mode: PlaybackMode) => void | Promise<void>;
}

const MODES: Array<{ value: PlaybackMode; label: string }> = [
  { value: "one-shot", label: "One shot" },
  { value: "loop-current", label: "Loop current" },
  { value: "sequential", label: "Sequential" },
  { value: "random", label: "Random" },
];

export function PlaybackModeSelector({
  mode,
  disabled = false,
  error = null,
  onChange,
}: PlaybackModeSelectorProps) {
  return (
    <div className="playback-mode-selector">
      <label htmlFor="playback-mode">Playback mode</label>
      <select
        id="playback-mode"
        value={mode}
        disabled={disabled}
        onChange={(event) => void onChange(event.target.value as PlaybackMode)}
      >
        {MODES.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
      {error ? <p role="alert">{error}</p> : null}
    </div>
  );
}
