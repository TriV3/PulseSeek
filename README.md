# PulseSeek

PulseSeek is an open-source, cross-platform audio browser and library manager
written primarily in Rust.

Its first goal is deliberately focused: let users open any folder, preview its
audio files with minimal latency, navigate quickly, enable looping, and remove
unwanted files safely. No library or database is required for this workflow.

## Core principles

- Fast and lightweight
- Local-first and fully usable offline
- Cross-platform: macOS, Windows, and Linux
- Suitable for local disks, external drives, and mounted network storage
- Explicit imports: browsing a folder never adds files to the PulseSeek library
- Safe file management: deletion uses the operating system trash when possible
- Designed to scale to large audio collections

## Browser and library

PulseSeek treats the audio browser and the PulseSeek library as separate
concepts:

- The **browser** opens arbitrary folders and works without a user library.
- The **library** contains only items explicitly added by the user.

Technical cache data, such as waveforms or durations, does not turn a browsed
file into a library item.

## Initial milestone

The first milestone targets a macOS prototype that can:

- Open an arbitrary folder
- List supported audio files
- Select and immediately play a file
- Pause playback
- Toggle looping
- Move to the previous or next file
- Move unwanted files to the system trash

The initial format targets are WAV, FLAC, and MP3. AIFF, OGG, and M4A are
planned next.

## Direction

Later releases are expected to add waveforms, advanced file operations,
search, an explicitly managed sample and music library, playlists, audio
analysis, DJ exports, and DAW integration.

PulseSeek is intended to remain one coherent audio application: a place to
browse, listen to, select, organize, and use audio files.

## Technology

The application core will be developed in Rust. Desktop UI and supporting
library choices will be validated through prototypes before the architecture is
fixed.

## Project status

PulseSeek is at the specification and project-initialization stage.

## License

The project is intended to be open source. A license has not yet been selected.
