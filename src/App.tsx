import { useCallback, useEffect, useRef, useState } from "react";
import { useFolderTree } from "./hooks/useFolderTree";
import { FolderTree } from "./components/FolderTree/FolderTree";
import { FileList } from "./components/FileList/FileList";
import { usePlaybackSelection } from "./hooks/usePlaybackSelection";
import { usePlaybackTransport } from "./hooks/usePlaybackTransport";
import { PlayerTransport } from "./components/PlayerTransport/PlayerTransport";
import { PlaybackModeSelector } from "./components/PlaybackModeSelector/PlaybackModeSelector";
import { usePlaybackMode } from "./hooks/usePlaybackMode";
import { useAudioDevices } from "./hooks/useAudioDevices";
import { AudioDeviceSelector } from "./components/AudioDeviceSelector/AudioDeviceSelector";
import { useKeyboardShortcuts } from "./hooks/useKeyboardShortcuts";
import { usePlayerPreferences } from "./hooks/usePlayerPreferences";
import { useTheme } from "./hooks/useTheme";
import { ThemeSelector } from "./components/ThemeSelector/ThemeSelector";
import { WaveformStyleSelector } from "./components/WaveformStyleSelector/WaveformStyleSelector";
import { WaveformPanel } from "./components/Waveform/WaveformPanel";
import "./styles/tokens.css";
import "./styles/themes/light.css";
import "./styles/themes/dark.css";
import "./styles/themes/midnight.css";
import "./styles/themes/high-contrast.css";
import "./App.css";

const clamp = (value: number, minimum: number, maximum: number) =>
  Math.min(maximum, Math.max(minimum, value));

function App() {
  const [waveformSize, setWaveformSize] = useState(38);
  const [browserSize, setBrowserSize] = useState(24);
  const [waveformResetRevision, setWaveformResetRevision] = useState(0);
  const folderTree = useFolderTree();
  const { state } = folderTree;
  const playback = usePlaybackSelection();
  const playbackMode = usePlaybackMode();
  const audioDevices = useAudioDevices();
  const playerPreferences = usePlayerPreferences();
  const updatePreferences = playerPreferences.update;
  useTheme(playerPreferences.preferences.theme);
  const restoredOptions = useRef(false);
  const restoredBrowser = useRef(false);
  const restoredDevice = useRef(false);
  const restoredFile = useRef(false);
  const restoredResume = useRef<{ entryId: string; positionMs: number } | null>(
    null,
  );

  const fileListEntries = state.playableEntries[state.selectedPath ?? ""] ?? [];
  const fileListFolder = state.folders[state.selectedPath ?? ""] ?? undefined;
  const selectAndRemember = useCallback(
    async (
      entry: import("./components/FolderTree/folderTreeTypes").BrowserEntry,
    ) => {
      const savedResume = restoredResume.current;
      const startPositionMs =
        savedResume?.entryId === entry.id ? savedResume.positionMs : 0;
      if (await playback.select(entry, startPositionMs)) {
        if (savedResume?.entryId === entry.id) restoredResume.current = null;
        updatePreferences({
          last_played_file_path: entry.id,
          last_played_position_ms: startPositionMs,
          last_played_duration_ms: entry.metadata?.duration_ms ?? null,
          selected_folder_path:
            entry.id.substring(0, entry.id.lastIndexOf("/")) || null,
        });
      }
    },
    [playback, updatePreferences],
  );
  const transport = usePlaybackTransport({
    entries: fileListEntries,
    selectedEntryId: playback.playback.entryId,
    playbackStatus: playback.playback.status,
    playbackGeneration: playback.playback.generation,
    playbackMode: playbackMode.mode,
    onSelectEntry: selectAndRemember,
  });

  useEffect(() => {
    if (!playerPreferences.isLoaded || restoredOptions.current) return;
    restoredOptions.current = true;
    const saved = playerPreferences.preferences;
    setWaveformSize(saved.waveform_size);
    setBrowserSize(saved.browser_size);
    void playbackMode.selectMode(saved.playback_mode);
    void transport.restoreVolume(saved.volume, saved.muted);
  }, [
    playbackMode,
    playerPreferences.isLoaded,
    playerPreferences.preferences,
    transport,
  ]);

  useEffect(() => {
    const saved = playerPreferences.preferences;
    if (
      !playerPreferences.isLoaded ||
      restoredBrowser.current ||
      state.status !== "ready"
    ) {
      return;
    }
    restoredBrowser.current = true;
    if (saved.selected_folder_path) {
      void folderTree
        .restoreContext(saved.selected_folder_path)
        .then((restoredPath) => {
          if (restoredPath !== saved.selected_folder_path) {
            playerPreferences.update({
              selected_folder_path:
                restoredPath === "computer://" ? null : restoredPath,
              expanded_folder_paths:
                restoredPath === "computer://"
                  ? ["computer://"]
                  : saved.expanded_folder_paths.filter((path) =>
                      restoredPath.startsWith(path),
                    ),
              last_played_file_path: null,
              last_played_position_ms: 0,
              last_played_duration_ms: null,
            });
          }
        });
    }
  }, [
    folderTree,
    playerPreferences,
    playerPreferences.isLoaded,
    playerPreferences.preferences,
    state.status,
  ]);

  useEffect(() => {
    const savedDevice = playerPreferences.preferences.output_device_id;
    if (
      !playerPreferences.isLoaded ||
      restoredDevice.current ||
      audioDevices.isLoading
    ) {
      return;
    }
    restoredDevice.current = true;
    if (
      savedDevice &&
      audioDevices.devices.some((device) => device.id === savedDevice)
    ) {
      void audioDevices.choose(savedDevice);
    }
  }, [
    audioDevices,
    playerPreferences.isLoaded,
    playerPreferences.preferences.output_device_id,
  ]);

  useEffect(() => {
    if (!playerPreferences.isLoaded || restoredFile.current) return;
    const lastPath = playerPreferences.preferences.last_played_file_path;
    if (!lastPath) {
      restoredFile.current = true;
      return;
    }
    const restoredEntry = Object.values(state.playableEntries)
      .flat()
      .find((entry) => entry.id === lastPath);
    if (restoredEntry) {
      restoredFile.current = true;
      const savedPosition =
        playerPreferences.preferences.last_played_position_ms;
      restoredResume.current = {
        entryId: restoredEntry.id,
        positionMs: savedPosition,
      };
      playback.restore(restoredEntry.id);
      transport.restorePosition(
        restoredEntry.id,
        savedPosition,
        restoredEntry.metadata?.duration_ms ??
          playerPreferences.preferences.last_played_duration_ms,
      );
    }
  }, [
    playback,
    playerPreferences.isLoaded,
    playerPreferences.preferences.last_played_file_path,
    playerPreferences.preferences.last_played_position_ms,
    playerPreferences.preferences.last_played_duration_ms,
    state.playableEntries,
    transport,
  ]);

  const selectedEntry = fileListEntries.find(
    (entry) => entry.id === playback.playback.entryId,
  );

  const seekAndRemember = useCallback(
    async (positionMs: number) => {
      const confirmed = await transport.handleSeek(positionMs);
      if (confirmed !== null && playback.playback.entryId) {
        restoredResume.current = null;
        updatePreferences({
          last_played_file_path: playback.playback.entryId,
          last_played_position_ms: confirmed,
          last_played_duration_ms: transport.durationMs,
        });
      }
    },
    [playback.playback.entryId, transport, updatePreferences],
  );

  const playbackSnapshot = useRef({
    entryId: playback.playback.entryId,
    positionMs: transport.positionMs,
    status: transport.status,
  });
  useEffect(() => {
    playbackSnapshot.current = {
      entryId: playback.playback.entryId,
      positionMs: transport.positionMs,
      status: transport.status,
    };
  }, [playback.playback.entryId, transport.positionMs, transport.status]);
  const lastPeriodicPosition = useRef(0);
  useEffect(() => {
    const timer = window.setInterval(() => {
      const snapshot = playbackSnapshot.current;
      if (
        snapshot.status !== "playing" ||
        !snapshot.entryId ||
        Math.abs(snapshot.positionMs - lastPeriodicPosition.current) < 1_000
      ) {
        return;
      }
      lastPeriodicPosition.current = snapshot.positionMs;
      updatePreferences({
        last_played_file_path: snapshot.entryId,
        last_played_position_ms: snapshot.positionMs,
        last_played_duration_ms: transport.durationMs,
      });
    }, 1_000);
    return () => window.clearInterval(timer);
  }, [transport.durationMs, updatePreferences]);

  const startResize = useCallback(
    (
      axis: "horizontal" | "vertical",
      update: (value: number) => void,
      commit: (value: number) => void,
    ) =>
      (event: React.PointerEvent<HTMLDivElement>) => {
        event.preventDefault();
        const container = event.currentTarget.parentElement;
        if (!container) return;

        let latestValue: number | null = null;
        const handleMove = (moveEvent: PointerEvent) => {
          const bounds = container.getBoundingClientRect();
          const value =
            axis === "horizontal"
              ? ((moveEvent.clientY - bounds.top) / bounds.height) * 100
              : ((moveEvent.clientX - bounds.left) / bounds.width) * 100;
          latestValue = clamp(
            Math.round(value),
            axis === "horizontal" ? 22 : 16,
            axis === "horizontal" ? 62 : 46,
          );
          update(latestValue);
        };
        const handleUp = () => {
          document.removeEventListener("pointermove", handleMove);
          document.removeEventListener("pointerup", handleUp);
          document.body.classList.remove("is-resizing");
          if (latestValue !== null) commit(latestValue);
        };
        document.body.classList.add("is-resizing");
        document.addEventListener("pointermove", handleMove);
        document.addEventListener("pointerup", handleUp);
      },
    [],
  );

  useKeyboardShortcuts({
    onTogglePlayPause: transport.togglePlayPause,
    onPreviousTrack: transport.handlePrevious,
    onNextTrack: transport.handleNext,
    onSeekBackward: () =>
      seekAndRemember(Math.max(0, transport.positionMs - 5_000)),
    onSeekForward: () =>
      seekAndRemember(
        transport.durationMs === null
          ? transport.positionMs + 5_000
          : Math.min(transport.durationMs, transport.positionMs + 5_000),
      ),
    onToggleLoop: () => {
      const mode =
        playbackMode.mode === "loop-current" ? "one-shot" : "loop-current";
      return playbackMode.selectMode(mode).then((confirmed) => {
        if (confirmed) playerPreferences.update({ playback_mode: confirmed });
      });
    },
  });

  return (
    <main className="app-shell">
      <h1 className="visually-hidden">PulseSeek</h1>
      <div
        className="app-layout"
        style={{ gridTemplateRows: `${waveformSize}% 7px 1fr` }}
      >
        <WaveformPanel
          entryPath={selectedEntry?.id ?? null}
          entryName={selectedEntry?.name ?? "No file selected"}
          durationMs={
            transport.durationMs ??
            selectedEntry?.metadata?.duration_ms ??
            (selectedEntry?.id ===
            playerPreferences.preferences.last_played_file_path
              ? playerPreferences.preferences.last_played_duration_ms
              : null)
          }
          restoredPositionMs={transport.positionMs}
          resetRevision={waveformResetRevision}
          onSeek={seekAndRemember}
          style={playerPreferences.preferences.waveform_style}
        />
        <div
          className="splitter splitter--horizontal"
          role="separator"
          aria-label="Resize waveform"
          aria-orientation="horizontal"
          aria-valuemin={22}
          aria-valuemax={62}
          aria-valuenow={waveformSize}
          tabIndex={0}
          onPointerDown={startResize("horizontal", setWaveformSize, (size) =>
            playerPreferences.update({ waveform_size: size }),
          )}
          onKeyDown={(event) => {
            if (event.key === "ArrowDown" || event.key === "ArrowUp") {
              event.preventDefault();
              const next = clamp(
                waveformSize + (event.key === "ArrowDown" ? 2 : -2),
                22,
                62,
              );
              setWaveformSize(next);
              playerPreferences.update({ waveform_size: next });
            }
          }}
        />
        <section className="lower-workspace">
          <div
            className="transport-strip"
            role="toolbar"
            aria-label="Playback controls"
          >
            <PlayerTransport
              status={transport.status}
              hasSelection={transport.hasSelection}
              positionMs={transport.positionMs}
              durationMs={transport.durationMs}
              volume={transport.volume}
              muted={transport.muted}
              canPrevious={transport.canPrevious}
              canNext={transport.canNext}
              error={transport.error ?? playback.playback.error}
              onTogglePlayPause={transport.togglePlayPause}
              onStop={async () => {
                await transport.handleStop();
                setWaveformResetRevision((revision) => revision + 1);
                restoredResume.current = null;
                if (playback.playback.entryId) {
                  playerPreferences.update({
                    last_played_file_path: playback.playback.entryId,
                    last_played_position_ms: 0,
                    last_played_duration_ms:
                      selectedEntry?.metadata?.duration_ms ??
                      transport.durationMs,
                  });
                }
              }}
              onPrevious={transport.handlePrevious}
              onNext={transport.handleNext}
              onVolume={async (volume) => {
                if (await transport.handleVolume(volume)) {
                  playerPreferences.update({ volume });
                }
              }}
              onToggleMute={async () => {
                const muted = !transport.muted;
                if (await transport.toggleMute()) {
                  playerPreferences.update({ muted });
                }
              }}
            />
            <div className="transport-options">
              <PlaybackModeSelector
                mode={playbackMode.mode}
                error={playbackMode.error}
                onChange={async (mode) => {
                  const confirmed = await playbackMode.selectMode(mode);
                  if (confirmed) {
                    playerPreferences.update({ playback_mode: confirmed });
                  }
                }}
              />
              <AudioDeviceSelector
                {...audioDevices}
                onChange={async (deviceId) => {
                  const confirmed = await audioDevices.choose(deviceId);
                  if (confirmed) {
                    playerPreferences.update({ output_device_id: confirmed });
                  }
                }}
                onRetry={audioDevices.refresh}
              />
              <ThemeSelector
                theme={playerPreferences.preferences.theme}
                onChange={(theme) => {
                  playerPreferences.update({ theme });
                }}
              />
              <WaveformStyleSelector
                style={playerPreferences.preferences.waveform_style}
                onChange={(waveform_style) => {
                  playerPreferences.update({ waveform_style });
                }}
              />
            </div>
          </div>
          <div
            className="browser-tabs"
            role="tablist"
            aria-label="Workspace panels"
          >
            <button type="button" role="tab" aria-selected="true">
              Browser
            </button>
            <button type="button" role="tab" aria-selected="true">
              File list
            </button>
          </div>
          <div
            className="browser-workspace"
            style={{ gridTemplateColumns: `${browserSize}% 7px 1fr` }}
          >
            <aside className="app-sidebar">
              <FolderTree
                {...folderTree}
                toggleExpand={(path) => {
                  const expanded = new Set(
                    Object.entries(state.folders)
                      .filter(([, folder]) => folder.expanded)
                      .map(([folderPath]) => folderPath),
                  );
                  if (state.folders[path]?.expanded) expanded.delete(path);
                  else expanded.add(path);
                  playerPreferences.update({
                    expanded_folder_paths: [...expanded],
                  });
                  folderTree.toggleExpand(path);
                }}
                selectFolder={(path) => {
                  playerPreferences.update({ selected_folder_path: path });
                  folderTree.selectFolder(path);
                }}
                navigateUp={() => {
                  const selected = state.selectedPath;
                  const trimmed = selected?.replace(/\/+$/, "") ?? "";
                  const separator = trimmed.lastIndexOf("/");
                  if (separator > 0) {
                    playerPreferences.update({
                      selected_folder_path: trimmed.substring(0, separator),
                    });
                  }
                  folderTree.navigateUp();
                }}
              />
            </aside>
            <div
              className="splitter splitter--vertical"
              role="separator"
              aria-label="Resize browser"
              aria-orientation="vertical"
              aria-valuemin={16}
              aria-valuemax={46}
              aria-valuenow={browserSize}
              tabIndex={0}
              onPointerDown={startResize("vertical", setBrowserSize, (size) =>
                playerPreferences.update({ browser_size: size }),
              )}
              onKeyDown={(event) => {
                if (event.key === "ArrowRight" || event.key === "ArrowLeft") {
                  event.preventDefault();
                  const next = clamp(
                    browserSize + (event.key === "ArrowRight" ? 2 : -2),
                    16,
                    46,
                  );
                  setBrowserSize(next);
                  playerPreferences.update({ browser_size: next });
                }
              }}
            />
            <section className="app-content">
              <FileList
                entries={fileListEntries}
                selectedPath={state.selectedPath}
                isLoading={fileListFolder?.isLoading ?? false}
                error={fileListFolder?.error ?? null}
                onFileSelect={selectAndRemember}
                playbackEntryId={playback.playback.entryId}
                playbackStatus={playback.playback.status}
                playbackError={playback.playback.error}
                onEntriesTrashed={(entryIds) => {
                  if (state.selectedPath) {
                    folderTree.removeEntries(state.selectedPath, entryIds);
                  }
                }}
              />
            </section>
          </div>
        </section>
      </div>
    </main>
  );
}

export default App;
