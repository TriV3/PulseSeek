import { useState } from "react";
import { useWaveform } from "../../hooks/useWaveform";
import { defaultTargetPeaksForWidth } from "./waveformRenderer";
import { WaveformCanvas } from "./WaveformCanvas";

export interface WaveformPanelProps {
  /** Path of the selected file, or null when nothing is selected. */
  entryPath: string | null;
  /** Display name of the selected file. */
  entryName: string;
  /** Duration of the selected file, or null when unknown. */
  durationMs: number | null;
}

/**
 * Seeds a resolution request that fits a typical desktop panel so the first
 * paint does not wait for the ResizeObserver round-trip; the canvas refines
 * the target once it measures its real width.
 */
const INITIAL_TARGET_PEAKS = defaultTargetPeaksForWidth(800);

/**
 * Waveform overview workspace region.
 *
 * Owns the waveform resolution selection and surfaces load errors to screen
 * readers. The playhead is drawn imperatively by {@link WaveformCanvas} and
 * never re-renders React.
 */
export function WaveformPanel({
  entryPath,
  entryName,
  durationMs,
}: WaveformPanelProps) {
  const [targetPeaks, setTargetPeaks] = useState(INITIAL_TARGET_PEAKS);
  const { status, waveform, error } = useWaveform(entryPath, targetPeaks);

  return (
    <section className="waveform-panel" aria-label="Waveform overview">
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
      <div className="audio-summary">44.1 kHz, stereo · lossless audio</div>
      <div className="waveform-canvas">
        {status === "error" ? (
          <p className="waveform-error" role="alert">
            {error}
          </p>
        ) : (
          <WaveformCanvas
            waveform={waveform}
            durationMs={durationMs}
            onRequestRefetch={setTargetPeaks}
          />
        )}
      </div>
    </section>
  );
}
