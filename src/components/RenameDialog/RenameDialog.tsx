import { useEffect, useRef, useState } from "react";
import "./RenameDialog.css";
import "../ConfirmDialog/ConfirmDialog.css";

interface RenameDialogProps {
  open: boolean;
  title: string;
  initialName: string;
  busy?: boolean;
  error?: string | null;
  confirmLabel?: string;
  cancelLabel?: string;
  onConfirm: (newName: string) => void;
  onCancel: () => void;
}

/** Modal dialog for renaming a single file (FR-FM-004).
 *
 * Follows the focus-trapping and keyboard behavior of `ConfirmDialog` and adds
 * a text field: Enter submits, Escape cancels, and the confirm button is
 * disabled while the name is empty or the backend is busy. Backend failures
 * (invalid name, collision, permission) surface through `error` with a live
 * region so screen readers announce them.
 */
export function RenameDialog({
  open,
  title,
  initialName,
  busy = false,
  error = null,
  confirmLabel = "Rename",
  cancelLabel = "Cancel",
  onConfirm,
  onCancel,
}: RenameDialogProps) {
  const [name, setName] = useState(initialName);
  const dialogRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);
  const onCancelRef = useRef(onCancel);

  useEffect(() => {
    onCancelRef.current = onCancel;
  }, [onCancel]);

  useEffect(() => {
    if (!open) return;

    previousFocusRef.current = document.activeElement as HTMLElement | null;
    inputRef.current?.focus();
    inputRef.current?.select();

    const handler = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onCancelRef.current();
        return;
      }
      if (event.key !== "Tab") return;

      const focusable = dialogRef.current?.querySelectorAll<HTMLElement>(
        "button:not([disabled]), input:not([disabled])",
      );
      if (!focusable || focusable.length === 0) return;
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

    window.addEventListener("keydown", handler);
    return () => {
      window.removeEventListener("keydown", handler);
      previousFocusRef.current?.focus();
      previousFocusRef.current = null;
    };
    // The parent remounts this component (via `key`) for every open so the
    // text field always starts from the current basename.
  }, [open]);

  if (!open) return null;

  const trimmed = name.trim();

  const submit = () => {
    if (trimmed && !busy) onConfirm(trimmed);
  };

  return (
    <div className="confirm-dialog-backdrop">
      <div
        ref={dialogRef}
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="rename-dialog-title"
        aria-describedby="rename-dialog-message"
        className="confirm-dialog"
      >
        <h2 id="rename-dialog-title" className="confirm-dialog-title">
          {title}
        </h2>
        <p id="rename-dialog-message" className="confirm-dialog-message">
          Enter a new file name for “{initialName}”.
        </p>
        <div className="rename-dialog-field">
          <label className="visually-hidden" htmlFor="rename-dialog-input">
            New file name
          </label>
          <input
            id="rename-dialog-input"
            ref={inputRef}
            type="text"
            value={name}
            onChange={(event) => setName(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                submit();
              }
            }}
            disabled={busy}
            aria-invalid={error ? true : undefined}
          />
        </div>
        {error ? (
          <p className="rename-dialog-error" role="alert">
            {error}
          </p>
        ) : null}
        <div className="confirm-dialog-actions">
          <button
            className="confirm-dialog-button confirm-dialog-button--cancel"
            onClick={onCancel}
            disabled={busy}
            type="button"
          >
            {cancelLabel}
          </button>
          <button
            className="confirm-dialog-button confirm-dialog-button--confirm rename-dialog-confirm"
            onClick={submit}
            disabled={!trimmed || busy}
            type="button"
          >
            {busy ? "Renaming…" : confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
