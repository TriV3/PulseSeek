import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  loadVisualizationSettings,
  saveVisualizationSettings,
  type VisualizationSettings,
} from "../api/commandEnvelope";

const REDUCED_MOTION_QUERY = "(prefers-reduced-motion: reduce)";

export const DEFAULT_VISUALIZATION_SETTINGS: VisualizationSettings = {
  enabled: true,
  mode: "waveform",
  quality: "balanced",
};

function systemPrefersReducedMotion(): boolean {
  return window.matchMedia?.(REDUCED_MOTION_QUERY).matches ?? false;
}

export function useVisualizationSettings() {
  const [settings, setSettings] = useState(DEFAULT_VISUALIZATION_SETTINGS);
  const [reducedMotion, setReducedMotion] = useState(
    systemPrefersReducedMotion,
  );
  const [isLoaded, setIsLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const current = useRef(DEFAULT_VISUALIZATION_SETTINGS);
  const updateRevision = useRef(0);

  useEffect(() => {
    const media = window.matchMedia?.(REDUCED_MOTION_QUERY);
    if (!media) return;
    const update = () => setReducedMotion(media.matches);
    update();
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, []);

  useEffect(() => {
    let active = true;
    const revisionAtStart = updateRevision.current;
    void loadVisualizationSettings(reducedMotion)
      .then((loaded) => {
        if (!active || updateRevision.current !== revisionAtStart) return;
        current.current = loaded;
        setSettings(loaded);
        setError(null);
      })
      .catch((cause: unknown) => {
        if (!active) return;
        setError(
          cause instanceof Error
            ? cause.message
            : "Visualization settings unavailable.",
        );
      })
      .finally(() => {
        if (active) setIsLoaded(true);
      });
    return () => {
      active = false;
    };
  }, [reducedMotion]);

  const update = useCallback(
    (change: Partial<VisualizationSettings>) => {
      updateRevision.current += 1;
      const next = { ...current.current, ...change };
      current.current = next;
      setSettings(next);
      setError(null);
      void saveVisualizationSettings(next, reducedMotion)
        .then((confirmed) => {
          if (current.current !== next) return;
          current.current = confirmed;
          setSettings(confirmed);
        })
        .catch((cause: unknown) => {
          setError(
            cause instanceof Error
              ? cause.message
              : "Could not save visualization settings.",
          );
        });
    },
    [reducedMotion],
  );

  const effectiveMode = useMemo(
    () =>
      settings.enabled && !reducedMotion
        ? settings.mode
        : ("waveform" as const),
    [reducedMotion, settings.enabled, settings.mode],
  );

  return {
    settings,
    effectiveMode,
    reducedMotion,
    isLoaded,
    error,
    update,
  };
}
