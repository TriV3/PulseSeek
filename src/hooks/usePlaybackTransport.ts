import { useEffect, useState } from "react";
import { pause, resume, seek, setVolume, stop } from "../api/commandEnvelope";
import { onPosition, onStateChanged } from "../api/playbackEvents";
import type { BrowserEntry } from "../components/FolderTree/folderTreeTypes";

export type TransportPlaybackStatus =
  "idle" | "loading" | "playing" | "paused" | "failed";

interface PlaybackTransportOptions {
  entries: BrowserEntry[];
  selectedEntryId: string | null;
  playbackStatus: TransportPlaybackStatus;
  onSelectEntry: (entry: BrowserEntry) => void | Promise<void>;
}

export function usePlaybackTransport({
  entries,
  selectedEntryId,
  playbackStatus,
  onSelectEntry,
}: PlaybackTransportOptions) {
  const [positionMs, setPositionMs] = useState(0);
  const [durationMs, setDurationMs] = useState<number | null>(null);
  const [positionEntryId, setPositionEntryId] = useState<string | null>(null);
  const [volume, setVolumeState] = useState(1);
  const [muted, setMuted] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [commandStatus, setCommandStatus] = useState<{
    entryId: string;
    status: "idle" | "playing" | "paused";
  } | null>(null);

  useEffect(() => {
    let disposed = false;
    let unlistenState: (() => void) | undefined;
    let unlistenPosition: (() => void) | undefined;

    void Promise.resolve(
      onStateChanged((payload) => {
        if (disposed) return;
        if (payload.state === "stopped") {
          setPositionMs(0);
          setDurationMs(null);
          setCommandStatus({ entryId: selectedEntryId ?? "", status: "idle" });
        }
        if (payload.state === "failed") setError("Playback failed.");
      }),
    )
      .then((cleanup) => {
        if (disposed) cleanup();
        else unlistenState = cleanup;
      })
      .catch(() => {
        if (!disposed) setError("Playback state updates unavailable.");
      });
    void Promise.resolve(
      onPosition((payload) => {
        if (disposed) return;
        setPositionEntryId(selectedEntryId);
        setPositionMs(payload.position_ms);
        setDurationMs(payload.duration_ms);
      }),
    )
      .then((cleanup) => {
        if (disposed) cleanup();
        else unlistenPosition = cleanup;
      })
      .catch(() => {
        if (!disposed) setError("Playback position updates unavailable.");
      });

    return () => {
      disposed = true;
      unlistenState?.();
      unlistenPosition?.();
    };
  }, [selectedEntryId]);

  const runCommand = async (command: () => Promise<void>) => {
    setError(null);
    try {
      await command();
    } catch (cause: unknown) {
      setError(
        cause instanceof Error ? cause.message : "Playback command failed.",
      );
    }
  };

  const selectedIndex = entries.findIndex(
    (entry) => entry.id === selectedEntryId,
  );
  const canPrevious = selectedIndex > 0;
  const canNext = selectedIndex >= 0 && selectedIndex < entries.length - 1;
  const effectiveStatus =
    commandStatus?.entryId === selectedEntryId
      ? commandStatus.status
      : playbackStatus;

  return {
    positionMs: positionEntryId === selectedEntryId ? positionMs : 0,
    durationMs: positionEntryId === selectedEntryId ? durationMs : null,
    volume,
    muted,
    error,
    status: effectiveStatus,
    canPrevious,
    canNext,
    togglePlayPause: () => {
      if (effectiveStatus === "playing") {
        return runCommand(async () => {
          await pause();
          if (selectedEntryId) {
            setCommandStatus({ entryId: selectedEntryId, status: "paused" });
          }
        });
      }
      if (effectiveStatus === "paused") {
        return runCommand(async () => {
          await resume();
          if (selectedEntryId) {
            setCommandStatus({ entryId: selectedEntryId, status: "playing" });
          }
        });
      }
      const selected = entries[selectedIndex];
      return selected ? onSelectEntry(selected) : Promise.resolve();
    },
    handleStop: () =>
      runCommand(async () => {
        await stop();
        setPositionMs(0);
        setDurationMs(null);
        if (selectedEntryId) {
          setCommandStatus({ entryId: selectedEntryId, status: "idle" });
        }
      }),
    handlePrevious: () => {
      if (canPrevious) return onSelectEntry(entries[selectedIndex - 1]);
      return Promise.resolve();
    },
    handleNext: () => {
      if (canNext) return onSelectEntry(entries[selectedIndex + 1]);
      return Promise.resolve();
    },
    handleSeek: async (nextPositionMs: number) => {
      await runCommand(async () => {
        const actual = await seek(nextPositionMs);
        setPositionMs(actual);
      });
    },
    handleVolume: async (nextVolume: number) => {
      const bounded = Math.max(0, Math.min(1, nextVolume));
      const previousVolume = volume;
      setVolumeState(bounded);
      await runCommand(async () => {
        try {
          await setVolume(bounded, muted);
        } catch (error) {
          setVolumeState(previousVolume);
          throw error;
        }
      });
    },
    toggleMute: async () => {
      const nextMuted = !muted;
      const previousMuted = muted;
      setMuted(nextMuted);
      await runCommand(async () => {
        try {
          await setVolume(volume, nextMuted);
        } catch (error) {
          setMuted(previousMuted);
          throw error;
        }
      });
    },
  };
}
