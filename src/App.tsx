import { useFolderTree } from "./hooks/useFolderTree";
import { FolderTree } from "./components/FolderTree/FolderTree";
import { FileList } from "./components/FileList/FileList";
import { usePlaybackSelection } from "./hooks/usePlaybackSelection";
import "./App.css";

function App() {
  const folderTree = useFolderTree();
  const { state } = folderTree;
  const playback = usePlaybackSelection();

  const fileListEntries = state.playableEntries[state.selectedPath ?? ""] ?? [];
  const fileListFolder = state.folders[state.selectedPath ?? ""] ?? undefined;

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
        </section>
      </div>
    </main>
  );
}

export default App;
