import { useRef, useState, type ReactNode } from "react";
import { useWaveform } from "../../hooks/useWaveform";
import type { PlayableFileMetadata } from "../FolderTree/folderTreeTypes";
import { LinearAnalyzerCanvas } from "../LinearAnalyzer/LinearAnalyzerCanvas";
import { LogAnalyzerCanvas } from "../LogAnalyzer/LogAnalyzerCanvas";
import { MusicalSpectrumCanvas } from "../MusicalSpectrum/MusicalSpectrumCanvas";
import type { ResolvedTheme } from "../../hooks/useTheme";
import type { VisualizationMode } from "../VisualizationSelector/VisualizationSelector";
import type { WaveformStyle } from "./waveformRenderer";
import { WaveformCanvas, type WaveformCanvasHandle } from "./WaveformCanvas";

export interface WaveformPanelProps {
  /** Path of the selected file, or null when nothing is selected. */
  entryPath: string | null;
  /** Display name of the selected file. */
  entryName: string;
  /** Duration of the selected file, or null when unknown. */
  durationMs: number | null;
  /** Audio characteristics reported for the selected file. */
  metadata?: PlayableFileMetadata | null;
  /** Saved position shown before playback starts, without autoplay. */
  restoredPositionMs?: number;
  /** Current playhead used to place A/B points. Defaults to the restored one. */
  playheadPositionMs?: number;
  resetRevision?: number;
  /** Seeks playback to a millisecond position (confirmed by the backend). */
  onSeek?: (positionMs: number) => void | Promise<void>;
  seekStepMs?: number;
  /** Renderer style for the envelope. */
  style?: WaveformStyle;
  /** Resolved semantic theme used to repaint the analyzer immediately. */
  theme?: ResolvedTheme;
  /** The single visualization rendered in this workspace. */
  visualization?: VisualizationMode;
  /** Placed A/B points; a null side is not yet placed. */
  abPoints?: { startMs: number | null; endMs: number | null };
  /** Region confirmed by the Rust engine, or null when inactive. */
  loopRegion?: { startMs: number; endMs: number } | null;
  /** Placement error from the transport (reversed/equal points, rejection). */
  abError?: string | null;
  /** Places the A or B point at the given playhead position. */
  onSetAbPoint?: (
    point: "a" | "b",
    positionMs: number,
  ) => void | Promise<boolean>;
  /** Hides the A/B row and other chrome for the compact player mode. */
  compact?: boolean;
  /** Toggles the compact player mode from the header control. */
  onToggleCompact?: () => void;
  /** Application controls rendered beside compact toggle in both layouts. */
  headerActions?: ReactNode;
  /** Clears the confirmed region and placed points. */
  onClearAB?: () => void | Promise<boolean>;
}

/**
 * Seeds a resolution request that fits a typical desktop panel so the first
 * paint does not wait for the ResizeObserver round-trip; the canvas refines
 * the target once it measures its real width.
 */
const INITIAL_TARGET_PEAKS = 64;

function formatAudioSummary(
  metadata: PlayableFileMetadata | null | undefined,
): string {
  if (!metadata) return "Audio details unavailable";

  const characteristics: string[] = [];
  if (metadata.sample_rate !== null && metadata.sample_rate > 0) {
    characteristics.push(
      `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 3 }).format(metadata.sample_rate / 1_000)} kHz`,
    );
  }
  if (metadata.channels === 1) characteristics.push("mono");
  else if (metadata.channels === 2) characteristics.push("stereo");
  else if (metadata.channels !== null && metadata.channels > 0) {
    characteristics.push(`${metadata.channels} channels`);
  }

  const encoding: string[] = [];
  if (metadata.bit_depth !== null && metadata.bit_depth > 0) {
    encoding.push(`${metadata.bit_depth}-bit`);
  }
  const codec = metadata.codec?.trim();
  if (codec) encoding.push(codec);

  const groups = [characteristics.join(", "), encoding.join(" ")].filter(
    Boolean,
  );
  return groups.length > 0 ? groups.join(" · ") : "Audio details unavailable";
}

function formatAbTime(milliseconds: number | null): string {
  if (milliseconds === null) return "—";
  const seconds = Math.max(0, Math.floor(milliseconds / 1_000));
  const minutes = Math.floor(seconds / 60);
  return `${minutes}:${String(seconds % 60).padStart(2, "0")}`;
}

/**
 * Exclusive waveform/analyzer workspace region.
 *
 * Owns the waveform resolution selection and surfaces load errors to screen
 * readers. The playhead is drawn imperatively by {@link WaveformCanvas} and
 * never re-renders React.
 */
export function WaveformPanel({
  entryPath,
  entryName,
  durationMs,
  metadata,
  restoredPositionMs = 0,
  playheadPositionMs = restoredPositionMs,
  resetRevision = 0,
  onSeek,
  seekStepMs,
  style = "outline",
  theme = "light",
  visualization = "waveform",
  abPoints = { startMs: null, endMs: null },
  loopRegion = null,
  abError = null,
  onSetAbPoint,
  onClearAB,
  compact = false,
  onToggleCompact,
  headerActions,
}: WaveformPanelProps) {
  const [resolution, setResolution] = useState<{
    entryPath: string | null;
    targetPeaks: number;
  }>({ entryPath: null, targetPeaks: INITIAL_TARGET_PEAKS });
  const waveformHandleRef = useRef<WaveformCanvasHandle | null>(null);
  const targetPeaks =
    resolution.entryPath === entryPath
      ? resolution.targetPeaks
      : INITIAL_TARGET_PEAKS;
  const waveformPath = visualization === "waveform" ? entryPath : null;
  const { status, waveform, error } = useWaveform(waveformPath, targetPeaks);

  // A/B points are placed at the position the waveform canvas currently
  // displays (including drag previews and live position events), so the
  // recorded point always matches what the user sees.
  const placementPlayhead = (): number =>
    waveformHandleRef.current?.getPlayheadPosition() ?? playheadPositionMs;

  return (
    <section
      className={
        compact ? "waveform-panel waveform-panel--compact" : "waveform-panel"
      }
      aria-label="Audio visualization"
    >
      <header className="now-playing">
        <div className="now-playing-track">
          <span className="now-playing-label">Play:</span>{" "}
          <strong>{entryName}</strong>
        </div>
        <div className="now-playing-actions">
          {headerActions}
          <button
            type="button"
            className="compact-toggle"
            aria-pressed={compact}
            aria-label="Toggle compact mode"
            title="Toggle compact mode"
            onClick={onToggleCompact}
          >
            ⇲
          </button>
        </div>
      </header>
      <div className="audio-summary">{formatAudioSummary(metadata)}</div>
      {!compact && (
        <div className="ab-controls" aria-label="A-B repeat region">
          <button
            type="button"
            className="ab-control"
            onClick={() => void onSetAbPoint?.("a", placementPlayhead())}
            disabled={
              entryPath === null || durationMs === null || durationMs <= 0
            }
            title="Set A at the playhead position"
          >
            Set A point
          </button>
          <button
            type="button"
            className="ab-control"
            onClick={() => void onSetAbPoint?.("b", placementPlayhead())}
            disabled={
              entryPath === null || durationMs === null || durationMs <= 0
            }
            title="Set B at the playhead position"
          >
            Set B point
          </button>
          <span className="ab-readout" data-testid="ab-readout-start">
            A {formatAbTime(abPoints.startMs)}
          </span>
          <span className="ab-readout" data-testid="ab-readout-end">
            B {formatAbTime(abPoints.endMs)}
          </span>
          {loopRegion ? (
            <span className="ab-readout ab-readout--active">Looping A–B</span>
          ) : null}
          <button
            type="button"
            className="ab-control"
            onClick={() => void onClearAB?.()}
            disabled={
              abPoints.startMs === null &&
              abPoints.endMs === null &&
              loopRegion === null
            }
            title="Clear the A-B region"
          >
            Clear A-B
          </button>
          {abError ? (
            <p className="ab-controls-error" role="alert">
              {abError}
            </p>
          ) : null}
        </div>
      )}
      <div className="visualization-workspace">
        {visualization === "waveform" ? (
          <div className="waveform-canvas">
            {status === "error" ? (
              <p className="waveform-error" role="alert">
                {error}
              </p>
            ) : (
              <WaveformCanvas
                ref={waveformHandleRef}
                trackId={entryPath}
                waveform={waveform}
                durationMs={durationMs}
                restoredPositionMs={restoredPositionMs}
                resetRevision={resetRevision}
                onRequestRefetch={(nextTarget) => {
                  if (waveform && targetPeaks === INITIAL_TARGET_PEAKS) {
                    setResolution({ entryPath, targetPeaks: nextTarget });
                  }
                }}
                onSeek={onSeek}
                seekStepMs={seekStepMs}
                style={style}
                abPoints={abPoints}
                loopRegion={loopRegion}
                onSetAbPoint={onSetAbPoint}
                showZoomControls
              />
            )}
          </div>
        ) : visualization === "logarithmic" ? (
          <LogAnalyzerCanvas
            enabled
            theme={theme}
            trackId={entryPath}
            durationMs={durationMs}
            restoredPositionMs={restoredPositionMs}
            resetRevision={resetRevision}
            onSeek={onSeek}
          />
        ) : visualization === "linear" ? (
          <LinearAnalyzerCanvas
            enabled
            theme={theme}
            trackId={entryPath}
            durationMs={durationMs}
            restoredPositionMs={restoredPositionMs}
            resetRevision={resetRevision}
            onSeek={onSeek}
          />
        ) : (
          <MusicalSpectrumCanvas
            enabled
            theme={theme}
            trackId={entryPath}
            durationMs={durationMs}
            restoredPositionMs={restoredPositionMs}
            resetRevision={resetRevision}
            onSeek={onSeek}
          />
        )}
      </div>
    </section>
  );
}
