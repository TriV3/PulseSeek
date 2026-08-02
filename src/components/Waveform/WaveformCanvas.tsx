import { useCallback, useEffect, useRef } from "react";
import { onPosition } from "../../api/playbackEvents";
import type { WaveformLevel } from "../../api/waveform";
import {
  buildEnvelope,
  defaultTargetPeaksForWidth,
  drawEnvelope,
  positionMsForX,
  resolveTokens,
  type Canvas2D,
  type WaveformStyle,
  type WaveformTokens,
} from "./waveformRenderer";

export interface WaveformCanvasProps {
  waveform: WaveformLevel | null;
  durationMs: number | null;
  restoredPositionMs?: number;
  /** Increments when Stop must reset the visible playhead immediately. */
  resetRevision?: number;
  onRequestRefetch?: (targetPeaks: number) => void;
  onSeek?: (positionMs: number) => void | Promise<void>;
  seekStepMs?: number;
  targetPeaksForWidth?: (widthPx: number) => number;
  getTokens?: (scope: Element | null | undefined) => WaveformTokens;
  style?: WaveformStyle;
}

const REFETCH_DEBOUNCE_MS = 200;
const DEFAULT_SEEK_STEP_MS = 5_000;
const TIME_LABEL_HALF_WIDTH_PX = 24;

function timeLabelX(markerX: number, width: number): number {
  if (width <= TIME_LABEL_HALF_WIDTH_PX * 2) return width / 2;
  return Math.max(
    TIME_LABEL_HALF_WIDTH_PX,
    Math.min(width - TIME_LABEL_HALF_WIDTH_PX, markerX),
  );
}

/**
 * Canvas 2D waveform renderer with an imperative playhead.
 *
 * Playback position is consumed directly from the throttled position event
 * stream and drawn through `requestAnimationFrame`; it is never stored in
 * React state, so the high-frequency position updates do not re-render the
 * component. Resize re-requests a resolution level that fits the new width.
 */
export function WaveformCanvas({
  waveform,
  durationMs,
  restoredPositionMs,
  resetRevision = 0,
  onRequestRefetch,
  onSeek,
  seekStepMs = DEFAULT_SEEK_STEP_MS,
  targetPeaksForWidth = defaultTargetPeaksForWidth,
  getTokens = resolveTokens,
  style = "outline",
}: WaveformCanvasProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const currentTimeRef = useRef<HTMLSpanElement | null>(null);
  const currentMarkerRef = useRef<HTMLSpanElement | null>(null);
  const hoverMarkerRef = useRef<HTMLSpanElement | null>(null);
  const hoverTimeRef = useRef<HTMLSpanElement | null>(null);
  const positionRef = useRef<number | null>(null);
  const durationRef = useRef<number | null>(durationMs);
  const widthRef = useRef(0);
  const heightRef = useRef(0);
  const rafRef = useRef<number | null>(null);
  const refetchTimer = useRef<number | null>(null);
  const draggingRef = useRef(false);
  const dragTargetRef = useRef<number | null>(null);

  const propsRef = useRef({
    waveform,
    durationMs,
    restoredPositionMs,
    resetRevision,
    onRequestRefetch,
    onSeek,
    seekStepMs,
    targetPeaksForWidth,
    getTokens,
    style,
  });
  useEffect(() => {
    propsRef.current = {
      waveform,
      durationMs,
      restoredPositionMs,
      resetRevision,
      onRequestRefetch,
      onSeek,
      seekStepMs,
      targetPeaksForWidth,
      getTokens,
      style,
    };
  }, [
    waveform,
    durationMs,
    restoredPositionMs,
    resetRevision,
    onRequestRefetch,
    onSeek,
    seekStepMs,
    targetPeaksForWidth,
    getTokens,
    style,
  ]);

  const renderPlayhead = useCallback(() => {
    const width = widthRef.current;
    const effectiveDurationMs = durationRef.current;
    const positionMs = positionRef.current;
    const canvas = canvasRef.current;
    if (!canvas || width <= 0) return;

    canvas.setAttribute("aria-valuenow", String(positionMs ?? 0));
    canvas.setAttribute("aria-valuemax", String(effectiveDurationMs ?? 0));
    canvas.setAttribute("aria-disabled", String(effectiveDurationMs === null));
    canvas.tabIndex = effectiveDurationMs === null ? -1 : 0;

    const playheadX =
      effectiveDurationMs === null || positionMs === null
        ? null
        : Math.min(
            width,
            Math.max(0, (positionMs / effectiveDurationMs) * width),
          );
    const currentTime = currentTimeRef.current;
    const currentMarker = currentMarkerRef.current;
    if (!currentTime) return;
    if (playheadX === null || positionMs === null) {
      currentTime.hidden = true;
      if (currentMarker) currentMarker.hidden = true;
      return;
    }
    currentTime.hidden = false;
    currentTime.textContent = formatWaveformTime(positionMs);
    currentTime.style.left = `${timeLabelX(playheadX, width)}px`;
    if (currentMarker) {
      currentMarker.hidden = false;
      currentMarker.style.left = `${playheadX}px`;
    }
  }, []);

  const draw = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d") as unknown as Canvas2D | null;
    if (!ctx) return;
    const width = widthRef.current;
    const height = heightRef.current;
    if (width <= 0 || height <= 0) return;

    const { waveform, getTokens, style } = propsRef.current;
    const effectiveDurationMs = durationRef.current;
    const tokens = getTokens(canvas);
    const geometry = waveform
      ? buildEnvelope(waveform, width, height, null, effectiveDurationMs)
      : { channels: [], playheadX: null };
    // The waveform itself is static between data updates. The progress marker
    // is a composited DOM layer so its movement remains smooth without
    // repainting the full envelope on every position event.
    drawEnvelope(
      ctx,
      { ...geometry, playheadX: null },
      tokens,
      width,
      height,
      style,
    );

    renderPlayhead();
  }, [renderPlayhead]);

  const scheduleDraw = useCallback(() => {
    if (rafRef.current !== null) return;
    rafRef.current = window.requestAnimationFrame(() => {
      rafRef.current = null;
      draw();
    });
  }, [draw]);

  const schedulePlayhead = useCallback(() => {
    if (rafRef.current !== null) return;
    rafRef.current = window.requestAnimationFrame(() => {
      rafRef.current = null;
      renderPlayhead();
    });
  }, [renderPlayhead]);

  const commitSeek = useCallback(
    (targetMs: number) => {
      positionRef.current = targetMs;
      schedulePlayhead();
      propsRef.current.onSeek?.(targetMs);
    },
    [schedulePlayhead],
  );

  const previewSeek = useCallback(
    (targetMs: number) => {
      positionRef.current = targetMs;
      schedulePlayhead();
    },
    [schedulePlayhead],
  );

  const targetFromPointer = useCallback((clientX: number): number | null => {
    const canvas = canvasRef.current;
    if (!canvas) return null;
    const rect = canvas.getBoundingClientRect();
    return positionMsForX(clientX - rect.left, rect.width, durationRef.current);
  }, []);

  const updateHover = useCallback((clientX: number) => {
    const canvas = canvasRef.current;
    const marker = hoverMarkerRef.current;
    const label = hoverTimeRef.current;
    if (!canvas || !marker || !label) return;
    const rect = canvas.getBoundingClientRect();
    const target = positionMsForX(
      clientX - rect.left,
      rect.width,
      durationRef.current,
    );
    if (target === null) {
      marker.hidden = true;
      label.hidden = true;
      return;
    }
    const x = Math.max(0, Math.min(rect.width, clientX - rect.left));
    marker.hidden = false;
    label.hidden = false;
    marker.style.left = `${x}px`;
    label.style.left = `${timeLabelX(x, rect.width)}px`;
    label.textContent = formatWaveformTime(target);
  }, []);

  const handlePointerDown = useCallback(
    (event: React.PointerEvent<HTMLCanvasElement>) => {
      if (durationRef.current === null) return;
      const target = targetFromPointer(event.clientX);
      if (target === null) return;
      event.preventDefault();
      draggingRef.current = true;
      dragTargetRef.current = target;
      try {
        event.currentTarget.setPointerCapture(event.pointerId);
      } catch {
        // Pointer capture can be unavailable in non-browser environments;
        // dragging still works without it.
      }
      previewSeek(target);
    },
    [previewSeek, targetFromPointer],
  );

  const handlePointerMove = useCallback(
    (event: React.PointerEvent<HTMLCanvasElement>) => {
      updateHover(event.clientX);
      if (!draggingRef.current) return;
      const target = targetFromPointer(event.clientX);
      if (target === null) return;
      dragTargetRef.current = target;
      previewSeek(target);
    },
    [previewSeek, targetFromPointer, updateHover],
  );

  const handlePointerLeave = useCallback(() => {
    if (draggingRef.current) return;
    if (hoverMarkerRef.current) hoverMarkerRef.current.hidden = true;
    if (hoverTimeRef.current) hoverTimeRef.current.hidden = true;
  }, []);

  const finishPointerInteraction = useCallback(
    (event: React.PointerEvent<HTMLCanvasElement>, shouldCommit: boolean) => {
      if (!draggingRef.current) return;
      draggingRef.current = false;
      const target = dragTargetRef.current;
      dragTargetRef.current = null;
      try {
        event.currentTarget.releasePointerCapture(event.pointerId);
      } catch {
        // Pointer capture is best-effort.
      }
      if (shouldCommit && target !== null) commitSeek(target);
    },
    [commitSeek],
  );

  const handleKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLCanvasElement>) => {
      const { durationMs, seekStepMs } = propsRef.current;
      if (durationMs === null) return;
      const current = positionRef.current ?? 0;
      let target: number | null = null;
      switch (event.key) {
        case "ArrowRight":
          target = Math.min(durationMs, current + seekStepMs);
          break;
        case "ArrowLeft":
          target = Math.max(0, current - seekStepMs);
          break;
        case "Home":
          target = 0;
          break;
        case "End":
          target = durationMs;
          break;
        default:
          return;
      }
      event.preventDefault();
      commitSeek(target);
    },
    [commitSeek],
  );

  // Re-draw when waveform data or duration changes (low frequency). The
  // playhead is reset only when the file's waveform changes, so the same
  // file's first duration update never wipes a freshly confirmed position.
  const lastWaveformRef = useRef<WaveformLevel | null>(waveform);
  const receivedPositionEventRef = useRef(false);
  const lastResetRevisionRef = useRef(resetRevision);
  useEffect(() => {
    durationRef.current = durationMs;
    if (lastWaveformRef.current !== waveform) {
      lastWaveformRef.current = waveform;
      receivedPositionEventRef.current = false;
      positionRef.current = restoredPositionMs ?? null;
    } else if (
      restoredPositionMs !== undefined &&
      !receivedPositionEventRef.current &&
      !draggingRef.current
    ) {
      positionRef.current = restoredPositionMs;
    }
    scheduleDraw();
    // style is intentionally not part of the refetch path: a style change only
    // repaints the canvas and never re-requests waveform data (FR-VS-004).
  }, [waveform, durationMs, restoredPositionMs, style, scheduleDraw]);

  useEffect(() => {
    if (lastResetRevisionRef.current === resetRevision) return;
    lastResetRevisionRef.current = resetRevision;
    receivedPositionEventRef.current = false;
    positionRef.current = 0;
    scheduleDraw();
  }, [resetRevision, scheduleDraw]);

  // Position events drive the playhead imperatively, never via React state.
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void onPosition((payload) => {
      if (disposed) return;
      // During an active drag the pointer preview owns the playhead; a
      // confirmed position event must not fight the pointer. The next event
      // after the drag reconciles the visual with the Rust position.
      if (!draggingRef.current) {
        positionRef.current = payload.position_ms;
      }
      receivedPositionEventRef.current = true;
      if (payload.duration_ms !== null) {
        durationRef.current = payload.duration_ms;
      }
      schedulePlayhead();
    })
      .then((cleanup) => {
        if (disposed) cleanup();
        else unlisten = cleanup;
      })
      .catch(() => {
        // Position updates are cosmetic; the waveform still renders.
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [schedulePlayhead]);

  // Measure the canvas and request a fitting resolution level on resize.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const measure = (width: number, height: number) => {
      widthRef.current = Math.round(width);
      heightRef.current = Math.round(height);
      canvas.width = Math.max(1, Math.round(width));
      canvas.height = Math.max(1, Math.round(height));
      // Draw immediately: the observer may fire before any rAF that was
      // scheduled earlier has run, and a pending rAF guard must not swallow
      // the first real paint.
      draw();
    };

    const scheduleRefetch = (width: number) => {
      if (refetchTimer.current !== null)
        window.clearTimeout(refetchTimer.current);
      refetchTimer.current = window.setTimeout(() => {
        refetchTimer.current = null;
        propsRef.current.onRequestRefetch?.(
          propsRef.current.targetPeaksForWidth(width),
        );
      }, REFETCH_DEBOUNCE_MS);
    };

    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      const { width, height } = entry.contentRect;
      if (!Number.isFinite(width) || !Number.isFinite(height)) return;
      if (width <= 0 || height <= 0) return;
      measure(width, height);
      scheduleRefetch(width);
    });
    observer.observe(canvas);

    return () => {
      observer.disconnect();
      if (refetchTimer.current !== null)
        window.clearTimeout(refetchTimer.current);
    };
  }, [draw]);

  // Cancel any pending animation frame on unmount so a stale callback never
  // paints to a detached canvas. The ref is cleared too: in dev StrictMode
  // (mount → cleanup → mount) a cancelled-but-still-set id would otherwise
  // make every later `scheduleDraw` skip, permanently freezing the playhead.
  useEffect(() => {
    return () => {
      if (rafRef.current !== null) {
        window.cancelAnimationFrame(rafRef.current);
        rafRef.current = null;
      }
    };
  }, []);

  return (
    <div className="waveform-interaction">
      <canvas
        ref={canvasRef}
        className="waveform-canvas-surface"
        role="slider"
        aria-label="Waveform seek"
        aria-valuemin={0}
        aria-valuemax={durationMs ?? 0}
        aria-valuenow={0}
        aria-disabled={durationMs === null}
        tabIndex={durationMs === null ? -1 : 0}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={(event) => finishPointerInteraction(event, true)}
        onPointerCancel={(event) => finishPointerInteraction(event, false)}
        onPointerLeave={handlePointerLeave}
        onKeyDown={handleKeyDown}
      />
      <span
        ref={currentMarkerRef}
        className="waveform-current-marker"
        data-testid="waveform-current-marker"
        aria-hidden="true"
        hidden
      />
      <span
        ref={currentTimeRef}
        className="waveform-time waveform-time--current"
        data-testid="waveform-current-time"
        aria-hidden="true"
        hidden
      />
      <span
        ref={hoverMarkerRef}
        className="waveform-hover-marker"
        data-testid="waveform-hover-marker"
        aria-hidden="true"
        hidden
      />
      <span
        ref={hoverTimeRef}
        className="waveform-time waveform-time--hover"
        data-testid="waveform-hover-time"
        aria-hidden="true"
        hidden
      />
    </div>
  );
}

function formatWaveformTime(positionMs: number): string {
  const totalSeconds = Math.max(0, Math.floor(positionMs / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  return hours > 0
    ? `${hours}:${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`
    : `${minutes}:${String(seconds).padStart(2, "0")}`;
}
