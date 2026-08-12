import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useFolderTree } from "./hooks/useFolderTree";
import { FolderTree } from "./components/FolderTree/FolderTree";
import { RecentFolders } from "./components/RecentFolders/RecentFolders";
import { useRecentFolders } from "./hooks/useRecentFolders";
import { useFolderBookmarks } from "./hooks/useFolderBookmarks";
import { Bookmarks } from "./components/Bookmarks/Bookmarks";
import {
  collectFolderEntries,
  getParentPath,
} from "./components/FolderTree/folderTreeTypes";
import { FileList } from "./components/FileList/FileList";
import {
  DEFAULT_FILE_SORT,
  sortFileEntries,
  type FileSort,
} from "./components/FileList/fileSort";
import { filterFileEntries } from "./components/FileList/fileSearch";
import {
  filterByFormat,
  type AudioFileFormat,
} from "./components/FileList/fileFilter";
import {
  filterByMark,
  type MarkFilter,
} from "./components/FileList/sessionMarks";
import { usePlaybackSelection } from "./hooks/usePlaybackSelection";
import { useMarkReconciliation } from "./hooks/useMarkReconciliation";
import { usePlaybackTransport } from "./hooks/usePlaybackTransport";
import { useSessionMarks } from "./hooks/useSessionMarks";
import { PlayerTransport } from "./components/PlayerTransport/PlayerTransport";
import { PlaybackModeSelector } from "./components/PlaybackModeSelector/PlaybackModeSelector";
import { usePlaybackMode } from "./hooks/usePlaybackMode";
import { useAudioDevices } from "./hooks/useAudioDevices";
import { AudioDeviceSelector } from "./components/AudioDeviceSelector/AudioDeviceSelector";
import { useKeyboardShortcuts } from "./hooks/useKeyboardShortcuts";
import { useShortcutMappings } from "./hooks/useShortcutMappings";
import { ShortcutEditor } from "./components/ShortcutEditor/ShortcutEditor";
import { getShortcutPlatform } from "./shortcuts/keyboardShortcuts";
import { pickFolder, type PlaybackMode } from "./api/commandEnvelope";
import { usePlayerPreferences } from "./hooks/usePlayerPreferences";
import { useTheme } from "./hooks/useTheme";
import { ThemeSelector } from "./components/ThemeSelector/ThemeSelector";
import { WaveformStyleSelector } from "./components/WaveformStyleSelector/WaveformStyleSelector";
import { WaveformPanel } from "./components/Waveform/WaveformPanel";
import {
  VisualizationSelector,
  VisualizationSettingsControls,
} from "./components/VisualizationSelector/VisualizationSelector";
import { useVisualizationSettings } from "./hooks/useVisualizationSettings";
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
  const [fileSort, setFileSort] = useState<FileSort>(DEFAULT_FILE_SORT);
  const [searchQuery, setSearchQuery] = useState("");
  const [formatFilter, setFormatFilter] = useState<AudioFileFormat[]>([]);
  const [markFilter, setMarkFilter] = useState<MarkFilter>("all");
  const [sidebarView, setSidebarView] = useState<
    "browser" | "bookmarks" | "recent"
  >("browser");
  const [shortcutEditorOpen, setShortcutEditorOpen] = useState(false);
  const [focusSearchRevision, setFocusSearchRevision] = useState(0);
  const [folderPickerError, setFolderPickerError] = useState<string | null>(
    null,
  );
  const sessionMarks = useSessionMarks();
  const playerPreferences = usePlayerPreferences();
  const visualizationSettings = useVisualizationSettings();
  const folderTree = useFolderTree(
    playerPreferences.preferences.show_hidden_folders,
  );
  const recentFolders = useRecentFolders();
  const folderBookmarks = useFolderBookmarks();
  const { state } = folderTree;
  const playback = usePlaybackSelection();
  useMarkReconciliation(
    state.selectedPath,
    state.status,
    state.playableEntries,
    sessionMarks.marks,
    sessionMarks.replace,
  );
  const playbackMode = usePlaybackMode();
  const audioDevices = useAudioDevices();
  const shortcutMappings = useShortcutMappings();
  const updatePreferences = playerPreferences.update;
  const resolvedTheme = useTheme(playerPreferences.preferences.theme);
  const restoredOptions = useRef(false);
  const restoredBrowser = useRef(false);
  const restoredDevice = useRef(false);
  const restoredFile = useRef(false);
  const restoredResume = useRef<{ entryId: string; positionMs: number } | null>(
    null,
  );
  const browserTabRef = useRef<HTMLButtonElement | null>(null);
  const recentTabRef = useRef<HTMLButtonElement | null>(null);
  const bookmarksTabRef = useRef<HTMLButtonElement | null>(null);

  const handleSidebarTabKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLButtonElement>) => {
      const tabs = ["browser", "bookmarks", "recent"] as const;
      const current = tabs.indexOf(sidebarView);
      let next: (typeof tabs)[number] | null = null;
      if (event.key === "Home") next = "browser";
      if (event.key === "End") next = "recent";
      if (event.key === "ArrowLeft")
        next = tabs[(current + tabs.length - 1) % tabs.length];
      if (event.key === "ArrowRight") next = tabs[(current + 1) % tabs.length];
      if (!next) return;
      event.preventDefault();
      setSidebarView(next);
      ({
        browser: browserTabRef,
        bookmarks: bookmarksTabRef,
        recent: recentTabRef,
      })[next].current?.focus();
    },
    [sidebarView],
  );

  const fileListFolder = state.folders[state.selectedPath ?? ""] ?? undefined;
  const recursiveView = fileListFolder?.recursive ?? false;
  const fileListEntries = useMemo(
    () => state.playableEntries[state.selectedPath ?? ""] ?? [],
    [state.playableEntries, state.selectedPath],
  );
  // When a search is active, matched folders from the whole explored tree are
  // shown alongside the current folder's playable files so the user can find
  // any folder (e.g. Downloads) and navigate into it (FR-LS-004).
  const folderEntries = useMemo(
    () => collectFolderEntries(state.folders),
    [state.folders],
  );
  const searchBaseEntries = useMemo(
    () =>
      searchQuery.trim() === ""
        ? fileListEntries
        : [...folderEntries, ...fileListEntries],
    [searchQuery, folderEntries, fileListEntries],
  );
  const filteredFileListEntries = useMemo(
    () => filterFileEntries(searchBaseEntries, searchQuery),
    [searchBaseEntries, searchQuery],
  );
  const formatFilteredFileListEntries = useMemo(
    () => filterByFormat(filteredFileListEntries, formatFilter),
    [filteredFileListEntries, formatFilter],
  );
  const markFilteredFileListEntries = useMemo(
    () =>
      filterByMark(
        formatFilteredFileListEntries,
        sessionMarks.marks,
        markFilter,
      ),
    [formatFilteredFileListEntries, sessionMarks.marks, markFilter],
  );
  const sortedFileListEntries = useMemo(
    () => sortFileEntries(markFilteredFileListEntries, fileSort),
    [markFilteredFileListEntries, fileSort],
  );
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
    entries: sortedFileListEntries,
    selectedEntryId: playback.playback.entryId,
    playbackStatus: playback.playback.status,
    playbackGeneration: playback.playback.generation,
    playbackMode: playbackMode.mode,
    onSelectEntry: selectAndRemember,
  });

  // Opens a folder: records it in the recent-folder history (FR-BR-011),
  // persists it as the selected folder, and selects it in the tree. When
  // `expand` is set the folder is also enumerated so its contents appear.
  const openFolder = useCallback(
    (path: string, options?: { expand?: boolean }) => {
      playerPreferences.update({ selected_folder_path: path });
      recentFolders.record(path);
      folderTree.selectFolder(path);
      if (options?.expand) folderTree.toggleExpand(path);
    },
    [folderTree, playerPreferences, recentFolders],
  );

  const reopenFolder = useCallback(
    (path: string) => {
      playerPreferences.update({ selected_folder_path: path });
      recentFolders.record(path);
      void folderTree.restoreContext(path);
    },
    [folderTree, playerPreferences, recentFolders],
  );

  const openPickedFolder = useCallback(async () => {
    setFolderPickerError(null);
    try {
      const path = await pickFolder();
      if (path) openFolder(path, { expand: true });
    } catch {
      setFolderPickerError("Unable to open folder.");
    }
  }, [openFolder]);

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
          if (restoredPath !== "computer://") {
            recentFolders.record(restoredPath);
          }
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
    recentFolders,
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

  const selectPlaybackMode = useCallback(
    async (mode: PlaybackMode) => {
      const confirmed = await playbackMode.selectMode(mode);
      if (confirmed) playerPreferences.update({ playback_mode: confirmed });
    },
    [playbackMode, playerPreferences],
  );

  useKeyboardShortcuts(
    {
      onOpenFolder: openPickedFolder,
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
      onRefresh: folderTree.refreshSelected,
      onFocusSearch: () => setFocusSearchRevision((revision) => revision + 1),
      onSetPlaybackModeOneShot: () => selectPlaybackMode("one-shot"),
      onSetPlaybackModeLoopCurrent: () => selectPlaybackMode("loop-current"),
      onSetPlaybackModeSequential: () => selectPlaybackMode("sequential"),
      onSetPlaybackModeRandom: () => selectPlaybackMode("random"),
      onSetAbStart: () => {
        void transport.setAbPoint("a", transport.positionMs);
      },
      onSetAbEnd: () => {
        void transport.setAbPoint("b", transport.positionMs);
      },
      onToggleAbRepeat: () => {
        void transport.toggleAbRepeat();
      },
    },
    shortcutMappings.bindings,
  );

  return (
    <main
      className="app-shell"
      onContextMenu={(event) => event.preventDefault()}
    >
      <h1 className="visually-hidden">PulseSeek</h1>
      <div
        className="app-layout"
        style={{ gridTemplateRows: `${waveformSize}% 7px 1fr` }}
      >
        <WaveformPanel
          entryPath={selectedEntry?.id ?? null}
          entryName={selectedEntry?.name ?? "No file selected"}
          metadata={selectedEntry?.metadata}
          durationMs={
            transport.durationMs ??
            selectedEntry?.metadata?.duration_ms ??
            (selectedEntry?.id ===
            playerPreferences.preferences.last_played_file_path
              ? playerPreferences.preferences.last_played_duration_ms
              : null)
          }
          restoredPositionMs={transport.positionMs}
          playheadPositionMs={transport.positionMs}
          resetRevision={waveformResetRevision}
          onSeek={seekAndRemember}
          style={playerPreferences.preferences.waveform_style}
          theme={resolvedTheme}
          visualization={visualizationSettings.effectiveMode}
          abPoints={transport.abPoints}
          loopRegion={transport.loopRegion}
          abError={transport.abError}
          onSetAbPoint={(point, positionMs) =>
            transport.setAbPoint(point, positionMs)
          }
          onClearAB={() => transport.clearAB()}
        />
        <div
          className="splitter splitter--horizontal"
          role="separator"
          aria-label="Resize visualization"
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
              <div className="transport-settings">
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
                <VisualizationSelector
                  value={visualizationSettings.settings.mode}
                  onChange={(mode) => visualizationSettings.update({ mode })}
                />
                {visualizationSettings.effectiveMode === "waveform" && (
                  <WaveformStyleSelector
                    style={playerPreferences.preferences.waveform_style}
                    onChange={(waveform_style) => {
                      playerPreferences.update({ waveform_style });
                    }}
                  />
                )}
                {visualizationSettings.error && (
                  <span role="alert">{visualizationSettings.error}</span>
                )}
                {shortcutMappings.isLoading && (
                  <span role="status">Loading shortcuts…</span>
                )}
                {shortcutMappings.error && (
                  <span role="alert">{shortcutMappings.error}</span>
                )}
                {folderPickerError && (
                  <span role="alert">{folderPickerError}</span>
                )}
              </div>
              <details className="app-options-menu">
                <summary aria-label="Open application menu">☰</summary>
                <div className="app-options-menu-content">
                  <AudioDeviceSelector
                    {...audioDevices}
                    onChange={async (deviceId) => {
                      const confirmed = await audioDevices.choose(deviceId);
                      if (confirmed) {
                        playerPreferences.update({
                          output_device_id: confirmed,
                        });
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
                  <VisualizationSettingsControls
                    enabled={visualizationSettings.settings.enabled}
                    quality={visualizationSettings.settings.quality}
                    reducedMotion={visualizationSettings.reducedMotion}
                    onEnabledChange={(enabled) =>
                      visualizationSettings.update({ enabled })
                    }
                    onQualityChange={(quality) =>
                      visualizationSettings.update({ quality })
                    }
                  />
                  <div className="browser-hidden-option">
                    <label htmlFor="show-hidden-folders">
                      Show hidden folders
                    </label>
                    <input
                      id="show-hidden-folders"
                      type="checkbox"
                      checked={
                        playerPreferences.preferences.show_hidden_folders
                      }
                      onChange={(event) => {
                        playerPreferences.update({
                          show_hidden_folders: event.currentTarget.checked,
                        });
                      }}
                    />
                  </div>
                  <button
                    type="button"
                    className="shortcut-settings-button"
                    onClick={(event) => {
                      event.currentTarget
                        .closest("details")
                        ?.removeAttribute("open");
                      setShortcutEditorOpen(true);
                    }}
                  >
                    Keyboard shortcuts
                  </button>
                </div>
              </details>
            </div>
          </div>
          <div
            className="browser-workspace"
            style={{ gridTemplateColumns: `${browserSize}% 7px 1fr` }}
          >
            <aside className="app-sidebar">
              <div
                className="sidebar-tabs"
                role="tablist"
                aria-label="Browser views"
              >
                <button
                  ref={browserTabRef}
                  id="sidebar-tab-browser"
                  type="button"
                  role="tab"
                  aria-controls="sidebar-panel-browser"
                  aria-selected={sidebarView === "browser"}
                  tabIndex={sidebarView === "browser" ? 0 : -1}
                  onClick={() => setSidebarView("browser")}
                  onKeyDown={handleSidebarTabKeyDown}
                >
                  Browser
                </button>
                <button
                  ref={bookmarksTabRef}
                  id="sidebar-tab-bookmarks"
                  type="button"
                  role="tab"
                  aria-controls="sidebar-panel-bookmarks"
                  aria-selected={sidebarView === "bookmarks"}
                  tabIndex={sidebarView === "bookmarks" ? 0 : -1}
                  onClick={() => setSidebarView("bookmarks")}
                  onKeyDown={handleSidebarTabKeyDown}
                >
                  Bookmarks
                </button>
                <button
                  ref={recentTabRef}
                  id="sidebar-tab-recent"
                  type="button"
                  role="tab"
                  aria-controls="sidebar-panel-recent"
                  aria-selected={sidebarView === "recent"}
                  tabIndex={sidebarView === "recent" ? 0 : -1}
                  onClick={() => setSidebarView("recent")}
                  onKeyDown={handleSidebarTabKeyDown}
                >
                  Recent folders
                </button>
              </div>
              <div
                id="sidebar-panel-browser"
                className="sidebar-panel"
                role="tabpanel"
                aria-labelledby="sidebar-tab-browser"
                hidden={sidebarView !== "browser"}
              >
                <FolderTree
                  {...folderTree}
                  activeFilePath={playback.playback.entryId}
                  isBookmarked={folderBookmarks.isBookmarked(
                    state.selectedPath,
                  )}
                  isPathBookmarked={folderBookmarks.isBookmarked}
                  toggleBookmark={(path) => {
                    void folderBookmarks.toggle(path);
                  }}
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
                    openFolder(path);
                  }}
                  navigateUp={() => {
                    const parent = state.selectedPath
                      ? getParentPath(state.selectedPath)
                      : null;
                    if (parent) {
                      const expanded = Object.entries(state.folders)
                        .filter(
                          ([path, folder]) =>
                            folder.expanded && path !== parent,
                        )
                        .map(([path]) => path);
                      playerPreferences.update({
                        selected_folder_path: parent,
                        expanded_folder_paths: expanded,
                      });
                      recentFolders.record(parent);
                    }
                    folderTree.navigateUp();
                  }}
                />
              </div>
              <div
                id="sidebar-panel-bookmarks"
                className="sidebar-panel"
                role="tabpanel"
                aria-labelledby="sidebar-tab-bookmarks"
                hidden={sidebarView !== "bookmarks"}
              >
                <Bookmarks
                  bookmarks={folderBookmarks.bookmarks}
                  isLoading={folderBookmarks.isLoading}
                  error={folderBookmarks.error}
                  onReopen={(path) => {
                    setSidebarView("browser");
                    reopenFolder(path);
                  }}
                  onRemove={(path) => {
                    void folderBookmarks.toggle(path);
                  }}
                />
              </div>
              <div
                id="sidebar-panel-recent"
                className="sidebar-panel"
                role="tabpanel"
                aria-labelledby="sidebar-tab-recent"
                hidden={sidebarView !== "recent"}
              >
                <RecentFolders
                  folders={recentFolders.folders}
                  isLoading={recentFolders.isLoading}
                  error={recentFolders.error}
                  onReopen={(path) => {
                    setSidebarView("browser");
                    reopenFolder(path);
                  }}
                  onClear={() => {
                    void recentFolders.clear();
                  }}
                />
              </div>
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
              <div className="file-list-heading">File list</div>
              <FileList
                entries={sortedFileListEntries}
                selectedPath={state.selectedPath}
                isLoading={fileListFolder?.isLoading ?? false}
                error={fileListFolder?.error ?? null}
                onFileSelect={selectAndRemember}
                playbackEntryId={playback.playback.entryId}
                playbackStatus={playback.playback.status}
                playbackError={playback.playback.error}
                sort={fileSort}
                onSortChange={setFileSort}
                searchQuery={searchQuery}
                onSearchQueryChange={setSearchQuery}
                formatFilter={formatFilter}
                onFormatFilterChange={setFormatFilter}
                marks={sessionMarks.marks}
                onMarkChange={(ids, mark) => {
                  if (mark === null) sessionMarks.unmark(ids);
                  else sessionMarks.setMark(ids, mark);
                }}
                markFilter={markFilter}
                onMarkFilterChange={setMarkFilter}
                onSelectFolder={(path) => {
                  openFolder(path, { expand: true });
                }}
                isFolderBookmarked={folderBookmarks.isBookmarked}
                onToggleFolderBookmark={(path) => {
                  void folderBookmarks.toggle(path);
                }}
                onEntriesTrashed={(entryIds) => {
                  if (state.selectedPath) {
                    folderTree.removeEntries(state.selectedPath, entryIds);
                  }
                }}
                onEntryRenamed={(oldId, newId, newName) => {
                  if (state.selectedPath) {
                    folderTree.renameEntry(
                      state.selectedPath,
                      oldId,
                      newId,
                      newName,
                    );
                  }
                  playback.reconcile(oldId, newId);
                  sessionMarks.reconcile(oldId, newId);
                  if (
                    playerPreferences.preferences.last_played_file_path ===
                    oldId
                  ) {
                    playerPreferences.update({ last_played_file_path: newId });
                  }
                }}
                onEntriesMoved={(moved) => {
                  if (state.selectedPath) {
                    folderTree.removeEntries(
                      state.selectedPath,
                      moved.map((entry) => entry.oldId),
                    );
                  }
                  for (const entry of moved) {
                    playback.reconcile(entry.oldId, entry.newId);
                    sessionMarks.reconcile(entry.oldId, entry.newId);
                    if (
                      playerPreferences.preferences.last_played_file_path ===
                      entry.oldId
                    ) {
                      playerPreferences.update({
                        last_played_file_path: entry.newId,
                      });
                    }
                  }
                }}
                recursive={recursiveView}
                onRecursiveChange={(next) => {
                  if (state.selectedPath) {
                    folderTree.setRecursive(state.selectedPath, next);
                  }
                }}
                shortcutBindings={shortcutMappings.bindings}
                focusSearchRevision={focusSearchRevision}
              />
            </section>
          </div>
        </section>
      </div>
      <ShortcutEditor
        open={shortcutEditorOpen}
        bindings={shortcutMappings.bindings}
        platform={getShortcutPlatform()}
        onCancel={() => setShortcutEditorOpen(false)}
        onSave={async (bindings) => {
          const confirmed = await shortcutMappings.save(bindings);
          if (!confirmed) throw new Error("Could not save keyboard shortcuts.");
          setShortcutEditorOpen(false);
        }}
        onReset={async () => {
          const confirmed = await shortcutMappings.reset();
          if (!confirmed)
            throw new Error("Could not reset keyboard shortcuts.");
          return confirmed;
        }}
      />
    </main>
  );
}

export default App;
