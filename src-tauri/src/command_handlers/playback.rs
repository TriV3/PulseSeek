use std::sync::Arc;

use pulseseek_domain::playback::mode::PlaybackMode;
use serde_json::Value;

use crate::command_envelope::types::{
    ClearLoopRegionRequest, ClearLoopRegionResponse, ClearPreparedResponse, PauseRequest,
    PauseResponse, PlayRequest, PlayResponse, PrepareNextRequest, PrepareNextResponse,
    ResumeRequest, ResumeResponse, SeekRequest, SeekResponse, SetLoopRegionRequest,
    SetLoopRegionResponse, SetPlaybackModeRequest, SetPlaybackModeResponse, StopRequest,
    StopResponse, VolumeRequest, VolumeResponse,
};
use crate::command_envelope::{from_application_error, BoundaryError, CommandResponse};
use crate::command_handlers::parse_payload;
use crate::playback_events::PlaybackEventEmitter;
use crate::playback_service::PlaybackService;

/// Handles playback commands: play, pause, resume, stop, seek, volume, and
/// set_playback_mode.
pub(crate) fn handle(
    command: &str,
    payload: Value,
    service: &mut dyn PlaybackService,
    events: &Arc<dyn PlaybackEventEmitter>,
) -> CommandResponse {
    match command {
        "play" => {
            let request: PlayRequest = match parse_payload("play", payload) {
                Ok(request) => request,
                Err(response) => return response,
            };
            match service.play(&request.path) {
                Ok(()) => {
                    let _ = events.emit_state("playing");
                    CommandResponse::ok(serde_json::to_value(PlayResponse {}).unwrap())
                },
                Err(e) => CommandResponse::err(from_application_error(&e)),
            }
        },
        "prepare_next" => {
            let request: PrepareNextRequest = match parse_payload("prepare_next", payload) {
                Ok(request) => request,
                Err(response) => return response,
            };
            match service.prepare_next(&request.path) {
                Ok(()) => {
                    CommandResponse::ok(serde_json::to_value(PrepareNextResponse {}).unwrap())
                },
                Err(e) => CommandResponse::err(from_application_error(&e)),
            }
        },
        "clear_prepared" => match service.clear_prepared() {
            Ok(()) => CommandResponse::ok(serde_json::to_value(ClearPreparedResponse {}).unwrap()),
            Err(e) => CommandResponse::err(from_application_error(&e)),
        },
        "pause" => {
            let _request: PauseRequest = match parse_payload("pause", payload) {
                Ok(request) => request,
                Err(response) => return response,
            };
            match service.pause() {
                Ok(()) => {
                    let _ = events.emit_state("paused");
                    CommandResponse::ok(serde_json::to_value(PauseResponse {}).unwrap())
                },
                Err(e) => CommandResponse::err(from_application_error(&e)),
            }
        },
        "resume" => {
            let _request: ResumeRequest = match parse_payload("resume", payload) {
                Ok(request) => request,
                Err(response) => return response,
            };
            match service.resume() {
                Ok(()) => {
                    let _ = events.emit_state("playing");
                    CommandResponse::ok(serde_json::to_value(ResumeResponse {}).unwrap())
                },
                Err(e) => CommandResponse::err(from_application_error(&e)),
            }
        },
        "stop" => {
            let _request: StopRequest = match parse_payload("stop", payload) {
                Ok(request) => request,
                Err(response) => return response,
            };
            match service.stop() {
                Ok(()) => {
                    let _ = events.emit_state("stopped");
                    CommandResponse::ok(serde_json::to_value(StopResponse {}).unwrap())
                },
                Err(e) => CommandResponse::err(from_application_error(&e)),
            }
        },
        "seek" => {
            let request: SeekRequest = match parse_payload("seek", payload) {
                Ok(request) => request,
                Err(response) => return response,
            };
            match service.seek(request.position_ms) {
                Ok(position_ms) => {
                    CommandResponse::ok(serde_json::to_value(SeekResponse { position_ms }).unwrap())
                },
                Err(e) => CommandResponse::err(from_application_error(&e)),
            }
        },
        "volume" => {
            let request: VolumeRequest = match parse_payload("volume", payload) {
                Ok(request) => request,
                Err(response) => return response,
            };
            match service.set_volume(request.gain, request.muted) {
                Ok(()) => CommandResponse::ok(serde_json::to_value(VolumeResponse {}).unwrap()),
                Err(e) => CommandResponse::err(from_application_error(&e)),
            }
        },
        "set_playback_mode" => {
            let request: SetPlaybackModeRequest = match parse_payload("set_playback_mode", payload)
            {
                Ok(request) => request,
                Err(response) => return response,
            };
            let mode = match parse_playback_mode(&request.mode) {
                Some(mode) => mode,
                None => {
                    return CommandResponse::err(BoundaryError {
                        category: "InvalidInput".to_string(),
                        message: format!("Unknown playback mode: {}", request.mode),
                        diagnostic_code: "command.mode".to_string(),
                    });
                },
            };
            match service.set_mode(mode) {
                Ok(confirmed_mode) => CommandResponse::ok(
                    serde_json::to_value(SetPlaybackModeResponse {
                        mode: playback_mode_name(confirmed_mode),
                    })
                    .unwrap(),
                ),
                Err(e) => CommandResponse::err(from_application_error(&e)),
            }
        },
        "set_loop_region" => {
            let request: SetLoopRegionRequest = match parse_payload("set_loop_region", payload) {
                Ok(request) => request,
                Err(response) => return response,
            };
            match service.set_loop_region(request.start_ms, request.end_ms) {
                Ok(start_ms) => CommandResponse::ok(
                    serde_json::to_value(SetLoopRegionResponse { start_ms }).unwrap(),
                ),
                Err(e) => CommandResponse::err(from_application_error(&e)),
            }
        },
        "clear_loop_region" => {
            let _request: ClearLoopRegionRequest = match parse_payload("clear_loop_region", payload)
            {
                Ok(request) => request,
                Err(response) => return response,
            };
            match service.clear_loop_region() {
                Ok(()) => {
                    CommandResponse::ok(serde_json::to_value(ClearLoopRegionResponse {}).unwrap())
                },
                Err(e) => CommandResponse::err(from_application_error(&e)),
            }
        },
        _ => unreachable!("unhandled playback command: {command}"),
    }
}

fn parse_playback_mode(value: &str) -> Option<PlaybackMode> {
    match value {
        "one-shot" => Some(PlaybackMode::OneShot),
        "loop-current" => Some(PlaybackMode::LoopCurrent),
        "sequential" => Some(PlaybackMode::Sequential),
        "random" => Some(PlaybackMode::Random),
        _ => None,
    }
}

fn playback_mode_name(mode: PlaybackMode) -> String {
    match mode {
        PlaybackMode::OneShot => "one-shot",
        PlaybackMode::LoopCurrent => "loop-current",
        PlaybackMode::Sequential => "sequential",
        PlaybackMode::Random => "random",
    }
    .to_string()
}
