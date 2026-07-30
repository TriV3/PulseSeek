# Audio device selector

Audio output selection uses existing typed list/current/select commands and the
`audio:device-lost` event. Tauri now uses the cpal-backed adapter instead of the
fake device service. Missing devices fall back to the system default through the
native adapter; UI refresh exposes current availability and retry.

Hardware enumeration remains best-effort. CI tests use fakes and must not require
an audio device. Real playback integration is tracked in PR-049-2; until that
work lands, selecting a device updates native output service state but does not
rebind an active real playback stream.
