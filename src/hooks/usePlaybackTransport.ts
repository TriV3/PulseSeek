import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  pause,
  resume,
  seek,
  setLoopRegion,
  clearLoopRegion,
  setVolume,
  stop,
  type PlaybackMode,
} from "../api/commandEnvelope";
import { clearPrepared, prepareNext } from "../api/commandEnvelope";
import {
  onCompleted,
  onPosition,
  onStateChanged,
  onTrackChanged,
} from "../api/playbackEvents";
import type { BrowserEntry } from "../components/FolderTree/folderTreeTypes";

export type TransportPlaybackStatus =
  "idle" | "loading" | "playing" | "paused" | "failed";

interface PlaybackTransportOptions {
  entries: BrowserEntry[];
  selectedEntryId: string | null;
  playbackStatus: TransportPlaybackStatus;
  playbackGeneration?: number;
  playbackMode?: PlaybackMode;
  gaplessPlayback?: boolean;
  random?: () => number;
  onSelectEntry: (
    entry: BrowserEntry,
    options?: { alreadyPlaying?: boolean },
  ) => void | Promise<void>;
}

export function usePlaybackTransport({
  entries,
  selectedEntryId,
  playbackStatus,
  playbackGeneration = 0,
  playbackMode = "one-shot",
  gaplessPlayback = true,
  random = Math.random,
  onSelectEntry,
}: PlaybackTransportOptions) {
  const [positionMs, setPositionMs] = useState(0);
  const [durationMs, setDurationMs] = useState<number | null>(null);
  const [positionEntryId, setPositionEntryId] = useState<string | null>(null);
  const [volume, setVolumeState] = useState(1);
  const [muted, setMuted] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [abPoints, setAbPoints] = useState<{
    startMs: number | null;
    endMs: number | null;
  }>({ startMs: null, endMs: null });
  const [loopRegion, setLoopRegionState] = useState<{
    startMs: number;
    endMs: number;
  } | null>(null);
  const [abError, setAbError] = useState<string | null>(null);
  const [abEntryId, setAbEntryId] = useState<string | null>(null);
  const abEntryIdRef = useRef<string | null>(null);
  const abPointsRef = useRef(abPoints);
  const loopRegionRef = useRef(loopRegion);
  const abOperationRef = useRef(0);
  const activeSessionRef = useRef(
    playbackStatus === "playing" || playbackStatus === "paused",
  );
  const regionNeedsRestoreRef = useRef(false);

  // A–B points and the confirmed region are display state owned by the
  // transport. The engine is authoritative: a region is only shown after
  // `setLoopRegion` resolves, and an out-of-region seek (which clears the
  // engine region) also clears the markers.
  const resetABLocal = useCallback(() => {
    abPointsRef.current = { startMs: null, endMs: null };
    setAbPoints({ startMs: null, endMs: null });
    loopRegionRef.current = null;
    setLoopRegionState(null);
    abEntryIdRef.current = null;
    setAbEntryId(null);
    setAbError(null);
  }, []);

  const setAbPoint = useCallback(
    async (point: "a" | "b", positionMs: number): Promise<boolean> => {
      setAbError(null);
      const operation = ++abOperationRef.current;
      const operationEntryId = selectedEntryId;
      const previous =
        abEntryIdRef.current === operationEntryId
          ? abPointsRef.current
          : { startMs: null, endMs: null };
      if (abEntryIdRef.current !== operationEntryId) {
        loopRegionRef.current = null;
        setLoopRegionState(null);
      }
      const next = {
        ...previous,
        [point === "a" ? "startMs" : "endMs"]: positionMs,
      };
      abPointsRef.current = next;
      setAbPoints(next);
      abEntryIdRef.current = operationEntryId;
      setAbEntryId(operationEntryId);
      if (next.startMs === null || next.endMs === null) return false;
      if (next.startMs >= next.endMs) {
        abPointsRef.current = previous;
        setAbPoints(previous);
        setAbError("B point must be after the A point.");
        return false;
      }
      // Stop destroys the Rust playback worker. Keep a complete region as
      // local per-file state while stopped; it will be applied to the next
      // worker when playback starts again.
      if (!activeSessionRef.current) {
        loopRegionRef.current = { startMs: next.startMs, endMs: next.endMs };
        setLoopRegionState({ startMs: next.startMs, endMs: next.endMs });
        regionNeedsRestoreRef.current = true;
        return true;
      }
      try {
        await setLoopRegion(next.startMs, next.endMs);
        if (
          operation !== abOperationRef.current ||
          abEntryIdRef.current !== operationEntryId
        ) {
          return false;
        }
        loopRegionRef.current = { startMs: next.startMs, endMs: next.endMs };
        setLoopRegionState({ startMs: next.startMs, endMs: next.endMs });
        return true;
      } catch (cause) {
        if (
          operation !== abOperationRef.current ||
          abEntryIdRef.current !== operationEntryId
        ) {
          return false;
        }
        abPointsRef.current = previous;
        setAbPoints(previous);
        setAbError(
          cause instanceof Error
            ? cause.message
            : "Could not set the A-B region.",
        );
        return false;
      }
    },
    [selectedEntryId],
  );

  const clearAB = useCallback(async (): Promise<boolean> => {
    const operation = ++abOperationRef.current;
    const operationEntryId = abEntryIdRef.current;
    setAbError(null);
    try {
      if (activeSessionRef.current) await clearLoopRegion();
      if (
        operation !== abOperationRef.current ||
        abEntryIdRef.current !== operationEntryId
      ) {
        return false;
      }
      abPointsRef.current = { startMs: null, endMs: null };
      setAbPoints({ startMs: null, endMs: null });
      loopRegionRef.current = null;
      setLoopRegionState(null);
      abEntryIdRef.current = null;
      setAbEntryId(null);
      return true;
    } catch (cause) {
      if (
        operation !== abOperationRef.current ||
        abEntryIdRef.current !== operationEntryId
      ) {
        return false;
      }
      setAbError(
        cause instanceof Error
          ? cause.message
          : "Could not clear the A-B region.",
      );
      return false;
    }
  }, []);

  const toggleAbRepeat = useCallback(async (): Promise<boolean> => {
    if (loopRegionRef.current) return clearAB();
    const operation = ++abOperationRef.current;
    const operationEntryId = abEntryIdRef.current;
    const points = abPointsRef.current;
    if (
      points.startMs !== null &&
      points.endMs !== null &&
      points.startMs < points.endMs
    ) {
      if (!activeSessionRef.current) {
        loopRegionRef.current = {
          startMs: points.startMs,
          endMs: points.endMs,
        };
        setLoopRegionState({ startMs: points.startMs, endMs: points.endMs });
        regionNeedsRestoreRef.current = true;
        return true;
      }
      try {
        await setLoopRegion(points.startMs, points.endMs);
        if (
          operation !== abOperationRef.current ||
          abEntryIdRef.current !== operationEntryId
        ) {
          return false;
        }
        loopRegionRef.current = {
          startMs: points.startMs,
          endMs: points.endMs,
        };
        setLoopRegionState({ startMs: points.startMs, endMs: points.endMs });
        return true;
      } catch (cause) {
        if (
          operation !== abOperationRef.current ||
          abEntryIdRef.current !== operationEntryId
        ) {
          return false;
        }
        setAbError(
          cause instanceof Error
            ? cause.message
            : "Could not set the A-B region.",
        );
        return false;
      }
    }
    return false;
  }, [clearAB]);
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
    gaplessPlayback,
    random,
    onSelectEntry,
  });
  const appliedRegionGeneration = useRef<number | null>(null);
  const preparedNextKeyRef = useRef<string | null>(null);
  // Folder rows appear in the visible list only while a search is active;
  // playback navigation must never select a folder.
  const playableEntries = useMemo(
    () => entries.filter((entry) => entry.kind === "playable"),
    [entries],
  );
  useLayoutEffect(() => {
    if (
      abEntryIdRef.current !== null &&
      abEntryIdRef.current !== selectedEntryId
    ) {
      abOperationRef.current += 1;
      resetABLocal();
      regionNeedsRestoreRef.current = false;
      appliedRegionGeneration.current = null;
    }
    playbackContext.current = {
      entries: playableEntries,
      selectedEntryId,
      playbackGeneration,
      playbackMode,
      gaplessPlayback,
      random,
      onSelectEntry,
    };
  }, [
    playableEntries,
    onSelectEntry,
    playbackGeneration,
    playbackMode,
    gaplessPlayback,
    random,
    resetABLocal,
    selectedEntryId,
  ]);

  useEffect(() => {
    let disposed = false;
    let unlistenState: (() => void) | undefined;
    let unlistenPosition: (() => void) | undefined;
    let unlistenCompleted: (() => void) | undefined;
    let unlistenTrackChanged: (() => void) | undefined;

    void Promise.resolve(
      onStateChanged((payload) => {
        if (disposed) return;
        if (payload.state === "stopped") {
          const context = playbackContext.current;
          activeSessionRef.current = false;
          regionNeedsRestoreRef.current = loopRegionRef.current !== null;
          setPositionMs(0);
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
      onTrackChanged((payload) => {
        if (disposed) return;
        const context = playbackContext.current;
        if (context.playbackMode !== "sequential") return;
        const entry = context.entries.find(
          (candidate) => candidate.id === payload.path,
        );
        if (!entry) return;
        void context.onSelectEntry(entry, { alreadyPlaying: true });
      }),
    )
      .then((cleanup) => {
        if (disposed) cleanup();
        else unlistenTrackChanged = cleanup;
      })
      .catch(() => {
        if (!disposed) setError("Playback track updates unavailable.");
      });
    void Promise.resolve(
      onCompleted(() => {
        if (disposed) return;
        if (!activeSessionRef.current) return;
        const context = playbackContext.current;
        if (
          context.playbackGeneration !== playbackGeneration ||
          context.selectedEntryId !== selectedEntryId
        ) {
          return;
        }
        const index = context.entries.findIndex(
          (entry) => entry.id === context.selectedEntryId,
        );
        const next =
          context.playbackMode === "sequential"
            ? index >= 0
              ? context.entries[index + 1]
              : undefined
            : context.playbackMode === "random"
              ? pickRandomEntry(
                  context.entries,
                  context.selectedEntryId,
                  context.random,
                )
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
      unlistenTrackChanged?.();
    };
  }, [playbackGeneration, resetABLocal, selectedEntryId]);

  useEffect(() => {
    if (
      !gaplessPlayback ||
      playbackMode !== "sequential" ||
      playbackStatus !== "playing"
    ) {
      void clearPrepared().catch(() => undefined);
      preparedNextKeyRef.current = null;
      return;
    }
    const index = playableEntries.findIndex(
      (entry) => entry.id === selectedEntryId,
    );
    const candidates = index >= 0 ? playableEntries.slice(index + 1) : [];
    const key = `${selectedEntryId ?? ""}:${candidates.map((entry) => entry.id).join("\u0000")}`;
    if (preparedNextKeyRef.current === key) return;
    preparedNextKeyRef.current = key;
    void (async () => {
      for (const candidate of candidates) {
        try {
          await prepareNext(candidate.id);
          return;
        } catch {
          // Skip candidates that cannot be prepared; next sequential item may work.
        }
      }
    })();
  }, [
    gaplessPlayback,
    playbackMode,
    playbackStatus,
    playableEntries,
    selectedEntryId,
  ]);

  useEffect(() => {
    if (playbackStatus === "playing" || playbackStatus === "paused") {
      activeSessionRef.current = true;
    } else if (
      playbackStatus === "loading" ||
      playbackStatus === "idle" ||
      playbackStatus === "failed"
    ) {
      activeSessionRef.current = false;
    }
  }, [playbackGeneration, playbackStatus]);

  // Stop destroys the engine but preserves local A-B state. Reapply a complete
  // region once the selected file gets a fresh playback worker.
  useEffect(() => {
    if (
      playbackStatus !== "playing" ||
      !regionNeedsRestoreRef.current ||
      appliedRegionGeneration.current === playbackGeneration ||
      abEntryId !== selectedEntryId
    ) {
      return;
    }
    const region = loopRegionRef.current;
    if (!region) return;
    regionNeedsRestoreRef.current = false;
    appliedRegionGeneration.current = playbackGeneration;
    void setLoopRegion(region.startMs, region.endMs).catch((cause) => {
      appliedRegionGeneration.current = null;
      regionNeedsRestoreRef.current = true;
      setAbError(
        cause instanceof Error
          ? cause.message
          : "Could not restore the A-B region.",
      );
    });
  }, [abEntryId, playbackGeneration, playbackStatus, selectedEntryId]);

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

  const selectedIndex = playableEntries.findIndex(
    (entry) => entry.id === selectedEntryId,
  );
  const canPrevious = selectedIndex > 0;
  const canNext =
    selectedIndex >= 0 && selectedIndex < playableEntries.length - 1;
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
      const selected = playableEntries[selectedIndex];
      return selected ? onSelectEntry(selected) : Promise.resolve();
    },
    handleStop: async () => {
      activeSessionRef.current = false;
      const succeeded = await runCommand(async () => {
        await stop();
        activeSessionRef.current = false;
        regionNeedsRestoreRef.current = loopRegionRef.current !== null;
        setPositionMs(0);
        if (selectedEntryId) {
          setCommandStatus({
            entryId: selectedEntryId,
            status: "idle",
            generation: playbackGeneration,
          });
        }
      });
      if (!succeeded) {
        activeSessionRef.current =
          playbackStatus === "playing" || playbackStatus === "paused";
      }
    },
    handlePrevious: () => {
      if (canPrevious) {
        return onSelectEntry(playableEntries[selectedIndex - 1]);
      }
      return Promise.resolve();
    },
    handleNext: () => {
      if (canNext) return onSelectEntry(playableEntries[selectedIndex + 1]);
      return Promise.resolve();
    },
    handleSeek: async (nextPositionMs: number) => {
      if (!activeSessionRef.current) {
        setPositionMs(nextPositionMs);
        setPositionEntryId(selectedEntryId);
        return nextPositionMs;
      }
      let confirmedPosition: number | null = null;
      const succeeded = await runCommand(async () => {
        const actual = await seek(nextPositionMs);
        setPositionMs(actual);
        confirmedPosition = actual;
      });
      if (confirmedPosition !== null) {
        const region = loopRegionRef.current;
        if (
          region &&
          (confirmedPosition < region.startMs ||
            confirmedPosition >= region.endMs)
        ) {
          // The engine cleared the region during this out-of-region seek;
          // mirror the confirmed Rust state in the markers. The command is
          // best-effort because the engine already dropped the region.
          void clearLoopRegion().catch(() => undefined);
          resetABLocal();
        }
      }
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
    // A–B selection is per-file: points are only visible for the entry that
    // placed them. The engine gives each file a fresh worker, so any other
    // selection simply shows no region.
    abPoints:
      abEntryId === selectedEntryId ? abPoints : { startMs: null, endMs: null },
    loopRegion: abEntryId === selectedEntryId ? loopRegion : null,
    abError,
    setAbPoint,
    clearAB,
    toggleAbRepeat,
  };
}

function pickRandomEntry(
  entries: BrowserEntry[],
  currentEntryId: string | null,
  random: () => number,
): BrowserEntry | undefined {
  const alternatives = entries.filter((entry) => entry.id !== currentEntryId);
  const candidates = alternatives.length > 0 ? alternatives : entries;
  if (candidates.length === 0) return undefined;
  const index = Math.min(
    candidates.length - 1,
    Math.max(0, Math.floor(random() * candidates.length)),
  );
  return candidates[index];
}
