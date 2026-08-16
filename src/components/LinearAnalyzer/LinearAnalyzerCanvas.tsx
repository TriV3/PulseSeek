import { useCallback, useEffect, useRef } from "react";
import {
  onSpectrumFrame,
  type SpectrumFramePayload,
} from "../../api/playbackEvents";
import type { ResolvedTheme } from "../../hooks/useTheme";
import { WaveformCanvas } from "../Waveform/WaveformCanvas";
import {
  drawLinearAnalyzer,
  type LinearAnalyzerCanvas2D,
  type LinearAnalyzerTokens,
} from "./linearAnalyzerRenderer";
import "./LinearAnalyzerCanvas.css";

export interface LinearAnalyzerCanvasProps {
  enabled: boolean;
  theme: ResolvedTheme;
  trackId?: string | null;
  durationMs?: number | null;
  restoredPositionMs?: number;
  resetRevision?: number;
  onSeek?: (positionMs: number) => void | Promise<void>;
}

export function LinearAnalyzerCanvas({
  enabled,
  theme,
  trackId,
  durationMs = null,
  restoredPositionMs = 0,
  resetRevision = 0,
  onSeek,
}: LinearAnalyzerCanvasProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const frameRef = useRef<SpectrumFramePayload | null>(null);
  const widthRef = useRef(0);
  const heightRef = useRef(0);
  const animationRef = useRef<number | null>(null);

  const draw = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas || widthRef.current <= 0 || heightRef.current <= 0) return;
    const context = canvas.getContext(
      "2d",
    ) as unknown as LinearAnalyzerCanvas2D | null;
    if (!context) return;
    drawLinearAnalyzer(
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
    void onSpectrumFrame((payload) => {
      if (disposed) return;
      frameRef.current = payload;
      // Do not depend on requestAnimationFrame here: WKWebView can defer it
      // during pointer tracking, which makes the analyzer appear frozen.
      draw();
    })
      .then((cleanup) => {
        if (disposed) cleanup();
        else unlisten = cleanup;
      })
      .catch(() => {
        // Visualization is optional; playback and the static grid remain usable.
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

  const label = enabled
    ? "Linear frequency analyzer"
    : "Linear frequency analyzer disabled";
  return (
    <div className="linear-analyzer" data-enabled={enabled}>
      <canvas
        ref={canvasRef}
        className="linear-analyzer-canvas"
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
          ariaLabel="Linear analyzer seek"
        />
      )}
      {!enabled && (
        <span className="linear-analyzer-disabled">Analyzer disabled</span>
      )}
    </div>
  );
}

function analyzerTokens(element: Element): LinearAnalyzerTokens {
  const style = getComputedStyle(element);
  const token = (name: string) => style.getPropertyValue(name).trim();
  return {
    spectrum: token("--analyzer-spectrum"),
    spectrumSoft: token("--analyzer-spectrum-soft"),
    grid: token("--analyzer-grid"),
    label: token("--analyzer-label"),
  };
}
