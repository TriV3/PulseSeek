import { useEffect, useState } from "react";
import type { ThemePreference } from "../api/commandEnvelope";

export type ResolvedTheme = "light" | "dark" | "midnight" | "high-contrast";

const SYSTEM_QUERY = "(prefers-color-scheme: dark)";

function systemPrefersDark(): boolean {
  return window.matchMedia?.(SYSTEM_QUERY).matches ?? false;
}

function resolve(preference: ThemePreference): ResolvedTheme {
  if (preference === "system") {
    return systemPrefersDark() ? "dark" : "light";
  }
  return preference;
}

/**
 * Resolves the active theme and applies it to the document without restart.
 * The "system" preference follows the operating-system color scheme live.
 */
export function useTheme(preference: ThemePreference): ResolvedTheme {
  const [resolved, setResolved] = useState<ResolvedTheme>(() =>
    resolve(preference),
  );

  useEffect(() => {
    const update = () => setResolved(resolve(preference));
    update();
    if (preference !== "system") return;

    const media = window.matchMedia?.(SYSTEM_QUERY);
    if (!media) return;
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, [preference]);

  useEffect(() => {
    document.documentElement.dataset.theme = resolved;
    return () => {
      delete document.documentElement.dataset.theme;
    };
  }, [resolved]);

  return resolved;
}
