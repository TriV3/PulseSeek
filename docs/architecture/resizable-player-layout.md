# Resizable player layout

The Audio Player shell follows a compact, Resonic-inspired hierarchy without
coupling the browser to a manager database:

1. The waveform overview occupies the upper workspace.
2. Playback, mode, volume, and output controls stay in a fixed transport strip.
3. The read-only folder browser and playable-file table share the lower
   workspace.

The browser loads its top-level roots from Rust at startup. On macOS this means
the system volume plus every mounted volume under `/Volumes`, including mounted
network shares. Linux uses the system root and conventional mount locations;
Windows exposes available drive letters. PulseSeek does not initiate network
connections: a remote share must first be mounted by the operating system.

The first launch presents a collapsed `Computer` root, leaving every disk and
folder closed until requested. A single click, Enter, or Space selects a
folder, toggles its expanded state, and starts a background enumeration the
first time it is opened. Each nested list applies the same compact indentation
step. Only playable audio files are sent to the file table. Browsing remains
read-only and never imports a file into a manager database.

Enumeration uses two phases. A lightweight preview reads directory entries
without opening audio decoders, so subfolders and recognized audio filenames
render immediately even on large or network-mounted directories. Before a
child folder is emitted, a bounded worker pool performs one shallow read to
determine whether it has subfolders. Known leaves therefore render without an
expand control from their first frame. Selecting a known leaf starts its file
enumeration directly, without temporarily adding an expand control. A
cancellable verification pass then probes direct-child audio candidates
concurrently and enriches trusted files in small progressive batches; rejected
candidates are removed. Moving to another folder cancels that verification
between files so obsolete scans do not monopolize background workers. Until the
complete scan finishes, both panes remain in a loading state rather than
displaying a false empty result.
When an audio file is selected, every visible ancestor folder in its filesystem
path is rendered with bold text and the theme-specific
`--folder-active-path` semantic color. Every theme keeps this token at WCAG AA
contrast against the normal, hover, and selected browser backgrounds.
Segment-aware matching prevents similarly prefixed sibling folders from being
highlighted.

The left workspace column has two keyboard-accessible tabs, Browser and Recent
folders. Their panels occupy the same full-height area instead of stacking.
Switching tabs preserves the mounted folder tree and its expansion state;
reopening a recent folder returns to the Browser tab at that location.

The horizontal separator resizes the waveform between 22% and 62% of the
window. The vertical separator resizes the folder browser between 16% and 46%
of the lower workspace. Pointer dragging provides direct manipulation;
horizontal or vertical arrow keys change the focused separator by two
percentage points. Limits keep every primary workflow visible and usable.

The waveform currently provides the visual shell for the future Canvas 2D
renderer. It does not read or analyze audio in React and does not claim to show
samples from the selected file. Real waveform data must later cross a narrow,
typed Tauri boundary and drawing must remain outside React renders.
