import { useCallback, useEffect, useRef } from "react";
import {
  onMusicalSpectrumFrame,
  type MusicalSpectrumFramePayload,
} from "../../api/playbackEvents";
import type { ResolvedTheme } from "../../hooks/useTheme";
import { WaveformCanvas } from "../Waveform/WaveformCanvas";
import {
  drawMusicalSpectrum,
  type MusicalSpectrumCanvas2D,
  type MusicalSpectrumTokens,
} from "./musicalSpectrumRenderer";
import "./MusicalSpectrumCanvas.css";

export interface MusicalSpectrumCanvasProps {
  enabled: boolean;
  theme: ResolvedTheme;
  trackId?: string | null;
  durationMs?: number | null;
  restoredPositionMs?: number;
  resetRevision?: number;
  onSeek?: (positionMs: number) => void | Promise<void>;
}

export function MusicalSpectrumCanvas({
  enabled,
  theme,
  trackId,
  durationMs = null,
  restoredPositionMs = 0,
  resetRevision = 0,
  onSeek,
}: MusicalSpectrumCanvasProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const frameRef = useRef<MusicalSpectrumFramePayload | null>(null);
  const widthRef = useRef(0);
  const heightRef = useRef(0);
  const animationRef = useRef<number | null>(null);

  const draw = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas || widthRef.current <= 0 || heightRef.current <= 0) return;
    const context = canvas.getContext(
      "2d",
    ) as unknown as MusicalSpectrumCanvas2D | null;
    if (!context) return;
    drawMusicalSpectrum(
      context,
      frameRef.current,
      widthRef.current,
      heightRef.current,
      analyzerTokens(canvas),
    );
  }, []);

  const scheduleDraw = useCallback(() => {
    if (animationRef.current !== null) return;
    animationRef.current = window.requestAnimationFrame(() => {
      animationRef.current = null;
      draw();
    });
  }, [draw]);

  useEffect(() => {
    if (!enabled) {
      frameRef.current = null;
      if (animationRef.current !== null) {
        window.cancelAnimationFrame(animationRef.current);
        animationRef.current = null;
      }
      const canvas = canvasRef.current;
      canvas?.getContext("2d")?.clearRect(0, 0, canvas.width, canvas.height);
      return;
    }
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void onMusicalSpectrumFrame((payload) => {
      if (disposed) return;
      frameRef.current = payload;
      draw();
    })
      .then((cleanup) => {
        if (disposed) cleanup();
        else unlisten = cleanup;
      })
      .catch(() => {
        // Visualization is optional; playback and seek remain available.
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [draw, enabled]);

  useEffect(() => {
    if (!enabled) return;
    const canvas = canvasRef.current;
    if (!canvas) return;
    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      const { width, height } = entry.contentRect;
      if (
        !Number.isFinite(width) ||
        !Number.isFinite(height) ||
        width <= 0 ||
        height <= 0
      ) {
        return;
      }
      widthRef.current = Math.round(width);
      heightRef.current = Math.round(height);
      canvas.width = Math.max(1, widthRef.current);
      canvas.height = Math.max(1, heightRef.current);
      scheduleDraw();
    });
    observer.observe(canvas);
    return () => observer.disconnect();
  }, [enabled, scheduleDraw]);

  useEffect(() => {
    if (enabled) scheduleDraw();
  }, [enabled, theme, scheduleDraw]);

  useEffect(
    () => () => {
      if (animationRef.current !== null) {
        window.cancelAnimationFrame(animationRef.current);
        animationRef.current = null;
      }
    },
    [],
  );

  const label = enabled ? "Musical spectrum" : "Musical spectrum disabled";
  return (
    <div className="musical-spectrum" data-enabled={enabled}>
      <canvas
        ref={canvasRef}
        className="musical-spectrum-canvas"
        role="img"
        aria-label={label}
      />
      {enabled && (
        <WaveformCanvas
          trackId={trackId}
          waveform={null}
          durationMs={durationMs}
          restoredPositionMs={restoredPositionMs}
          resetRevision={resetRevision}
          onSeek={onSeek}
          ariaLabel="Musical spectrum seek"
        />
      )}
      {!enabled && (
        <span className="musical-spectrum-disabled">Spectrum disabled</span>
      )}
    </div>
  );
}

function analyzerTokens(element: Element): MusicalSpectrumTokens {
  const style = getComputedStyle(element);
  const token = (name: string) => style.getPropertyValue(name).trim();
  return {
    spectrum: token("--analyzer-spectrum"),
    spectrumSoft: token("--analyzer-spectrum-soft"),
    grid: token("--analyzer-grid"),
    label: token("--analyzer-label"),
  };
}
