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
  type EnvelopeGeometry,
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
  /** Accessible name for the seek surface when reused by another visualizer. */
  ariaLabel?: string;
}

const REFETCH_DEBOUNCE_MS = 200;
const DEFAULT_SEEK_STEP_MS = 5_000;
const TIME_LABEL_HALF_WIDTH_PX = 24;
const POINTER_UPDATE_INTERVAL_MS = 1_000 / 60;

function timeLabelX(markerX: number, width: number): number {
  if (width <= TIME_LABEL_HALF_WIDTH_PX * 2) return width / 2;
  return Math.max(
    TIME_LABEL_HALF_WIDTH_PX,
    Math.min(width - TIME_LABEL_HALF_WIDTH_PX, markerX),
  );
}

function setSeekX(element: HTMLElement, positionPx: number): void {
  element.style.setProperty("--seek-x", `${positionPx}px`);
}

/**
 * Canvas 2D waveform renderer with an imperative playhead.
 *
 * Playback position is consumed directly from the throttled position event
 * stream and is never stored in React state, so high-frequency position
 * updates do not re-render the component. Resize re-requests a resolution
 * level that fits the new width.
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
  ariaLabel = "Waveform seek",
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
  const drawRafRef = useRef<number | null>(null);
  const interactionRectRef = useRef<DOMRect | null>(null);
  const geometryRef = useRef<CachedGeometry | null>(null);
  const refetchTimer = useRef<number | null>(null);
  const draggingRef = useRef(false);
  const dragTargetRef = useRef<number | null>(null);
  const pendingPointerXRef = useRef<number | null>(null);
  const pointerTimerRef = useRef<number | null>(null);
  const lastPointerUpdateAtRef = useRef(Number.NEGATIVE_INFINITY);

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
    const formattedPosition = formatWaveformTime(positionMs);
    if (currentTime.textContent !== formattedPosition) {
      currentTime.textContent = formattedPosition;
    }
    setSeekX(currentTime, timeLabelX(playheadX, width));
    if (currentMarker) {
      currentMarker.hidden = false;
      setSeekX(currentMarker, playheadX);
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
    const waveformChanged = geometryRef.current?.source !== waveform;
    if (
      waveformChanged ||
      geometryRef.current?.width !== width ||
      geometryRef.current?.height !== height
    ) {
      const geometry = waveform
        ? buildEnvelope(waveform, width, height, null, effectiveDurationMs)
        : { channels: [], playheadX: null };
      geometryRef.current = { ...geometry, source: waveform, width, height };
    }
    const tokens = getTokens(canvas);
    // The waveform itself is static between data updates. The progress marker
    // is a composited DOM layer so its movement remains smooth without
    // repainting the full envelope on every position event.
    drawEnvelope(
      ctx,
      { ...geometryRef.current, playheadX: null },
      tokens,
      width,
      height,
      style,
    );

    renderPlayhead();
  }, [renderPlayhead]);

  const scheduleDraw = useCallback(() => {
    if (drawRafRef.current !== null) return;
    drawRafRef.current = window.requestAnimationFrame(() => {
      drawRafRef.current = null;
      draw();
    });
  }, [draw]);

  const commitSeek = useCallback(
    (targetMs: number) => {
      positionRef.current = targetMs;
      renderPlayhead();
      propsRef.current.onSeek?.(targetMs);
    },
    [renderPlayhead],
  );

  const previewSeek = useCallback(
    (targetMs: number) => {
      positionRef.current = targetMs;
      renderPlayhead();
    },
    [renderPlayhead],
  );

  const targetFromPointer = useCallback((clientX: number): number | null => {
    const canvas = canvasRef.current;
    if (!canvas) return null;
    const cachedRect = interactionRectRef.current;
    const rect =
      cachedRect && cachedRect.width > 0
        ? cachedRect
        : canvas.getBoundingClientRect();
    interactionRectRef.current = rect;
    return positionMsForX(clientX - rect.left, rect.width, durationRef.current);
  }, []);

  const updateHover = useCallback((clientX: number) => {
    const marker = hoverMarkerRef.current;
    const label = hoverTimeRef.current;
    const canvas = canvasRef.current;
    const cachedRect = interactionRectRef.current;
    const rect =
      cachedRect && cachedRect.width > 0
        ? cachedRect
        : canvas?.getBoundingClientRect();
    if (!marker || !label || !rect) return;
    interactionRectRef.current = rect;
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
    setSeekX(marker, x);
    setSeekX(label, timeLabelX(x, rect.width));
    const formattedTarget = formatWaveformTime(target);
    if (label.textContent !== formattedTarget) {
      label.textContent = formattedTarget;
    }
  }, []);

  const updatePointer = useCallback(
    (clientX: number) => {
      updateHover(clientX);
      if (!draggingRef.current) return;
      const target = targetFromPointer(clientX);
      if (target === null) return;
      dragTargetRef.current = target;
      previewSeek(target);
    },
    [previewSeek, targetFromPointer, updateHover],
  );

  const schedulePointerUpdate = useCallback(
    (clientX: number) => {
      pendingPointerXRef.current = clientX;
      const now = performance.now();
      const elapsed = now - lastPointerUpdateAtRef.current;

      if (
        pointerTimerRef.current === null &&
        elapsed >= POINTER_UPDATE_INTERVAL_MS
      ) {
        pendingPointerXRef.current = null;
        lastPointerUpdateAtRef.current = now;
        updatePointer(clientX);
        return;
      }

      if (pointerTimerRef.current !== null) return;
      pointerTimerRef.current = window.setTimeout(
        () => {
          pointerTimerRef.current = null;
          const latestClientX = pendingPointerXRef.current;
          pendingPointerXRef.current = null;
          if (latestClientX === null) return;
          lastPointerUpdateAtRef.current = performance.now();
          updatePointer(latestClientX);
        },
        Math.max(0, POINTER_UPDATE_INTERVAL_MS - elapsed),
      );
    },
    [updatePointer],
  );

  const flushPointerUpdate = useCallback(() => {
    if (pointerTimerRef.current !== null) {
      window.clearTimeout(pointerTimerRef.current);
      pointerTimerRef.current = null;
    }
    const latestClientX = pendingPointerXRef.current;
    pendingPointerXRef.current = null;
    if (latestClientX === null) return;
    lastPointerUpdateAtRef.current = performance.now();
    updatePointer(latestClientX);
  }, [updatePointer]);

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
      // Slow movement remains immediate. During a fast pointer stream, keep
      // only the latest coordinate and update at most once per display frame.
      // A timer is used instead of requestAnimationFrame because WKWebView can
      // defer animation frames while it is dispatching pointer events.
      schedulePointerUpdate(event.clientX);
    },
    [schedulePointerUpdate],
  );

  const handlePointerLeave = useCallback(() => {
    if (draggingRef.current) return;
    pendingPointerXRef.current = null;
    if (pointerTimerRef.current !== null) {
      window.clearTimeout(pointerTimerRef.current);
      pointerTimerRef.current = null;
    }
    if (hoverMarkerRef.current) hoverMarkerRef.current.hidden = true;
    if (hoverTimeRef.current) hoverTimeRef.current.hidden = true;
  }, []);

  const finishPointerInteraction = useCallback(
    (event: React.PointerEvent<HTMLCanvasElement>, shouldCommit: boolean) => {
      if (!draggingRef.current) return;
      // Pointer-up can arrive before the 60 Hz timer. Apply the latest queued
      // coordinate first so the committed seek never trails behind the cursor.
      flushPointerUpdate();
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
    [commitSeek, flushPointerUpdate],
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
    if (!waveform || widthRef.current <= 0) return;
    propsRef.current.onRequestRefetch?.(
      propsRef.current.targetPeaksForWidth(widthRef.current),
    );
  }, [waveform]);

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
      // Keep the progress bar independent from WebKit's animation-frame
      // scheduling during pointer tracking.
      renderPlayhead();
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
  }, [renderPlayhead]);

  // Measure the canvas and request a fitting resolution level on resize.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const measure = (width: number, height: number) => {
      widthRef.current = Math.round(width);
      heightRef.current = Math.round(height);
      interactionRectRef.current = canvas.getBoundingClientRect();
      geometryRef.current = null;
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
      if (drawRafRef.current !== null)
        window.cancelAnimationFrame(drawRafRef.current);
      drawRafRef.current = null;
      if (pointerTimerRef.current !== null)
        window.clearTimeout(pointerTimerRef.current);
      pointerTimerRef.current = null;
      pendingPointerXRef.current = null;
    };
  }, []);

  return (
    <div className="waveform-interaction">
      <canvas
        ref={canvasRef}
        className={`waveform-canvas-surface${
          waveform ? " waveform-canvas-surface--revealing" : ""
        }`}
        role="slider"
        aria-label={ariaLabel}
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

interface CachedGeometry extends EnvelopeGeometry {
  source: WaveformLevel | null;
  width: number;
  height: number;
}
