# Resizable player layout

The Audio Player shell follows a compact hierarchy without coupling the browser
to a manager database:

1. The waveform overview occupies the upper workspace.
2. Playback, mode, volume, and output controls stay in a fixed transport strip.
3. The read-only folder browser and playable-file table share the lower
   workspace.

The browser loads its top-level roots from Rust at startup. On macOS this means
the system volume plus every mounted volume under `/Volumes`, including mounted
network shares. Linux uses the system root and conventional mount locations;
Windows exposes available drive letters. PulseSeek does not initiate network
connections: a remote share must first be mounted by the operating system.
On macOS, the existing mount table classifies SMB, AFP, NFS, WebDAV, SSHFS, and
other remote filesystems as network roots; Linux uses `/proc/self/mounts` for
the same classification. If mount metadata is unavailable, a discovered
non-system root safely falls back to the physical-volume presentation.

The first launch presents expanded `Drives` and `Libraries` sections. `Drives`
contains the system root, the user's `Home` directory as its own drive, and
physical and network-mounted roots; `Libraries` contains
the existing Documents, Music, Pictures, Videos, and Downloads directories for
the current operating-system user. Clicking either section heading collapses
only that section, and the same controls expose `aria-expanded` for keyboard
and assistive-technology users. Each
root type has a distinct SVG icon and a semantic color supplied by every
theme; ordinary directories keep a separate folder icon. A single click,
Enter, or Space selects a folder, toggles its expanded state, and starts a
background enumeration the first time it is opened. Each nested list applies
the same compact indentation step. Only playable audio files are sent to the
file table. Browsing remains read-only and never imports a file into a manager
database.
`Go Up` selects the parent and collapses it while retaining its already loaded
children. This exposes the parent among its siblings for orientation, persists
the collapsed state, and prevents a late enumeration chunk from reopening it.
The application menu is isolated at the far right of the transport strip by a
semantic divider. General actions and settings, including audio output, theme,
hidden folders, and Keyboard shortcuts, live inside this menu; playback mode
and waveform style remain grouped as transport settings.

Directories whose names start with `.` are omitted by the filesystem adapter
by default, including during recursive enumeration. The persisted `Show hidden
folders` option exposes them and immediately refreshes the selected directory;
disabling it hides them again without changing any source file.
The broad system and Home roots are not watched for live changes: doing so
would include PulseSeek's own cache and preference writes and could create a
refresh loop when hidden directories are visible. Their contents are still
fully enumerable, and ordinary selected directories retain live direct-child
refreshes. Delayed or nested watcher events are discarded before reaching the
UI.

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

The active folder is watched for external content changes. Access-only and
metadata-only filesystem notifications are ignored because reading some local
or network filesystems can produce them. The frontend accepts a watcher refresh
only for the currently selected folder and permits at most one pending
enumeration per path, preventing an old or duplicated notification from
cancelling the user's new selection or creating a refresh loop.

When an audio file is selected, every visible ancestor folder in its filesystem
path is rendered with bold text and the theme-specific
`--folder-active-path` semantic color. Every theme keeps this token at WCAG AA
contrast against the normal, hover, and selected browser backgrounds.
Segment-aware matching prevents similarly prefixed sibling folders from being
highlighted.

The left workspace column has three keyboard-accessible tabs: Browser,
Bookmarks, and Recent folders. Their panels occupy the same full-height area
instead of stacking. Switching tabs preserves the mounted folder tree and its
expansion state. Any real directory can be bookmarked from the Browser toolbar;
bookmarks persist in the technical cache and can be reopened or removed from
their tab. A missing or temporarily disconnected bookmarked directory remains
listed until explicitly removed and fails safely when reopened. Browsing and
bookmarking never imports a file or writes a manager database.
Every bookmarked folder visible in the Browser uses the semantic
`--folder-bookmark` color for its label and folder icon. The toolbar bookmark
star uses the same token, and every theme keeps it at AA contrast across the
normal, hover, and selected browser backgrounds.

The horizontal separator resizes the waveform between 22% and 62% of the
window. The vertical separator resizes the folder browser between 16% and 46%
of the lower workspace. Pointer dragging provides direct manipulation;
horizontal or vertical arrow keys change the focused separator by two
percentage points. Limits keep every primary workflow visible and usable.

The waveform currently provides the visual shell for the future Canvas 2D
renderer. It does not read or analyze audio in React and does not claim to show
samples from the selected file. Real waveform data must later cross a narrow,
typed Tauri boundary and drawing must remain outside React renders.
