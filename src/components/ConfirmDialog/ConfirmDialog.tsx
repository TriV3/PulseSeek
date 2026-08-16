import { useEffect, useRef } from "react";

interface ConfirmDialogProps {
  open: boolean;
  title: string;
  message: string;
  confirmLabel?: string;
  confirmDisabled?: boolean;
  cancelLabel?: string;
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmDialog({
  open,
  title,
  message,
  confirmLabel = "Confirm",
  confirmDisabled = false,
  cancelLabel = "Cancel",
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  const dialogRef = useRef<HTMLDivElement>(null);
  const confirmRef = useRef<HTMLButtonElement>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);
  const onCancelRef = useRef(onCancel);

  useEffect(() => {
    onCancelRef.current = onCancel;
  }, [onCancel]);

  useEffect(() => {
    if (!open) return;

    previousFocusRef.current = document.activeElement as HTMLElement | null;
    confirmRef.current?.focus();

    const handler = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onCancelRef.current();
        return;
      }
      if (event.key !== "Tab") return;

      const focusable = dialogRef.current?.querySelectorAll<HTMLElement>(
        "button:not([disabled])",
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
  }, [open]);

  if (!open) return null;

  return (
    <div className="confirm-dialog-backdrop">
      <div
        ref={dialogRef}
        role="alertdialog"
        aria-labelledby="confirm-dialog-title"
        aria-describedby="confirm-dialog-message"
        className="confirm-dialog"
        aria-modal="true"
      >
        <h2 id="confirm-dialog-title" className="confirm-dialog-title">
          {title}
        </h2>
        <p id="confirm-dialog-message" className="confirm-dialog-message">
          {message}
        </p>
        <div className="confirm-dialog-actions">
          <button
            className="confirm-dialog-button confirm-dialog-button--cancel"
            onClick={onCancel}
            type="button"
          >
            {cancelLabel}
          </button>
          <button
            className="confirm-dialog-button confirm-dialog-button--confirm"
            onClick={onConfirm}
            type="button"
            disabled={confirmDisabled}
            ref={confirmRef}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
