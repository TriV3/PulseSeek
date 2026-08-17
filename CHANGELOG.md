# Changelog

All notable changes to PulseSeek are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.1.0](https://github.com/TriV3/PulseSeek/compare/v1.0.6...v1.1.0) (2026-08-17)


### Added

* **ci:** build Windows ARM64 NSIS installer ([189eadd](https://github.com/TriV3/PulseSeek/commit/189eadd6792b5c5bc4e017b3b3285b2fba2f9676))


### Fixed

* **ci:** ad-hoc sign macOS bundles for Apple Silicon ([f5c54fa](https://github.com/TriV3/PulseSeek/commit/f5c54fa74e6cf9900101b0dd0bac0f154724aa36))


### Changed

* **release:** align release-notes footer with signing and ARM64 status ([0481138](https://github.com/TriV3/PulseSeek/commit/0481138cf85945cd1aba12d89aabae864b7d17c7))

## [1.0.6](https://github.com/TriV3/PulseSeek/compare/v1.0.5...v1.0.6) (2026-08-17)


### Fixed

* **ci:** restore changelog and platform table in release bodies ([d9ae9b2](https://github.com/TriV3/PulseSeek/commit/d9ae9b25c50b39b8d7e16b02468e25873ae07fd3))

## [1.0.5](https://github.com/TriV3/PulseSeek/compare/v1.0.4...v1.0.5) (2026-08-17)


### Fixed

* **ci:** grant actions permission and align release-please inputs ([6475a7b](https://github.com/TriV3/PulseSeek/commit/6475a7b1393cdf0b99b3f4b1723cfccfa247fe3c))

## [1.0.4](https://github.com/TriV3/PulseSeek/compare/v1.0.3...v1.0.4) (2026-08-17)


### Fixed

* **ci:** check out repository before dispatching binary builds ([bb7e572](https://github.com/TriV3/PulseSeek/commit/bb7e5727227dabd949a578c51a96f5295f124aa5))

## [1.0.3](https://github.com/TriV3/PulseSeek/compare/v1.0.2...v1.0.3) (2026-08-17)


### Fixed

* **ci:** dispatch binary builds after release-please publishes ([c0282fc](https://github.com/TriV3/PulseSeek/commit/c0282fc070eebf27ac2c284937d2306e532a8d10))


### Changed

* **playback:** make seek polling deterministic under load ([90b62b4](https://github.com/TriV3/PulseSeek/commit/90b62b48f1884373e64b5ba414dad90bbd8ab23c))
* **release:** add platform installation table to release notes ([7d1d165](https://github.com/TriV3/PulseSeek/commit/7d1d165c1f6c658ff3b5b0da6dcf6a6c75ba6b45))

## [1.0.2](https://github.com/TriV3/PulseSeek/compare/v1.0.1...v1.0.2) (2026-08-17)


### Fixed

* **ci:** trigger release builds on API-created releases ([957c868](https://github.com/TriV3/PulseSeek/commit/957c86847ce238b9a0e4393e750f2709dbf469fb))

## [1.0.1](https://github.com/TriV3/PulseSeek/compare/v1.0.0...v1.0.1) (2026-08-17)


### Fixed

* **bundle:** enable bundling in tauri config ([fd91a64](https://github.com/TriV3/PulseSeek/commit/fd91a6414bf8a5650385f8d605990b01a3e4e666))
* **ci:** make release-please work with the cargo workspace ([1983dc8](https://github.com/TriV3/PulseSeek/commit/1983dc8ae0c5543cb84e985a9460631c26407106))


### Changed

* **ci:** pin release-please tags to plain vX.Y.Z ([62e429a](https://github.com/TriV3/PulseSeek/commit/62e429a112092fa50f5940f74b0b705895bba836))

## [1.0.0] - 2026-08-16

### Added

- **First stable release** — PulseSeek is a desktop audio player and folder
  browser for local music collections.

### Changed

- **Audio playback** — Play WAV, MP3, FLAC, AIFF, Ogg Vorbis, and M4A/AAC
  files with gapless sequential playback, seek steps, playback modes, and
  output-device selection.
- **Folder browsing** — Navigate the filesystem tree, bookmark folders,
  browse recent folders, and filter files; browsing is read-only and never
  imports files into any manager.
- **Waveform and analysis** — Display waveform previews with zoom and stable
  A-B looping, plus musical-spectrum, linear, and log visualizations.
- **Player controls** — Transport controls, keyboard shortcuts, drag-and-drop
  playback, external file opening, and drag-out reveal of the playing file.
- **Themes and accessibility** — Light, dark, system, Midnight Blue, and High
  Contrast themes, with keyboard-accessible workflows throughout.
- **File operations** — Move, copy, and rename files, and move deletion
  targets to the operating system trash.
