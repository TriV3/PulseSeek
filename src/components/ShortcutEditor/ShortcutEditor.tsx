import { useEffect, useRef, useState } from "react";
import {
  formatShortcut,
  shortcutFromEvent,
  SHORTCUT_ACTIONS,
  validateShortcutBindings,
  type ShortcutActionId,
  type ShortcutBindings,
  type ShortcutPlatform,
} from "../../shortcuts/keyboardShortcuts";
import "./ShortcutEditor.css";

export interface ShortcutEditorProps {
  open: boolean;
  bindings: ShortcutBindings;
  platform: ShortcutPlatform;
  onSave: (bindings: ShortcutBindings) => void | Promise<void>;
  onReset: () => ShortcutBindings | Promise<ShortcutBindings>;
  onCancel: () => void;
}

function cloneBindings(bindings: ShortcutBindings): ShortcutBindings {
  return Object.fromEntries(
    Object.entries(bindings).map(([id, binding]) => [
      id,
      binding ? { ...binding } : null,
    ]),
  ) as unknown as ShortcutBindings;
}

export function ShortcutEditor({
  open,
  bindings,
  platform,
  onSave,
  onReset,
  onCancel,
}: ShortcutEditorProps) {
  if (!open) return null;
  return (
    <ShortcutEditorModal
      bindings={bindings}
      platform={platform}
      onSave={onSave}
      onReset={onReset}
      onCancel={onCancel}
    />
  );
}

type ShortcutEditorModalProps = Omit<ShortcutEditorProps, "open">;

function ShortcutEditorModal({
  bindings,
  platform,
  onSave,
  onReset,
  onCancel,
}: ShortcutEditorModalProps) {
  const [draft, setDraft] = useState(() => cloneBindings(bindings));
  const [capturing, setCapturing] = useState<ShortcutActionId | null>(null);
  const [operation, setOperation] = useState<"idle" | "saving" | "resetting">(
    "idle",
  );
  const [operationError, setOperationError] = useState<string | null>(null);
  const dialogRef = useRef<HTMLDivElement>(null);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);
  const onCancelRef = useRef(onCancel);

  const errors = validateShortcutBindings(draft, platform);
  const errorMessages = [...new Set(Object.values(errors))];

  useEffect(() => {
    onCancelRef.current = onCancel;
  }, [onCancel]);

  useEffect(() => {
    previousFocusRef.current = document.activeElement as HTMLElement | null;
    cancelRef.current?.focus();

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return;
      if (event.key === "Escape") {
        event.preventDefault();
        onCancelRef.current();
        return;
      }
      if (event.key !== "Tab") return;

      const focusable = dialogRef.current?.querySelectorAll<HTMLElement>(
        "button:not([disabled])",
      );
      if (!focusable?.length) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      previousFocusRef.current?.focus();
      previousFocusRef.current = null;
    };
  }, []);

  return (
    <div className="shortcut-editor-backdrop">
      <div
        ref={dialogRef}
        className="shortcut-editor"
        role="dialog"
        aria-modal="true"
        aria-labelledby="shortcut-editor-title"
        aria-describedby="shortcut-editor-description"
      >
        <header className="shortcut-editor-header">
          <h2 id="shortcut-editor-title">Keyboard shortcuts</h2>
          <p id="shortcut-editor-description">
            Select a shortcut, then press the new key combination.
          </p>
        </header>

        {errorMessages.length > 0 && (
          <div className="shortcut-editor-summary" role="alert">
            {errorMessages.join(" ")}
          </div>
        )}
        {operationError && (
          <div className="shortcut-editor-summary" role="alert">
            {operationError}
          </div>
        )}

        <div className="shortcut-editor-list">
          {SHORTCUT_ACTIONS.map((action) => {
            const binding = draft[action.id];
            const error = errors[action.id];
            return (
              <div
                className="shortcut-editor-row"
                key={action.id}
                data-available={action.available}
              >
                <span className="shortcut-editor-label">{action.label}</span>
                {action.available ? (
                  <button
                    type="button"
                    className="shortcut-editor-capture"
                    aria-label={`Change shortcut for ${action.label}`}
                    aria-pressed={capturing === action.id}
                    aria-invalid={error ? "true" : undefined}
                    aria-describedby={
                      error ? `shortcut-error-${action.id}` : undefined
                    }
                    disabled={operation !== "idle"}
                    onClick={() => setCapturing(action.id)}
                    onKeyDown={(event) => {
                      if (
                        capturing !== action.id ||
                        event.nativeEvent.isComposing
                      )
                        return;
                      if (event.key === "Escape") return;
                      const next = shortcutFromEvent(
                        event.nativeEvent,
                        platform,
                      );
                      if (!next) return;
                      event.preventDefault();
                      event.stopPropagation();
                      setDraft((current) => ({
                        ...current,
                        [action.id]: next,
                      }));
                      setCapturing(null);
                    }}
                  >
                    {capturing === action.id
                      ? "Press shortcut"
                      : binding
                        ? formatShortcut(binding, platform)
                        : "Unassigned"}
                  </button>
                ) : (
                  <span className="shortcut-editor-unavailable">
                    Unavailable
                  </span>
                )}
                {error && (
                  <span
                    id={`shortcut-error-${action.id}`}
                    className="shortcut-editor-error"
                  >
                    {error}
                  </span>
                )}
              </div>
            );
          })}
        </div>

        <footer className="shortcut-editor-actions">
          <button
            type="button"
            className="shortcut-editor-button"
            disabled={operation !== "idle"}
            onClick={() => {
              setOperation("resetting");
              setOperationError(null);
              void Promise.resolve(onReset())
                .then((confirmed) => {
                  setDraft(cloneBindings(confirmed));
                  setCapturing(null);
                })
                .catch((error: unknown) => {
                  setOperationError(
                    error instanceof Error
                      ? error.message
                      : "Could not reset keyboard shortcuts.",
                  );
                })
                .finally(() => setOperation("idle"));
            }}
          >
            {operation === "resetting" ? "Resetting…" : "Reset"}
          </button>
          <button
            ref={cancelRef}
            type="button"
            className="shortcut-editor-button"
            disabled={operation !== "idle"}
            onClick={onCancel}
          >
            Cancel
          </button>
          <button
            type="button"
            className="shortcut-editor-button shortcut-editor-button--primary"
            disabled={errorMessages.length > 0 || operation !== "idle"}
            onClick={() => {
              setOperation("saving");
              setOperationError(null);
              void Promise.resolve(onSave(cloneBindings(draft)))
                .catch((error: unknown) => {
                  setOperationError(
                    error instanceof Error
                      ? error.message
                      : "Could not save keyboard shortcuts.",
                  );
                })
                .finally(() => setOperation("idle"));
            }}
          >
            {operation === "saving" ? "Saving…" : "Save"}
          </button>
        </footer>
      </div>
    </div>
  );
}
