import { useEffect, useRef } from "react";
import type { CopyItemResultData } from "../../api/playbackEvents";
import "../ConfirmDialog/ConfirmDialog.css";
import "./CopyDialog.css";

export interface CopyProgress {
  completed: number;
  total: number;
}

export interface CopySummary {
  okCount: number;
  failed: CopyItemResultData[];
}

interface CopyDialogProps {
  open: boolean;
  title: string;
  fileNameCount: number;
  targetDir: string | null;
  busy?: boolean;
  error?: string | null;
  progress?: CopyProgress | null;
  summary?: CopySummary | null;
  confirmLabel?: string;
  cancelLabel?: string;
  onPickTarget: () => void;
  onConfirm: () => void;
  onCancel: () => void;
}

/** Modal dialog for copying multiple files (FR-FM-004, FR-FM-005).
 *
 * Mirrors the focus-trapping and keyboard behavior of `MoveDialog`: Escape
 * cancels while the batch is not running, focus is trapped and restored. The
 * user picks the target folder, confirms the copy, and then sees per-file
 * progress with a Cancel action. When the batch finishes, successful and
 * failed targets are reported separately (the PR-077 acceptance) through a
 * live region so screen readers announce the outcome. Originals remain
 * unchanged, so copied rows stay in the current view.
 */
export function CopyDialog({
  open,
  title,
  fileNameCount,
  targetDir,
  busy = false,
  error = null,
  progress = null,
  summary = null,
  confirmLabel = "Copy",
  cancelLabel = "Cancel",
  onPickTarget,
  onConfirm,
  onCancel,
}: CopyDialogProps) {
  const dialogRef = useRef<HTMLDivElement>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);
  const onCancelRef = useRef(onCancel);

  useEffect(() => {
    onCancelRef.current = onCancel;
  }, [onCancel]);

  useEffect(() => {
    if (!open) return;

    previousFocusRef.current = document.activeElement as HTMLElement | null;

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

  const canConfirm = targetDir !== null && !busy && summary === null;
  const failedCount = summary?.failed.length ?? 0;

  return (
    <div className="confirm-dialog-backdrop">
      <div
        ref={dialogRef}
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="copy-dialog-title"
        aria-describedby="copy-dialog-message"
        className="confirm-dialog"
      >
        <h2 id="copy-dialog-title" className="confirm-dialog-title">
          {title}
        </h2>
        <p id="copy-dialog-message" className="copy-dialog-message">
          {fileNameCount === 1
            ? "Copy 1 file into a folder."
            : `Copy ${fileNameCount} files into a folder.`}
        </p>

        <div className="copy-dialog-target" aria-live="polite">
          <button
            type="button"
            className="copy-dialog-pick"
            onClick={onPickTarget}
            disabled={busy || summary !== null}
          >
            {targetDir ? "Choose a different folder…" : "Choose folder…"}
          </button>
          <span className="copy-dialog-target-path">
            {targetDir ?? "No folder selected"}
          </span>
        </div>

        {progress ? (
          <div
            className="copy-dialog-progress"
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={progress.total}
            aria-valuenow={progress.completed}
            aria-label="Copy progress"
          >
            <span className="copy-dialog-progress-label">
              Copying file {progress.completed} of {progress.total}…
            </span>
            <div className="copy-dialog-progress-track">
              <div
                className="copy-dialog-progress-bar"
                style={{
                  width: `${progress.total === 0 ? 0 : (progress.completed / progress.total) * 100}%`,
                }}
              />
            </div>
          </div>
        ) : null}

        {error ? (
          <p className="copy-dialog-error" role="alert">
            {error}
          </p>
        ) : null}

        {summary ? (
          <div className="copy-dialog-summary" role="status">
            <p className="copy-dialog-summary-ok">
              {summary.okCount} file{summary.okCount === 1 ? "" : "s"} copied.
            </p>
            {failedCount > 0 ? (
              <>
                <p className="copy-dialog-summary-failed-title">
                  {failedCount} file{failedCount === 1 ? "" : "s"} could not be
                  copied:
                </p>
                <ul className="copy-dialog-summary-failed">
                  {summary.failed.map((item) => (
                    <li key={item.path}>
                      {item.message ?? "PulseSeek could not copy that file."}
                    </li>
                  ))}
                </ul>
              </>
            ) : null}
          </div>
        ) : null}

        <div className="confirm-dialog-actions">
          <button
            className="confirm-dialog-button confirm-dialog-button--cancel"
            onClick={onCancel}
            type="button"
          >
            {summary ? "Close" : busy ? "Cancel" : cancelLabel}
          </button>
          <button
            className="confirm-dialog-button confirm-dialog-button--confirm copy-dialog-confirm"
            onClick={onConfirm}
            disabled={!canConfirm}
            type="button"
          >
            {busy ? "Copying…" : confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
