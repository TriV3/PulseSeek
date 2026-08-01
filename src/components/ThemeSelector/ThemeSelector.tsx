import type { ThemePreference } from "../../api/commandEnvelope";

interface ThemeSelectorProps {
  theme: ThemePreference;
  disabled?: boolean;
  onChange: (theme: ThemePreference) => void | Promise<void>;
}

const THEMES: Array<{ value: ThemePreference; label: string }> = [
  { value: "system", label: "System" },
  { value: "light", label: "PulseSeek Light" },
  { value: "dark", label: "PulseSeek Dark" },
  { value: "midnight", label: "Midnight Blue" },
  { value: "high-contrast", label: "High Contrast" },
];

export function ThemeSelector({
  theme,
  disabled = false,
  onChange,
}: ThemeSelectorProps) {
  return (
    <div className="theme-selector">
      <label htmlFor="theme">Theme</label>
      <select
        id="theme"
        value={theme}
        disabled={disabled}
        onChange={(event) =>
          void onChange(event.target.value as ThemePreference)
        }
      >
        {THEMES.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </div>
  );
}
