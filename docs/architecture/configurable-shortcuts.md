# Configurable Shortcuts

PR-080 implements one application-wide shortcut profile for FR-KB-003 and
FR-KB-004. Shortcuts work while the PulseSeek window has focus; they are not
operating-system global shortcuts.

## Model and ownership

- `pulseseek-domain` owns stable action identifiers, canonical defaults,
  logical chords, and validation.
- A chord stores a logical key plus `primary`, Shift, and Alt flags. `primary`
  means Command on macOS and Control on Windows/Linux.
- `pulseseek-cache` persists the complete available profile in schema version
  4 of `app-cache.sqlite`. SQLite work stays on the technical-cache worker.
- Tauri validates every complete profile before one transaction replaces the
  stored mappings. React applies only mappings returned by a successful Tauri
  load, save, or reset command.
- The editor keeps draft and confirmed mappings separate. Invalid drafts never
  affect active shortcuts or persistent state.

## Defaults

| Action | Default |
| --- | --- |
| Open folder | Primary+O |
| Play/pause | Space |
| Play selection | Enter |
| Previous/next | Up / Down |
| Seek backward/forward | Left / Right; step configurable in player settings |
| Toggle current-file loop | L |
| Move to Trash | Delete |
| Refresh | Primary+R |
| Search | Primary+F |
| One-shot/loop-current/sequential/random mode | Primary+Alt+1/2/3/4 |
| Keep/maybe/reject/favorite/clear mark | Primary+Shift+K/M/R/F/U |

A-B action identifiers are reserved and shown as unavailable until PR-087–089
provides loop-region behavior. They are not persisted in PR-080.

## Validation

- Keys use normalized `KeyboardEvent.key` values, preserving logical keyboard
  layout behavior rather than physical key positions.
- Modifier matching is exact. Extra modifiers do not trigger a command.
- Every available action must occur exactly once and every chord must be
  unique. Conflicts appear in the editor and disable Save; backend validation
  remains authoritative.
- Modifier-only keys, Tab, Escape, and Enter are reserved. Enter is allowed only
  for native Play Selection activation.
- Primary+Q and Primary+W are reserved on every platform. macOS also reserves
  Primary+H, Primary+M, and Primary+Space. Windows/Linux reserve Alt+F4.

## Focus and accessibility

- App shortcuts stop while a modal is open.
- Editable controls suppress every app shortcut except Search, which moves
  focus to the file-list search field.
- Native widget activation and navigation keys retain priority. Handlers honor
  `defaultPrevented`, preventing duplicate tree, grid, splitter, or waveform
  actions.
- Shortcut editor traps focus, supports keyboard capture, restores prior focus,
  reports validation through alerts, and exposes explicit Save, Reset, and
  Cancel actions.

## Failure modes

- Cache unavailable: Audio Player starts with defaults and uses session-only
  in-memory mappings.
- Corrupt cache: normal technical-cache quarantine/recreation applies; defaults
  are returned when no profile remains.
- Invalid or conflicting save: transaction is not started, active mappings stay
  unchanged, and editor displays failure.
- Persistence failure: confirmed mappings stay active; draft remains available
  for correction or retry.
- Reset failure: current profile remains active and editor reports failure.
