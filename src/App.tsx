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
import "./App.css";

const clamp = (value: number, minimum: number, maximum: number) =>
  Math.min(maximum, Math.max(minimum, value));

function WaveformOverview({ title }: { title: string }) {
  return (
    <section className="waveform-panel" aria-label="Waveform overview">
      <header className="now-playing">
        <div>
          <span className="now-playing-label">Play:</span>{" "}
          <strong>{title}</strong>
        </div>
        <div className="brand-mark" aria-label="PulseSeek">
          <span className="brand-wave" aria-hidden="true">
            ∿
          </span>
          pulseseek
        </div>
      </header>
      <div className="audio-summary">44.1 kHz, stereo · lossless audio</div>
      <div className="waveform-canvas" aria-hidden="true">
        <svg viewBox="0 0 1200 220" preserveAspectRatio="none">
          <defs>
            <path
              id="wave-shape"
              d="M0 110 L38 109 52 103 66 111 80 96 94 118 108 91 122 126 136 82 150 120 164 72 178 129 192 83 206 121 220 64 234 130 248 75 262 119 276 58 290 127 304 68 318 119 332 49 346 128 360 65 374 121 388 57 402 135 416 60 430 125 444 45 458 134 472 58 486 122 500 40 514 130 528 51 542 119 556 56 570 136 584 49 598 125 612 60 626 132 640 48 654 125 668 65 682 130 696 55 710 120 724 68 738 127 752 61 766 122 780 71 794 127 808 65 822 118 836 76 850 122 864 71 878 119 892 80 906 121 920 77 934 117 948 84 962 119 976 82 990 116 1004 87 1018 117 1032 89 1046 115 1060 92 1074 114 1088 95 1102 113 1116 98 1130 112 1144 101 1158 111 1172 105 1200 110"
            />
          </defs>
          <g className="wave-grid">
            <line x1="0" y1="55" x2="1200" y2="55" />
            <line x1="0" y1="110" x2="1200" y2="110" />
            <line x1="0" y1="165" x2="1200" y2="165" />
          </g>
          <use href="#wave-shape" className="wave-line" />
          <use
            href="#wave-shape"
            className="wave-line wave-line--mirrored"
            transform="translate(0 220) scale(1 -1)"
          />
          <rect
            className="wave-selection"
            x="595"
            y="0"
            width="58"
            height="220"
          />
          <line className="wave-playhead" x1="618" y1="0" x2="618" y2="220" />
        </svg>
      </div>
    </section>
  );
}

function App() {
  const [waveformSize, setWaveformSize] = useState(38);
  const [browserSize, setBrowserSize] = useState(24);
  const folderTree = useFolderTree();
  const { state } = folderTree;
  const playback = usePlaybackSelection();
  const playbackMode = usePlaybackMode();
  const audioDevices = useAudioDevices();

  const fileListEntries = state.playableEntries[state.selectedPath ?? ""] ?? [];
  const fileListFolder = state.folders[state.selectedPath ?? ""] ?? undefined;
  const transport = usePlaybackTransport({
    entries: fileListEntries,
    selectedEntryId: playback.playback.entryId,
    playbackStatus: playback.playback.status,
    playbackGeneration: playback.playback.generation,
    playbackMode: playbackMode.mode,
    onSelectEntry: playback.select,
  });

  const selectedEntry = fileListEntries.find(
    (entry) => entry.id === playback.playback.entryId,
  );

  const startResize = useCallback(
    (axis: "horizontal" | "vertical", update: (value: number) => void) =>
      (event: React.PointerEvent<HTMLDivElement>) => {
        event.preventDefault();
        const container = event.currentTarget.parentElement;
        if (!container) return;

        const handleMove = (moveEvent: PointerEvent) => {
          const bounds = container.getBoundingClientRect();
          const value =
            axis === "horizontal"
              ? ((moveEvent.clientY - bounds.top) / bounds.height) * 100
              : ((moveEvent.clientX - bounds.left) / bounds.width) * 100;
          update(
            clamp(
              Math.round(value),
              axis === "horizontal" ? 22 : 16,
              axis === "horizontal" ? 62 : 46,
            ),
          );
        };
        const handleUp = () => {
          document.removeEventListener("pointermove", handleMove);
          document.removeEventListener("pointerup", handleUp);
          document.body.classList.remove("is-resizing");
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
      transport.handleSeek(Math.max(0, transport.positionMs - 5_000)),
    onSeekForward: () =>
      transport.handleSeek(
        transport.durationMs === null
          ? transport.positionMs + 5_000
          : Math.min(transport.durationMs, transport.positionMs + 5_000),
      ),
    onToggleLoop: () =>
      playbackMode.selectMode(
        playbackMode.mode === "loop-current" ? "one-shot" : "loop-current",
      ),
  });

  return (
    <main className="app-shell">
      <h1 className="visually-hidden">PulseSeek</h1>
      <div
        className="app-layout"
        style={{ gridTemplateRows: `${waveformSize}% 7px 1fr` }}
      >
        <WaveformOverview title={selectedEntry?.name ?? "No file selected"} />
        <div
          className="splitter splitter--horizontal"
          role="separator"
          aria-label="Resize waveform"
          aria-orientation="horizontal"
          aria-valuemin={22}
          aria-valuemax={62}
          aria-valuenow={waveformSize}
          tabIndex={0}
          onPointerDown={startResize("horizontal", setWaveformSize)}
          onKeyDown={(event) => {
            if (event.key === "ArrowDown" || event.key === "ArrowUp") {
              event.preventDefault();
              setWaveformSize((size) =>
                clamp(size + (event.key === "ArrowDown" ? 2 : -2), 22, 62),
              );
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
              onStop={transport.handleStop}
              onPrevious={transport.handlePrevious}
              onNext={transport.handleNext}
              onSeek={transport.handleSeek}
              onVolume={transport.handleVolume}
              onToggleMute={transport.toggleMute}
            />
            <div className="transport-options">
              <PlaybackModeSelector
                mode={playbackMode.mode}
                error={playbackMode.error}
                onChange={playbackMode.selectMode}
              />
              <AudioDeviceSelector
                {...audioDevices}
                onChange={audioDevices.choose}
                onRetry={audioDevices.refresh}
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
              <FolderTree {...folderTree} />
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
              onPointerDown={startResize("vertical", setBrowserSize)}
              onKeyDown={(event) => {
                if (event.key === "ArrowRight" || event.key === "ArrowLeft") {
                  event.preventDefault();
                  setBrowserSize((size) =>
                    clamp(size + (event.key === "ArrowRight" ? 2 : -2), 16, 46),
                  );
                }
              }}
            />
            <section className="app-content">
              <FileList
                entries={fileListEntries}
                selectedPath={state.selectedPath}
                isLoading={fileListFolder?.isLoading ?? false}
                error={fileListFolder?.error ?? null}
                onFileSelect={playback.select}
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
import { useCallback, useState } from "react";
