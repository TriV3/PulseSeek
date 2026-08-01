import type { TransportPlaybackStatus } from "../../hooks/usePlaybackTransport";
import "./PlayerTransport.css";

interface PlayerTransportProps {
  status: TransportPlaybackStatus;
  hasSelection: boolean;
  positionMs: number;
  durationMs: number | null;
  volume: number;
  muted: boolean;
  canPrevious: boolean;
  canNext: boolean;
  error: string | null;
  onTogglePlayPause: () => void | Promise<void>;
  onStop: () => void | Promise<void>;
  onPrevious: () => void | Promise<void>;
  onNext: () => void | Promise<void>;
  onSeek: (positionMs: number) => void | Promise<void>;
  onVolume: (volume: number) => void | Promise<void>;
  onToggleMute: () => void | Promise<void>;
}

function formatTime(milliseconds: number | null): string {
  if (milliseconds === null) return "—";
  const seconds = Math.max(0, Math.floor(milliseconds / 1_000));
  const minutes = Math.floor(seconds / 60);
  return `${minutes}:${String(seconds % 60).padStart(2, "0")}`;
}

export function PlayerTransport({
  status,
  hasSelection,
  positionMs,
  durationMs,
  volume,
  muted,
  canPrevious,
  canNext,
  error,
  onTogglePlayPause,
  onStop,
  onPrevious,
  onNext,
  onSeek,
  onVolume,
  onToggleMute,
}: PlayerTransportProps) {
  const playLabel = status === "playing" ? "Pause" : "Play";
  const seekMaximum = durationMs ?? 0;
  return (
    <section className="player-transport" aria-label="Player transport">
      <div className="player-transport-buttons">
        <button
          type="button"
          onClick={onPrevious}
          disabled={!canPrevious}
          aria-label="Previous"
          title="Previous"
        >
          ◀|
        </button>
        <button
          type="button"
          onClick={onTogglePlayPause}
          disabled={!hasSelection || status === "loading"}
          aria-label={playLabel}
          title={playLabel}
        >
          {status === "playing" ? "Ⅱ" : "▶"}
        </button>
        <button
          type="button"
          onClick={onStop}
          disabled={status === "idle"}
          aria-label="Stop"
          title="Stop"
        >
          ■
        </button>
        <button
          type="button"
          onClick={onNext}
          disabled={!canNext}
          aria-label="Next"
          title="Next"
        >
          |▶
        </button>
      </div>
      <div className="player-transport-position">
        <label htmlFor="playback-position">Playback position</label>
        <input
          id="playback-position"
          type="range"
          min={0}
          max={seekMaximum}
          value={Math.min(positionMs, seekMaximum)}
          disabled={durationMs === null}
          onChange={(event) => void onSeek(Number(event.target.value))}
        />
        <output>
          {formatTime(positionMs)} / {formatTime(durationMs)}
        </output>
      </div>
      <div className="player-transport-volume">
        <label htmlFor="playback-volume">Volume</label>
        <input
          id="playback-volume"
          type="range"
          min={0}
          max={100}
          value={Math.round(volume * 100)}
          onChange={(event) => void onVolume(Number(event.target.value) / 100)}
        />
        <button
          type="button"
          onClick={onToggleMute}
          aria-label={muted ? "Unmute" : "Mute"}
          title={muted ? "Unmute" : "Mute"}
        >
          {muted ? "×♪" : "♪"}
        </button>
      </div>
      {error ? (
        <p className="player-transport-error" role="alert">
          {error}
        </p>
      ) : null}
    </section>
  );
}
