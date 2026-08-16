import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";
import { onPosition } from "../../api/playbackEvents";
import type { WaveformLevel } from "../../api/waveform";
import {
  buildEnvelope,
  clampViewport,
  defaultTargetPeaksForWidth,
  drawEnvelope,
  positionMsForViewportX,
  viewportXForPositionMs,
  MIN_VIEWPORT_MS,
  pinchZoomFactor,
  resolveTokens,
  type Canvas2D,
  type EnvelopeGeometry,
  type WaveformStyle,
  type WaveformTokens,
  type WaveformViewport,
} from "./waveformRenderer";

export interface WaveformCanvasProps {
  /** Stable file identity used to reset state before new waveform data arrives. */
  trackId?: string | null;
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
  /** Placed A/B points in milliseconds; a null side is not yet placed. */
  abPoints?: { startMs: number | null; endMs: number | null };
  /** Region confirmed by the Rust engine; null until setLoopRegion resolves. */
  loopRegion?: { startMs: number; endMs: number } | null;
  /** Repositions an A/B point (drag on a marker). */
  onSetAbPoint?: (
    point: "a" | "b",
    positionMs: number,
  ) => void | Promise<boolean>;
  onViewportChange?: (viewport: WaveformViewport) => void;
  /** Shows temporal zoom controls for waveform mode only. */
  showZoomControls?: boolean;
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

export interface WaveformCanvasHandle {
  /** Returns the position the canvas currently displays (ms), or null. */
  getPlayheadPosition: () => number | null;
}

/**
 * Canvas 2D waveform renderer with an imperative playhead.
 *
 * Playback position is consumed directly from the throttled position event
 * stream and is never stored in React state, so high-frequency position
 * updates do not re-render the component. Resize re-requests a resolution
 * level that fits the new width.
 */
export const WaveformCanvas = forwardRef<
  WaveformCanvasHandle,
  WaveformCanvasProps
>(function WaveformCanvas(
  {
    trackId,
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
    abPoints = { startMs: null, endMs: null },
    loopRegion = null,
    onSetAbPoint,
    onViewportChange,
    showZoomControls = false,
  }: WaveformCanvasProps,
  ref,
) {
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
  const startMarkerRef = useRef<HTMLSpanElement | null>(null);
  const endMarkerRef = useRef<HTMLSpanElement | null>(null);
  const abDragRef = useRef<{ point: "a" | "b"; lastMs: number } | null>(null);
  const abPointsRef = useRef(abPoints);
  const viewportRef = useRef<WaveformViewport>({
    startMs: 0,
    endMs: durationMs ?? 0,
  });
  const [, setViewport] = useState<WaveformViewport>(viewportRef.current);
  const panRef = useRef<{
    startX: number;
    viewport: WaveformViewport;
    moved: boolean;
  } | null>(null);
  const pointersRef = useRef(new Map<number, number>());
  const pinchRef = useRef<{
    distance: number;
    viewport: WaveformViewport;
  } | null>(null);

  // Exposes the live visual playhead (position events, drag preview, and the
  // restored position) so A/B placement can target exactly where the user
  // sees the marker, without waiting for a React re-render.
  useImperativeHandle(
    ref,
    () => ({
      getPlayheadPosition: () =>
        positionRef.current ?? restoredPositionMs ?? null,
    }),
    [restoredPositionMs],
  );

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
    onSetAbPoint,
    onViewportChange,
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
      onSetAbPoint,
      onViewportChange,
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
    onSetAbPoint,
    onViewportChange,
  ]);
  useEffect(() => {
    abPointsRef.current = abPoints;
  }, [abPoints]);

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
        : viewportXForPositionMs(
            positionMs,
            viewportRef.current.startMs,
            viewportRef.current.endMs,
            width,
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
        ? buildEnvelope(
            waveform,
            width,
            height,
            null,
            effectiveDurationMs,
            viewportRef.current,
          )
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

  const updateViewport = useCallback(
    (next: WaveformViewport) => {
      const duration = durationRef.current;
      if (duration === null || duration <= 0) return;
      const resolved = clampViewport(next, duration, MIN_VIEWPORT_MS);
      viewportRef.current = resolved;
      setViewport(resolved);
      onViewportChange?.(resolved);
      geometryRef.current = null;
      scheduleDraw();
    },
    [onViewportChange, scheduleDraw],
  );

  const zoomAt = useCallback(
    (factor: number, clientX?: number) => {
      const duration = durationRef.current;
      const canvas = canvasRef.current;
      if (!duration || !canvas) return;
      const rect = interactionRectRef.current ?? canvas.getBoundingClientRect();
      const current = viewportRef.current;
      const anchor =
        clientX === undefined
          ? (current.startMs + current.endMs) / 2
          : (positionMsForViewportX(
              clientX - rect.left,
              rect.width,
              current.startMs,
              current.endMs,
            ) ?? (current.startMs + current.endMs) / 2);
      const span = Math.max(
        MIN_VIEWPORT_MS,
        Math.min(duration, (current.endMs - current.startMs) * factor),
      );
      const ratio =
        (anchor - current.startMs) / (current.endMs - current.startMs);
      updateViewport({
        startMs: anchor - ratio * span,
        endMs: anchor + (1 - ratio) * span,
      });
    },
    [updateViewport],
  );

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
    return positionMsForViewportX(
      clientX - rect.left,
      rect.width,
      viewportRef.current.startMs,
      viewportRef.current.endMs,
    );
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
    const target = positionMsForViewportX(
      clientX - rect.left,
      rect.width,
      viewportRef.current.startMs,
      viewportRef.current.endMs,
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

  // ── A/B marker drag ───────────────────────────────────────────────────
  // Dragging a marker repositions that point. The dragged side is clamped so
  // it can never cross the other side (A ≤ B-1, B ≥ A+1), keeping every
  // committed pair valid for the engine.
  const clampAbPosition = useCallback(
    (rawMs: number, point: "a" | "b"): number => {
      const duration = durationRef.current;
      const { startMs, endMs } = abPointsRef.current;
      if (point === "a") {
        const max = endMs !== null ? Math.max(0, endMs - 1) : (duration ?? 0);
        return Math.max(0, Math.min(rawMs, max));
      }
      const min = startMs !== null ? Math.min(duration ?? 0, startMs + 1) : 0;
      return Math.max(min, Math.min(rawMs, duration ?? 0));
    },
    [],
  );

  const previewAbMarker = useCallback(
    (marker: HTMLElement | null, ms: number, duration: number) => {
      if (!marker || duration <= 0) return;
      marker.style.setProperty(
        "--ab-x",
        String(
          (viewportXForPositionMs(
            ms,
            viewportRef.current.startMs,
            viewportRef.current.endMs,
            100,
          ) /
            100) *
            100,
        ),
      );
    },
    [],
  );

  const handleAbMarkerPointerDown = useCallback(
    (event: React.PointerEvent<HTMLSpanElement>, point: "a" | "b") => {
      if (durationRef.current === null) return;
      event.preventDefault();
      event.stopPropagation();
      try {
        event.currentTarget.setPointerCapture(event.pointerId);
      } catch {
        // Pointer capture is best-effort in non-browser environments.
      }
      const raw = targetFromPointer(event.clientX);
      if (raw === null) return;
      const clamped = clampAbPosition(raw, point);
      abDragRef.current = { point, lastMs: clamped };
      previewAbMarker(event.currentTarget, clamped, durationRef.current);
    },
    [clampAbPosition, previewAbMarker, targetFromPointer],
  );

  const handleAbMarkerPointerMove = useCallback(
    (event: React.PointerEvent<HTMLSpanElement>) => {
      const drag = abDragRef.current;
      if (!drag) return;
      const raw = targetFromPointer(event.clientX);
      if (raw === null) return;
      const clamped = clampAbPosition(raw, drag.point);
      drag.lastMs = clamped;
      previewAbMarker(event.currentTarget, clamped, durationRef.current ?? 0);
    },
    [clampAbPosition, previewAbMarker, targetFromPointer],
  );

  const handleAbMarkerPointerUp = useCallback(
    (event: React.PointerEvent<HTMLSpanElement>) => {
      const drag = abDragRef.current;
      abDragRef.current = null;
      if (!drag) return;
      try {
        event.currentTarget.releasePointerCapture(event.pointerId);
      } catch {
        // Pointer capture is best-effort.
      }
      propsRef.current.onSetAbPoint?.(drag.point, drag.lastMs);
    },
    [],
  );

  const handleAbMarkerPointerCancel = useCallback(
    (event: React.PointerEvent<HTMLSpanElement>) => {
      const drag = abDragRef.current;
      abDragRef.current = null;
      try {
        event.currentTarget.releasePointerCapture(event.pointerId);
      } catch {
        // Pointer capture is best-effort.
      }
      const duration = durationRef.current;
      if (duration === null) return;
      const points = abPointsRef.current;
      const ms = drag?.point === "a" ? points.startMs : points.endMs;
      if (ms !== null) previewAbMarker(event.currentTarget, ms, duration);
    },
    [previewAbMarker],
  );

  const handleAbMarkerKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLSpanElement>, point: "a" | "b") => {
      const current =
        point === "a" ? abPointsRef.current.startMs : abPointsRef.current.endMs;
      if (current === null) return;
      const step = event.shiftKey ? 100 : 1;
      let target: number | null = null;
      if (event.key === "ArrowLeft" || event.key === "ArrowDown") {
        target = current - step;
      } else if (event.key === "ArrowRight" || event.key === "ArrowUp") {
        target = current + step;
      } else if (event.key === "Home") {
        target = 0;
      } else if (event.key === "End") {
        target = durationRef.current;
      }
      if (target === null) return;
      event.preventDefault();
      propsRef.current.onSetAbPoint?.(point, clampAbPosition(target, point));
    },
    [clampAbPosition],
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
      pointersRef.current.set(event.pointerId, event.clientX);
      if (pointersRef.current.size === 2) {
        const xs = [...pointersRef.current.values()];
        pinchRef.current = {
          distance: Math.abs(xs[1] - xs[0]) || 1,
          viewport: viewportRef.current,
        };
        panRef.current = null;
        event.preventDefault();
        return;
      }
      if (
        viewportRef.current.endMs - viewportRef.current.startMs <
        durationRef.current
      ) {
        panRef.current = {
          startX: event.clientX,
          viewport: viewportRef.current,
          moved: false,
        };
        event.preventDefault();
        return;
      }
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
      if (pointersRef.current.has(event.pointerId))
        pointersRef.current.set(event.pointerId, event.clientX);
      const pinch = pinchRef.current;
      if (pinch && pointersRef.current.size >= 2) {
        const xs = [...pointersRef.current.values()];
        const distance = Math.abs(xs[1] - xs[0]) || 1;
        const factor = pinchZoomFactor(pinch.distance, distance);
        const center = (xs[0] + xs[1]) / 2;
        const rect = interactionRectRef.current;
        if (rect) {
          const anchor =
            positionMsForViewportX(
              center - rect.left,
              rect.width,
              pinch.viewport.startMs,
              pinch.viewport.endMs,
            ) ?? (pinch.viewport.startMs + pinch.viewport.endMs) / 2;
          const span = (pinch.viewport.endMs - pinch.viewport.startMs) * factor;
          const ratio =
            (anchor - pinch.viewport.startMs) /
            (pinch.viewport.endMs - pinch.viewport.startMs);
          updateViewport({
            startMs: anchor - ratio * span,
            endMs: anchor + (1 - ratio) * span,
          });
        }
        return;
      }
      const pan = panRef.current;
      if (pan) {
        const rect = interactionRectRef.current;
        const duration = durationRef.current;
        if (rect && duration) {
          pan.moved = pan.moved || Math.abs(event.clientX - pan.startX) > 3;
          const delta =
            ((event.clientX - pan.startX) / rect.width) *
            (pan.viewport.endMs - pan.viewport.startMs);
          updateViewport({
            startMs: pan.viewport.startMs - delta,
            endMs: pan.viewport.endMs - delta,
          });
        }
        return;
      }
      // Slow movement remains immediate. During a fast pointer stream, keep
      // only the latest coordinate and update at most once per display frame.
      // A timer is used instead of requestAnimationFrame because WKWebView can
      // defer animation frames while it is dispatching pointer events.
      schedulePointerUpdate(event.clientX);
    },
    [schedulePointerUpdate, updateViewport],
  );

  const handlePointerLeave = useCallback(() => {
    if (panRef.current) return;
    if (draggingRef.current) return;
    pendingPointerXRef.current = null;
    if (pointerTimerRef.current !== null) {
      window.clearTimeout(pointerTimerRef.current);
      pointerTimerRef.current = null;
    }
    if (hoverMarkerRef.current) hoverMarkerRef.current.hidden = true;
    if (hoverTimeRef.current) hoverTimeRef.current.hidden = true;
  }, []);

  const handleWheel = useCallback(
    (event: React.WheelEvent<HTMLCanvasElement>) => {
      event.preventDefault();
      zoomAt(event.deltaY < 0 ? 0.8 : 1.25, event.clientX);
    },
    [zoomAt],
  );

  const finishPan = useCallback(
    (event: React.PointerEvent<HTMLCanvasElement>) => {
      const pan = panRef.current;
      if (pan && !pan.moved) {
        const target = targetFromPointer(event.clientX);
        if (target !== null) commitSeek(target);
      }
      panRef.current = null;
      pointersRef.current.clear();
      pinchRef.current = null;
    },
    [commitSeek, targetFromPointer],
  );

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
  const lastTrackIdRef = useRef<string | null | undefined>(trackId);
  const receivedPositionEventRef = useRef(false);
  const lastResetRevisionRef = useRef(resetRevision);
  useEffect(() => {
    const trackChanged = lastTrackIdRef.current !== trackId;
    const waveformChanged = lastWaveformRef.current !== waveform;
    if (trackChanged) lastTrackIdRef.current = trackId;
    durationRef.current = durationMs;
    if ((waveformChanged || trackChanged) && durationMs !== null) {
      const nextViewport = { startMs: 0, endMs: durationMs };
      viewportRef.current = nextViewport;
      setViewport(nextViewport);
    } else if (durationMs !== null) {
      const nextViewport = clampViewport(viewportRef.current, durationMs);
      viewportRef.current = nextViewport;
      setViewport(nextViewport);
    }
    if (
      durationMs !== null &&
      viewportRef.current.endMs <= viewportRef.current.startMs
    ) {
      viewportRef.current = { startMs: 0, endMs: durationMs };
      setViewport(viewportRef.current);
    }
    if (waveformChanged) lastWaveformRef.current = waveform;
    if (trackChanged) {
      receivedPositionEventRef.current = false;
      positionRef.current = restoredPositionMs ?? 0;
    } else if (waveformChanged) {
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
  }, [trackId, waveform, durationMs, restoredPositionMs, style, scheduleDraw]);

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
        if (viewportRef.current.endMs <= viewportRef.current.startMs) {
          viewportRef.current = { startMs: 0, endMs: payload.duration_ms };
          setViewport(viewportRef.current);
        }
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
        onPointerUp={(event) => {
          finishPan(event);
          finishPointerInteraction(event, true);
        }}
        onPointerCancel={(event) => {
          finishPan(event);
          finishPointerInteraction(event, false);
        }}
        onPointerLeave={handlePointerLeave}
        onWheel={handleWheel}
        onKeyDown={handleKeyDown}
      />
      {showZoomControls ? (
        <div
          className="waveform-zoom-controls"
          aria-label="Waveform zoom controls"
        >
          <button
            type="button"
            aria-label="Zoom out waveform"
            onClick={() => zoomAt(1.25)}
          >
            −
          </button>
          <button
            type="button"
            aria-label="Zoom in waveform"
            onClick={() => zoomAt(0.8)}
          >
            +
          </button>
          <button
            type="button"
            aria-label="Reset waveform zoom"
            onClick={() =>
              updateViewport({ startMs: 0, endMs: durationMs ?? 0 })
            }
            disabled={
              viewportRef.current.startMs === 0 &&
              viewportRef.current.endMs === (durationMs ?? 0)
            }
          >
            Reset
          </button>
        </div>
      ) : null}
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
      {renderAbOverlays(abPoints, loopRegion, durationMs, viewportRef.current, {
        startRef: startMarkerRef,
        endRef: endMarkerRef,
        onPointerDown: handleAbMarkerPointerDown,
        onPointerMove: handleAbMarkerPointerMove,
        onPointerUp: handleAbMarkerPointerUp,
        onPointerCancel: handleAbMarkerPointerCancel,
        onKeyDown: handleAbMarkerKeyDown,
      })}
    </div>
  );
});

interface AbMarkerDragHandlers {
  startRef: React.RefObject<HTMLSpanElement | null>;
  endRef: React.RefObject<HTMLSpanElement | null>;
  onPointerDown: (
    event: React.PointerEvent<HTMLSpanElement>,
    point: "a" | "b",
  ) => void;
  onPointerMove: (event: React.PointerEvent<HTMLSpanElement>) => void;
  onPointerUp: (event: React.PointerEvent<HTMLSpanElement>) => void;
  onPointerCancel: (event: React.PointerEvent<HTMLSpanElement>) => void;
  onKeyDown: (
    event: React.KeyboardEvent<HTMLSpanElement>,
    point: "a" | "b",
  ) => void;
}

function renderAbOverlays(
  points: { startMs: number | null; endMs: number | null },
  region: { startMs: number; endMs: number } | null,
  durationMs: number | null,
  viewport: WaveformViewport,
  drag: AbMarkerDragHandlers,
): ReactNode {
  if (durationMs === null || durationMs <= 0) return null;
  const percent = (ms: number) =>
    (viewportXForPositionMs(ms, viewport.startMs, viewport.endMs, 100) / 100) *
    100;
  const startConfirmed =
    region !== null &&
    points.startMs !== null &&
    region.startMs === points.startMs &&
    region.endMs === points.endMs;
  const endConfirmed =
    region !== null && points.endMs !== null && region.endMs === points.endMs;
  const hasCompletePoints =
    points.startMs !== null &&
    points.endMs !== null &&
    points.startMs < points.endMs;
  const bandStartMs = points.startMs ?? 0;
  const bandEndMs = points.endMs ?? bandStartMs;
  const visibleBandStart = Math.max(bandStartMs, viewport.startMs);
  const visibleBandEnd = Math.min(bandEndMs, viewport.endMs);
  const bandVisible = hasCompletePoints && visibleBandEnd > visibleBandStart;
  return (
    <>
      {points.startMs !== null ? (
        <span
          ref={drag.startRef}
          className={`waveform-ab-marker waveform-ab-marker--start${
            startConfirmed ? "" : " waveform-ab-marker--pending"
          }`}
          data-testid="waveform-ab-start"
          role="slider"
          tabIndex={0}
          aria-label="A point"
          aria-valuemin={0}
          aria-valuemax={points.endMs === null ? durationMs : points.endMs - 1}
          aria-valuenow={points.startMs}
          style={{ "--ab-x": percent(points.startMs) } as CSSProperties}
          onPointerDown={(event) => drag.onPointerDown(event, "a")}
          onPointerMove={drag.onPointerMove}
          onPointerUp={drag.onPointerUp}
          onPointerCancel={drag.onPointerCancel}
          onKeyDown={(event) => drag.onKeyDown(event, "a")}
        />
      ) : null}
      {points.endMs !== null ? (
        <span
          ref={drag.endRef}
          className={`waveform-ab-marker waveform-ab-marker--end${
            endConfirmed ? "" : " waveform-ab-marker--pending"
          }`}
          data-testid="waveform-ab-end"
          role="slider"
          tabIndex={0}
          aria-label="B point"
          aria-valuemin={points.startMs === null ? 0 : points.startMs + 1}
          aria-valuemax={durationMs}
          aria-valuenow={points.endMs}
          style={{ "--ab-x": percent(points.endMs) } as CSSProperties}
          onPointerDown={(event) => drag.onPointerDown(event, "b")}
          onPointerMove={drag.onPointerMove}
          onPointerUp={drag.onPointerUp}
          onPointerCancel={drag.onPointerCancel}
          onKeyDown={(event) => drag.onKeyDown(event, "b")}
        />
      ) : null}
      {bandVisible ? (
        <span
          className="waveform-ab-band"
          data-testid="waveform-ab-band"
          aria-hidden="true"
          style={
            {
              "--ab-x": percent(visibleBandStart),
              "--ab-width": `${percent(visibleBandEnd) - percent(visibleBandStart)}`,
            } as CSSProperties
          }
        />
      ) : null}
    </>
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
