use std::sync::Arc;

use serde_json::Value;

use crate::audio_device_service::AudioDeviceService;
use crate::command_envelope::types::{
    CurrentDeviceRequest, CurrentDeviceResponse, ListDevicesRequest, ListDevicesResponse,
    SelectDeviceRequest, SelectDeviceResponse,
};
use crate::command_envelope::{from_application_error, CommandResponse};
use crate::command_handlers::parse_payload;
use crate::playback_events::{DeviceLostPayload, PlaybackEventEmitter, EVENT_DEVICE_LOST};
use crate::playback_service::PlaybackService;

/// Handles audio device commands: list_devices, current_device, and
/// select_device.
pub(crate) fn handle(
    command: &str,
    payload: Value,
    playback_service: &mut dyn PlaybackService,
    device_service: &mut dyn AudioDeviceService,
    events: &Arc<dyn PlaybackEventEmitter>,
) -> CommandResponse {
    match command {
        "list_devices" => {
            let _request: ListDevicesRequest = match parse_payload("list_devices", payload) {
                Ok(request) => request,
                Err(response) => return response,
            };
            match device_service.list_devices() {
                Ok(devices) => CommandResponse::ok(
                    serde_json::to_value(ListDevicesResponse { devices }).unwrap(),
                ),
                Err(e) => CommandResponse::err(from_application_error(&e)),
            }
        },
        "current_device" => {
            let _request: CurrentDeviceRequest = match parse_payload("current_device", payload) {
                Ok(request) => request,
                Err(response) => return response,
            };
            match device_service.current_device() {
                Ok(device) => CommandResponse::ok(
                    serde_json::to_value(CurrentDeviceResponse { device }).unwrap(),
                ),
                Err(e) => CommandResponse::err(from_application_error(&e)),
            }
        },
        "select_device" => {
            let request: SelectDeviceRequest = match parse_payload("select_device", payload) {
                Ok(request) => request,
                Err(response) => return response,
            };
            match playback_service.select_output_device(&request.device_id) {
                Ok(()) => {
                    if device_service.is_device_lost() {
                        let _ = events.emit(
                            EVENT_DEVICE_LOST,
                            serde_json::to_value(DeviceLostPayload {
                                previous_device_id: request.device_id.clone(),
                            })
                            .unwrap(),
                        );
                    }
                    CommandResponse::ok(serde_json::to_value(SelectDeviceResponse {}).unwrap())
                },
                Err(e) => CommandResponse::err(from_application_error(&e)),
            }
        },
        _ => unreachable!("unhandled device command: {command}"),
    }
}
