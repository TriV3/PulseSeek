import { useCallback, useEffect, useRef } from "react";
import { onPosition } from "../../api/playbackEvents";
import type { WaveformLevel } from "../../api/waveform";
import {
  buildEnvelope,
  defaultTargetPeaksForWidth,
  drawEnvelope,
  resolveTokens,
  type Canvas2D,
  type WaveformTokens,
} from "./waveformRenderer";

export interface WaveformCanvasProps {
  waveform: WaveformLevel | null;
  durationMs: number | null;
  onRequestRefetch?: (targetPeaks: number) => void;
  targetPeaksForWidth?: (widthPx: number) => number;
  getTokens?: (scope: Element | null | undefined) => WaveformTokens;
}

const REFETCH_DEBOUNCE_MS = 200;

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
  onRequestRefetch,
  targetPeaksForWidth = defaultTargetPeaksForWidth,
  getTokens = resolveTokens,
}: WaveformCanvasProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const positionRef = useRef<number | null>(null);
  const widthRef = useRef(0);
  const heightRef = useRef(0);
  const rafRef = useRef<number | null>(null);
  const refetchTimer = useRef<number | null>(null);

  const propsRef = useRef({
    waveform,
    durationMs,
    onRequestRefetch,
    targetPeaksForWidth,
    getTokens,
  });
  useEffect(() => {
    propsRef.current = {
      waveform,
      durationMs,
      onRequestRefetch,
      targetPeaksForWidth,
      getTokens,
    };
  }, [waveform, durationMs, onRequestRefetch, targetPeaksForWidth, getTokens]);

  const draw = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d") as unknown as Canvas2D | null;
    if (!ctx) return;
    const width = widthRef.current;
    const height = heightRef.current;
    if (width <= 0 || height <= 0) return;

    const { waveform, durationMs, getTokens } = propsRef.current;
    const tokens = getTokens(canvas);
    const geometry = waveform
      ? buildEnvelope(waveform, width, height, positionRef.current, durationMs)
      : { channels: [], playheadX: null };
    drawEnvelope(ctx, geometry, tokens, width, height);
  }, []);

  const scheduleDraw = useCallback(() => {
    if (rafRef.current !== null) return;
    rafRef.current = window.requestAnimationFrame(() => {
      rafRef.current = null;
      draw();
    });
  }, [draw]);

  // Re-draw when waveform data or duration changes (low frequency). The
  // playhead is reset first so a previous file's position never paints onto
  // the new waveform.
  useEffect(() => {
    positionRef.current = null;
    scheduleDraw();
  }, [waveform, durationMs, scheduleDraw]);

  // Position events drive the playhead imperatively, never via React state.
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void onPosition((payload) => {
      if (disposed) return;
      positionRef.current = payload.position_ms;
      scheduleDraw();
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
  }, [scheduleDraw]);

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
  // paints to a detached canvas.
  useEffect(() => {
    return () => {
      if (rafRef.current !== null) window.cancelAnimationFrame(rafRef.current);
    };
  }, []);

  return <canvas ref={canvasRef} className="waveform-canvas-surface" />;
}
