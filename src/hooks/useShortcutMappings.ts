import { useEffect, useState } from "react";
import {
  loadShortcuts,
  resetShortcuts,
  saveShortcuts,
} from "../api/commandEnvelope";
import {
  DEFAULT_SHORTCUTS,
  type ShortcutBindings,
} from "../shortcuts/keyboardShortcuts";

export function useShortcutMappings() {
  const [bindings, setBindings] = useState(DEFAULT_SHORTCUTS);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    void loadShortcuts()
      .then((confirmed) => {
        if (!active) return;
        setBindings(confirmed);
        setError(null);
      })
      .catch(() => {
        if (active) setError("Keyboard shortcuts unavailable.");
      })
      .finally(() => {
        if (active) setIsLoading(false);
      });
    return () => {
      active = false;
    };
  }, []);

  const save = async (
    requested: ShortcutBindings,
  ): Promise<ShortcutBindings | null> => {
    setIsSaving(true);
    setError(null);
    try {
      const confirmed = await saveShortcuts(requested);
      setBindings(confirmed);
      return confirmed;
    } catch {
      setError("Could not save keyboard shortcuts.");
      return null;
    } finally {
      setIsSaving(false);
    }
  };

  const reset = async (): Promise<ShortcutBindings | null> => {
    setIsSaving(true);
    setError(null);
    try {
      const confirmed = await resetShortcuts();
      setBindings(confirmed);
      return confirmed;
    } catch {
      setError("Could not reset keyboard shortcuts.");
      return null;
    } finally {
      setIsSaving(false);
    }
  };

  return { bindings, isLoading, isSaving, error, save, reset };
}
