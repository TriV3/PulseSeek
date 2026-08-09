pub mod audio_device_service;
pub mod command_envelope;
mod command_handlers;
pub mod copy_service;
pub mod diagnostics;
pub mod dialog_service;
pub mod drag_out_service;
pub mod external_service;
pub mod file_watcher_service;
pub mod folder_enumeration_service;
pub mod move_service;
pub mod native_audio_device_service;
pub mod native_playback_service;
pub mod path_validation;
pub mod playback_events;
pub mod playback_service;
pub mod player_preferences;
pub mod recent_folders_service;
pub mod rename_service;
pub mod shortcut_mappings_service;
pub mod trash_service;
mod visualization_service;
pub mod waveform_service;

use std::sync::{Arc, Mutex};

use diagnostics::DiagnosticsConfig;
use pulseseek_audio_cpal::CpalAudioOutput;
use pulseseek_cache::technical_cache::TechnicalCachePort;
use pulseseek_cache::waveform_cache::WaveformCachePort;
use tauri::Manager;

use crate::player_preferences::{
    JsonPlayerPreferencesRepository, SharedPlayerPreferencesRepository,
};
use crate::rename_service::{NativeRenameService, RenameService};
use crate::shortcut_mappings_service::{
    InMemoryShortcutMappingsService, NativeShortcutMappingsService, ShortcutMappingsService,
};
use crate::trash_service::{NativeTrashService, TrashService};
use crate::waveform_service::{NativeWaveformService, WaveformService};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _diagnostics_guard = diagnostics::init(DiagnosticsConfig::default());

    // Shared native audio output. Both the device service and playback service
    // operate on the same underlying cpal output instance.
    let audio_output: Arc<Mutex<CpalAudioOutput>> = Arc::new(Mutex::new(CpalAudioOutput::new()));

    // Native playback service that owns decoder workers and drives the
    // shared cpal output. Events are wired during setup. The value is managed
    // behind an `Arc` so long-lived workers (such as the move service) can
    // reconcile the tracked path without owning the playback engine.
    let playback_service: std::sync::Arc<
        std::sync::Mutex<Box<dyn playback_service::PlaybackService>>,
    > = std::sync::Arc::new(std::sync::Mutex::new(Box::new(
        native_playback_service::NativePlaybackService::new(Arc::clone(&audio_output)),
    )));

    // Native audio-device service. It falls back to the operating-system
    // default when a requested device is unavailable.
    let audio_device_service: std::sync::Mutex<Box<dyn audio_device_service::AudioDeviceService>> =
        std::sync::Mutex::new(Box::new(native_audio_device_service::NativeAudioDeviceService::new(
            audio_output,
        )));

    // Native folder enumeration service.
    let native_enum_service = folder_enumeration_service::NativeFolderEnumerationService::new();
    let enum_service: std::sync::Mutex<
        Box<dyn folder_enumeration_service::FolderEnumerationService>,
    > = std::sync::Mutex::new(Box::new(native_enum_service));

    let active_enumerations: folder_enumeration_service::ActiveEnumerations =
        folder_enumeration_service::ActiveEnumerations::new();

    // Native trash service backed by the operating system trash.
    let trash_service: std::sync::Mutex<Box<dyn TrashService>> = std::sync::Mutex::new(Box::new(
        NativeTrashService::new(pulseseek_browser_fs::trash::NativeFileTrash),
    ));

    // Native external-actions service backed by the browser-fs adapter. It
    // reveals or opens a single file through the operating system and never
    // exposes a general process-launch capability to the UI.
    let external_service: std::sync::Mutex<Box<dyn external_service::ExternalService>> =
        std::sync::Mutex::new(Box::new(external_service::NativeExternalService::new(
            pulseseek_browser_fs::external_actions::NativeFileActions,
        )));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(playback_service)
        .manage(audio_device_service)
        .manage(enum_service)
        .manage(active_enumerations)
        .manage(trash_service)
        .manage(external_service)
        .invoke_handler(tauri::generate_handler![
            diagnostics::report_error,
            command_envelope::invoke_command,
            command_envelope::pick_folder_dialog,
            player_preferences::load_player_preferences,
            player_preferences::save_player_preferences,
            playback_events::subscribe_spectrum_events,
            playback_events::unsubscribe_spectrum_events,
            playback_events::acknowledge_spectrum_frame,
            playback_events::subscribe_musical_spectrum_events,
            playback_events::unsubscribe_musical_spectrum_events,
            playback_events::acknowledge_musical_spectrum_frame,
            command_handlers::waveform::get_waveform
        ])
        .setup(|app| {
            let event_emitter: Arc<dyn playback_events::PlaybackEventEmitter> =
                Arc::new(playback_events::TauriEventEmitter::new(app.handle().clone()));

            // Native drag-out needs the runtime handle so AppKit startup can
            // be deferred until WKWebView has finished its `dragstart` turn.
            let drag_out_service: std::sync::Mutex<
                Box<dyn drag_out_service::DragOutService>,
            > = std::sync::Mutex::new(Box::new(
                drag_out_service::native_drag_out_service(app.handle().clone()),
            ));
            app.manage(drag_out_service);

            let preferences_path = app
                .path()
                .app_config_dir()
                .expect("application config directory unavailable")
                .join("player-preferences.json");
            let preferences: SharedPlayerPreferencesRepository = Arc::new(Mutex::new(Box::new(
                JsonPlayerPreferencesRepository::new(preferences_path),
            )));
            app.manage(preferences);

            // Technical cache on its own worker. A cache failure must never
            // prevent Audio Player startup, so startup logs and continues.
            // The same worker backs the waveform service through its own port;
            // without a cache the service degrades to extract-without-store.
            // The file watcher uses the same port to invalidate stale rows.
            let mut watcher_cache: Option<Arc<dyn WaveformCachePort>> = None;
            let mut waveform_service: Option<Arc<dyn WaveformService>> = None;
            // Rename service starts without a cache port and is replaced with
            // the opened cache below, so a cache failure never prevents rename
            // while a healthy cache lets PulseSeek invalidate the old row
            // proactively (FR-FM-010).
            let mut rename_service: std::sync::Mutex<Box<dyn RenameService>> =
                std::sync::Mutex::new(Box::new(NativeRenameService::new(
                    pulseseek_browser_fs::rename::NativeFileRename,
                    None,
                )));
            let mut recent_service: std::sync::Mutex<
                Box<dyn recent_folders_service::RecentFoldersService>,
            > = std::sync::Mutex::new(Box::new(
                recent_folders_service::InMemoryRecentFoldersService::new(),
            ));
            let mut shortcut_service: std::sync::Mutex<Box<dyn ShortcutMappingsService>> =
                std::sync::Mutex::new(Box::new(InMemoryShortcutMappingsService::new()));
            if let Ok(config_dir) = app.path().app_config_dir() {
                let cache_path = config_dir.join("app-cache.sqlite");
                match pulseseek_cache::technical_cache::TechnicalCache::start(&cache_path) {
                    Ok(cache) => {
                        let status = cache.status();
                        let recent_port: Arc<
                            dyn pulseseek_cache::recent_folders::RecentFoldersCachePort,
                        > = Arc::new(cache.clone());
                        let meta_port: Arc<
                            dyn pulseseek_cache::technical_cache::TechnicalCachePort,
                        > = Arc::new(cache.clone());
                        let bookmark_meta_port = Arc::clone(&meta_port);
                        let shortcut_port: Arc<
                            dyn pulseseek_cache::shortcut_mappings::ShortcutMappingsCachePort,
                        > = Arc::new(cache.clone());
                        let waveform_port: Arc<dyn WaveformCachePort> = Arc::new(cache);
                        watcher_cache = Some(Arc::clone(&waveform_port));
                        tracing::info!(status = ?status, "technical cache ready");
                        app.manage(meta_port);
                        recent_service = std::sync::Mutex::new(Box::new(
                            recent_folders_service::NativeRecentFoldersService::new(
                                recent_port,
                                Some(bookmark_meta_port),
                            ),
                        ));
                        shortcut_service = std::sync::Mutex::new(Box::new(
                            NativeShortcutMappingsService::new(shortcut_port),
                        ));
                        waveform_service = Some(Arc::new(
                            NativeWaveformService::new(Some(waveform_port))
                                .with_events(Arc::clone(&event_emitter)),
                        ));
                    },
                    Err(error) => {
                        tracing::warn!(
                            error = %error,
                            "technical cache unavailable; continuing without cache"
                        );
                        waveform_service = Some(Arc::new(
                            NativeWaveformService::new(None)
                                .with_events(Arc::clone(&event_emitter)),
                        ));
                    },
                }
            }
            app.manage(recent_service);
            app.manage(shortcut_service);

            // Rename service with the opened cache so a PulseSeek rename
            // proactively invalidates the old waveform row (FR-FM-010).
            if let Some(cache_port) = watcher_cache.clone() {
                rename_service = std::sync::Mutex::new(Box::new(NativeRenameService::new(
                    pulseseek_browser_fs::rename::NativeFileRename,
                    Some(cache_port),
                )));
            }
            app.manage(rename_service);
            // Move service: runs each batch on its own worker thread, streams
            // per-file progress events, invalidates moved-away waveform rows
            // through the opened cache, and reconciles the tracked playback
            // path when the playing file moves. Cache and reconcile failures
            // never fail the move itself.
            let playback_for_reconcile: std::sync::Arc<
                std::sync::Mutex<Box<dyn playback_service::PlaybackService>>,
            > = app
                .state::<std::sync::Arc<
                    std::sync::Mutex<Box<dyn playback_service::PlaybackService>>,
                >>()
                .inner()
                .clone();
            let move_service: std::sync::Mutex<Box<dyn move_service::MoveService>> =
                std::sync::Mutex::new(Box::new(
                    move_service::NativeMoveService::new(
                        pulseseek_browser_fs::move_file::NativeFileMover::new(),
                    )
                    .with_cache(watcher_cache.clone())
                    .with_reconcile(Some(Arc::new(move |old: &str, new: &str| {
                        match playback_for_reconcile.lock() {
                            Ok(mut playback) => playback.reconcile_path(old, new),
                            Err(_) => Err(
                                pulseseek_domain::error::ApplicationError::new(
                                    pulseseek_domain::error::ErrorCategory::Internal,
                                    pulseseek_domain::error::DiagnosticContext::new(
                                        pulseseek_domain::error::DiagnosticCode::PlaybackControl,
                                    ),
                                    std::io::Error::other("playback service lock poisoned"),
                                ),
                            ),
                        }
                    }))),
                ));
            app.manage(move_service);

            // Copy service: runs each batch on its own worker thread and
            // streams per-file progress events. Copying never modifies the
            // source, so there is no playback reconcile or cache
            // invalidation; the original keeps its waveform row and the new
            // copy simply has no cached row yet.
            let copy_service: std::sync::Mutex<Box<dyn copy_service::CopyService>> =
                std::sync::Mutex::new(Box::new(
                    copy_service::NativeCopyService::new(
                        pulseseek_browser_fs::copy_file::NativeFileCopier::new(),
                    ),
                ));
            app.manage(copy_service);

            // Waveform service always exists: with a cache port when the
            // cache opened, without one when it did not. It is only reachable
            // through the async `get_waveform` command.
            app.manage(
                waveform_service.unwrap_or_else(|| {
                    Arc::new(
                        NativeWaveformService::new(None)
                            .with_events(Arc::clone(&event_emitter)),
                    )
                }),
            );

            // Wire real events into the playback service before manage moves
            // the emitter into Tauri managed state.
            if let Ok(mut playback) = app
                .state::<std::sync::Arc<
                    std::sync::Mutex<Box<dyn playback_service::PlaybackService>>,
                >>()
                .lock()
            {
                playback.set_events(Some(Arc::clone(&event_emitter)));
            }

            app.manage(event_emitter);

            // Inject the file watcher into the enumeration service so it
            // watches the browsed folder for external changes (FR-BR-008).
            let watcher = match file_watcher_service::NativeFileWatcherService::with_defaults(
                // We already managed event_emitter; cloning the Arc is safe.
                // The debouncer callback runs on its own thread and holds an
                // Arc clone, keeping the emitter alive independently.
                (*app.state::<Arc<dyn playback_events::PlaybackEventEmitter>>()).clone(),
                watcher_cache,
            ) {
                Ok(watcher) => {
                    Box::new(watcher) as Box<dyn file_watcher_service::FileWatcherService>
                },
                Err(error) => {
                    tracing::warn!(
                        error = %error,
                        "file watcher unavailable; continuing without watching"
                    );
                    Box::new(file_watcher_service::FakeFileWatcherService::new())
                        as Box<dyn file_watcher_service::FileWatcherService>
                },
            };
            if let Ok(mut enum_service) =
                app.state::<std::sync::Mutex<Box<dyn folder_enumeration_service::FolderEnumerationService>>>().lock()
            {
                enum_service.set_watcher(watcher);
            }

            // Native folder picker dialog backed by the OS. Wrapped in Arc so
            // the command handler can clone it and spawn a blocking dialog
            // without holding a MutexGuard across an async boundary.
            let folder_picker: std::sync::Arc<
                std::sync::Mutex<Box<dyn dialog_service::FolderPicker>>,
            > = std::sync::Arc::new(std::sync::Mutex::new(Box::new(
                dialog_service::TauriFolderPicker::new(app.handle().clone()),
            )));
            app.manage(folder_picker);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
