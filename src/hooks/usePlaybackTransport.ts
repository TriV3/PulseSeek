import { useEffect, useLayoutEffect, useRef, useState } from "react";
import {
  pause,
  resume,
  seek,
  setVolume,
  stop,
  type PlaybackMode,
} from "../api/commandEnvelope";
import { onCompleted, onPosition, onStateChanged } from "../api/playbackEvents";
import type { BrowserEntry } from "../components/FolderTree/folderTreeTypes";

export type TransportPlaybackStatus =
  "idle" | "loading" | "playing" | "paused" | "failed";

interface PlaybackTransportOptions {
  entries: BrowserEntry[];
  selectedEntryId: string | null;
  playbackStatus: TransportPlaybackStatus;
  playbackGeneration?: number;
  playbackMode?: PlaybackMode;
  onSelectEntry: (entry: BrowserEntry) => void | Promise<void>;
}

export function usePlaybackTransport({
  entries,
  selectedEntryId,
  playbackStatus,
  playbackGeneration = 0,
  playbackMode = "one-shot",
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
    generation: number;
  } | null>(null);
  const playbackContext = useRef({
    entries,
    selectedEntryId,
    playbackGeneration,
    playbackMode,
    onSelectEntry,
  });
  useLayoutEffect(() => {
    playbackContext.current = {
      entries,
      selectedEntryId,
      playbackGeneration,
      playbackMode,
      onSelectEntry,
    };
  }, [
    entries,
    onSelectEntry,
    playbackGeneration,
    playbackMode,
    selectedEntryId,
  ]);

  useEffect(() => {
    let disposed = false;
    let unlistenState: (() => void) | undefined;
    let unlistenPosition: (() => void) | undefined;
    let unlistenCompleted: (() => void) | undefined;

    void Promise.resolve(
      onStateChanged((payload) => {
        if (disposed) return;
        if (payload.state === "stopped") {
          const context = playbackContext.current;
          setPositionMs(0);
          setDurationMs(null);
          setCommandStatus({
            entryId: context.selectedEntryId ?? "",
            status: "idle",
            generation: context.playbackGeneration,
          });
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
        setPositionEntryId(playbackContext.current.selectedEntryId);
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
    void Promise.resolve(
      onCompleted(() => {
        if (disposed) return;
        const context = playbackContext.current;
        const index = context.entries.findIndex(
          (entry) => entry.id === context.selectedEntryId,
        );
        const next =
          context.playbackMode === "sequential"
            ? index >= 0
              ? context.entries[index + 1]
              : undefined
            : context.playbackMode === "random"
              ? pickRandomEntry(context.entries, context.selectedEntryId)
              : undefined;
        if (next) void context.onSelectEntry(next);
      }),
    )
      .then((cleanup) => {
        if (disposed) cleanup();
        else unlistenCompleted = cleanup;
      })
      .catch(() => {
        if (!disposed) setError("Playback completion updates unavailable.");
      });

    return () => {
      disposed = true;
      unlistenState?.();
      unlistenPosition?.();
      unlistenCompleted?.();
    };
  }, []);

  const runCommand = async (command: () => Promise<void>) => {
    setError(null);
    try {
      await command();
      return true;
    } catch (cause: unknown) {
      setError(
        cause instanceof Error ? cause.message : "Playback command failed.",
      );
      return false;
    }
  };

  const selectedIndex = entries.findIndex(
    (entry) => entry.id === selectedEntryId,
  );
  const canPrevious = selectedIndex > 0;
  const canNext = selectedIndex >= 0 && selectedIndex < entries.length - 1;
  const effectiveStatus =
    commandStatus?.entryId === selectedEntryId &&
    commandStatus.generation === playbackGeneration
      ? commandStatus.status
      : playbackStatus;

  return {
    positionMs: positionEntryId === selectedEntryId ? positionMs : 0,
    durationMs: positionEntryId === selectedEntryId ? durationMs : null,
    volume,
    muted,
    error,
    status: effectiveStatus,
    hasSelection: selectedEntryId !== null,
    canPrevious,
    canNext,
    togglePlayPause: () => {
      if (effectiveStatus === "playing") {
        return runCommand(async () => {
          await pause();
          if (selectedEntryId) {
            setCommandStatus({
              entryId: selectedEntryId,
              status: "paused",
              generation: playbackGeneration,
            });
          }
        }).then(() => undefined);
      }
      if (effectiveStatus === "paused") {
        return runCommand(async () => {
          await resume();
          if (selectedEntryId) {
            setCommandStatus({
              entryId: selectedEntryId,
              status: "playing",
              generation: playbackGeneration,
            });
          }
        }).then(() => undefined);
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
          setCommandStatus({
            entryId: selectedEntryId,
            status: "idle",
            generation: playbackGeneration,
          });
        }
      }).then(() => undefined),
    handlePrevious: () => {
      if (canPrevious) return onSelectEntry(entries[selectedIndex - 1]);
      return Promise.resolve();
    },
    handleNext: () => {
      if (canNext) return onSelectEntry(entries[selectedIndex + 1]);
      return Promise.resolve();
    },
    handleSeek: async (nextPositionMs: number) => {
      let confirmedPosition: number | null = null;
      const succeeded = await runCommand(async () => {
        const actual = await seek(nextPositionMs);
        setPositionMs(actual);
        confirmedPosition = actual;
      });
      return succeeded ? confirmedPosition : null;
    },
    handleVolume: async (nextVolume: number) => {
      const bounded = Math.max(0, Math.min(1, nextVolume));
      const previousVolume = volume;
      setVolumeState(bounded);
      return runCommand(async () => {
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
      return runCommand(async () => {
        try {
          await setVolume(volume, nextMuted);
        } catch (error) {
          setMuted(previousMuted);
          throw error;
        }
      });
    },
    restoreVolume: async (nextVolume: number, nextMuted: boolean) => {
      const bounded = Math.max(0, Math.min(1, nextVolume));
      setVolumeState(bounded);
      setMuted(nextMuted);
      await runCommand(() => setVolume(bounded, nextMuted));
    },
    restorePosition: (
      entryId: string,
      nextPositionMs: number,
      nextDurationMs: number | null,
    ) => {
      setPositionEntryId(entryId);
      setPositionMs(nextPositionMs);
      setDurationMs(nextDurationMs);
    },
  };
}

function pickRandomEntry(
  entries: BrowserEntry[],
  currentEntryId: string | null,
): BrowserEntry | undefined {
  const alternatives = entries.filter((entry) => entry.id !== currentEntryId);
  const candidates = alternatives.length > 0 ? alternatives : entries;
  return candidates[Math.floor(Math.random() * candidates.length)];
}
