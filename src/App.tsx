import { useFolderTree } from "./hooks/useFolderTree";
import { FolderTree } from "./components/FolderTree/FolderTree";
import { FileList } from "./components/FileList/FileList";
import { usePlaybackSelection } from "./hooks/usePlaybackSelection";
import { usePlaybackTransport } from "./hooks/usePlaybackTransport";
import { PlayerTransport } from "./components/PlayerTransport/PlayerTransport";
import { PlaybackModeSelector } from "./components/PlaybackModeSelector/PlaybackModeSelector";
import { usePlaybackMode } from "./hooks/usePlaybackMode";
import "./App.css";

function App() {
  const folderTree = useFolderTree();
  const { state } = folderTree;
  const playback = usePlaybackSelection();
  const playbackMode = usePlaybackMode();

  const fileListEntries = state.playableEntries[state.selectedPath ?? ""] ?? [];
  const fileListFolder = state.folders[state.selectedPath ?? ""] ?? undefined;
  const transport = usePlaybackTransport({
    entries: fileListEntries,
    selectedEntryId: playback.playback.entryId,
    playbackStatus: playback.playback.status,
    onSelectEntry: playback.select,
  });

  return (
    <main>
      <h1 className="visually-hidden">PulseSeek</h1>
      <div className="app-layout">
        <aside className="app-sidebar">
          <FolderTree {...folderTree} />
        </aside>
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
          />
          <PlayerTransport
            status={transport.status}
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
          <PlaybackModeSelector
            mode={playbackMode.mode}
            disabled={playbackMode.isChanging}
            error={playbackMode.error}
            onChange={playbackMode.selectMode}
          />
        </section>
      </div>
    </main>
  );
}

export default App;
