use std::sync::Arc;

use pulseseek_cache::technical_cache::TechnicalCachePort;

/// Removes only recalculable waveform data from technical cache.
#[tauri::command(rename = "clear_waveform_cache")]
pub fn clear_waveform_cache_command(
    cache: tauri::State<'_, std::sync::Mutex<Option<Arc<dyn TechnicalCachePort>>>>,
) -> Result<(), String> {
    let cache = cache.lock().map_err(|_| "Cache lock unavailable".to_string())?;
    cache
        .as_ref()
        .ok_or_else(|| "Technical cache unavailable".to_string())?
        .clear_waveform_cache()
        .map_err(|error| error.to_string())
}
