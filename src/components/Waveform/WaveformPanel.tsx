import { useState } from "react";
import { useWaveform } from "../../hooks/useWaveform";
import type { PlayableFileMetadata } from "../FolderTree/folderTreeTypes";
import { LinearAnalyzerCanvas } from "../LinearAnalyzer/LinearAnalyzerCanvas";
import { LogAnalyzerCanvas } from "../LogAnalyzer/LogAnalyzerCanvas";
import { MusicalSpectrumCanvas } from "../MusicalSpectrum/MusicalSpectrumCanvas";
import type { ResolvedTheme } from "../../hooks/useTheme";
import type { VisualizationMode } from "../VisualizationSelector/VisualizationSelector";
import type { WaveformStyle } from "./waveformRenderer";
import { WaveformCanvas } from "./WaveformCanvas";

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
  resetRevision?: number;
  /** Seeks playback to a millisecond position (confirmed by the backend). */
  onSeek?: (positionMs: number) => void | Promise<void>;
  /** Renderer style for the envelope. */
  style?: WaveformStyle;
  /** Resolved semantic theme used to repaint the analyzer immediately. */
  theme?: ResolvedTheme;
  /** The single visualization rendered in this workspace. */
  visualization?: VisualizationMode;
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
  resetRevision = 0,
  onSeek,
  style = "outline",
  theme = "light",
  visualization = "waveform",
}: WaveformPanelProps) {
  const [resolution, setResolution] = useState<{
    entryPath: string | null;
    targetPeaks: number;
  }>({ entryPath: null, targetPeaks: INITIAL_TARGET_PEAKS });
  const targetPeaks =
    resolution.entryPath === entryPath
      ? resolution.targetPeaks
      : INITIAL_TARGET_PEAKS;
  const waveformPath = visualization === "waveform" ? entryPath : null;
  const { status, waveform, error } = useWaveform(waveformPath, targetPeaks);

  return (
    <section className="waveform-panel" aria-label="Audio visualization">
      <header className="now-playing">
        <div>
          <span className="now-playing-label">Play:</span>{" "}
          <strong>{entryName}</strong>
        </div>
        <div className="brand-mark" aria-label="PulseSeek">
          <span className="brand-wave" aria-hidden="true">
            ∿
          </span>
          pulseseek
        </div>
      </header>
      <div className="audio-summary">{formatAudioSummary(metadata)}</div>
      <div className="visualization-workspace">
        {visualization === "waveform" ? (
          <div className="waveform-canvas">
            {status === "error" ? (
              <p className="waveform-error" role="alert">
                {error}
              </p>
            ) : (
              <WaveformCanvas
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
                style={style}
              />
            )}
          </div>
        ) : visualization === "logarithmic" ? (
          <LogAnalyzerCanvas
            enabled
            theme={theme}
            durationMs={durationMs}
            restoredPositionMs={restoredPositionMs}
            resetRevision={resetRevision}
            onSeek={onSeek}
          />
        ) : visualization === "linear" ? (
          <LinearAnalyzerCanvas
            enabled
            theme={theme}
            durationMs={durationMs}
            restoredPositionMs={restoredPositionMs}
            resetRevision={resetRevision}
            onSeek={onSeek}
          />
        ) : (
          <MusicalSpectrumCanvas
            enabled
            theme={theme}
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
