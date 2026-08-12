//! Platform-logical keyboard shortcut definitions and validation.

use std::collections::{HashMap, HashSet};
use std::fmt;

/// User-triggerable command with stable persistence identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ShortcutAction {
    OpenFolder,
    TogglePlayPause,
    PlaySelection,
    PreviousTrack,
    NextTrack,
    SeekBackward,
    SeekForward,
    ToggleLoop,
    MoveToTrash,
    Refresh,
    FocusSearch,
    SetPlaybackModeOneShot,
    SetPlaybackModeLoopCurrent,
    SetPlaybackModeSequential,
    SetPlaybackModeRandom,
    MarkKeep,
    MarkMaybe,
    MarkReject,
    MarkFavorite,
    MarkClear,
    SetAbStart,
    SetAbEnd,
    ToggleAbRepeat,
}

impl ShortcutAction {
    pub const ALL: &'static [Self] = &[
        Self::OpenFolder,
        Self::TogglePlayPause,
        Self::PlaySelection,
        Self::PreviousTrack,
        Self::NextTrack,
        Self::SeekBackward,
        Self::SeekForward,
        Self::ToggleLoop,
        Self::MoveToTrash,
        Self::Refresh,
        Self::FocusSearch,
        Self::SetPlaybackModeOneShot,
        Self::SetPlaybackModeLoopCurrent,
        Self::SetPlaybackModeSequential,
        Self::SetPlaybackModeRandom,
        Self::MarkKeep,
        Self::MarkMaybe,
        Self::MarkReject,
        Self::MarkFavorite,
        Self::MarkClear,
        Self::SetAbStart,
        Self::SetAbEnd,
        Self::ToggleAbRepeat,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::OpenFolder => "open_folder",
            Self::TogglePlayPause => "toggle_play_pause",
            Self::PlaySelection => "play_selection",
            Self::PreviousTrack => "previous_track",
            Self::NextTrack => "next_track",
            Self::SeekBackward => "seek_backward",
            Self::SeekForward => "seek_forward",
            Self::ToggleLoop => "toggle_loop",
            Self::MoveToTrash => "move_to_trash",
            Self::Refresh => "refresh",
            Self::FocusSearch => "focus_search",
            Self::SetPlaybackModeOneShot => "set_playback_mode_one_shot",
            Self::SetPlaybackModeLoopCurrent => "set_playback_mode_loop_current",
            Self::SetPlaybackModeSequential => "set_playback_mode_sequential",
            Self::SetPlaybackModeRandom => "set_playback_mode_random",
            Self::MarkKeep => "mark_keep",
            Self::MarkMaybe => "mark_maybe",
            Self::MarkReject => "mark_reject",
            Self::MarkFavorite => "mark_favorite",
            Self::MarkClear => "mark_clear",
            Self::SetAbStart => "set_ab_start",
            Self::SetAbEnd => "set_ab_end",
            Self::ToggleAbRepeat => "toggle_ab_repeat",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|action| action.id() == id)
    }

    /// All shipped actions are available; PR-089 activates A-B selection.
    pub const fn is_available(self) -> bool {
        true
    }
}

/// Desktop platform used only for reserved operating-system chord checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Platform {
    MacOs,
    Windows,
    Linux,
}

/// Logical chord. `primary` means Command on macOS and Control elsewhere.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ShortcutChord {
    pub key: String,
    pub primary: bool,
    pub shift: bool,
    pub alt: bool,
}

impl ShortcutChord {
    pub fn new(key: impl Into<String>, primary: bool, shift: bool, alt: bool) -> Self {
        Self { key: key.into(), primary, shift, alt }
    }

    pub fn key(key: impl Into<String>) -> Self {
        Self::new(key, false, false, false)
    }

    pub fn normalize(&self) -> Result<Self, ShortcutError> {
        let key = normalize_key(&self.key).ok_or_else(|| ShortcutError::InvalidChord {
            reason: "key must identify a non-modifier key".to_string(),
        })?;
        Ok(Self { key, primary: self.primary, shift: self.shift, alt: self.alt })
    }
}

/// One action-to-chord assignment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShortcutMapping {
    pub action: ShortcutAction,
    pub chord: ShortcutChord,
}

impl ShortcutMapping {
    pub fn new(action: ShortcutAction, chord: ShortcutChord) -> Self {
        Self { action, chord }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ShortcutError {
    InvalidChord { reason: String },
    IncompleteProfile,
    DuplicateAction(ShortcutAction),
    DuplicateChord { first: ShortcutAction, second: ShortcutAction, chord: ShortcutChord },
    ReservedChord { platform: Platform, chord: ShortcutChord },
    UnavailableAction(ShortcutAction),
}

impl fmt::Display for ShortcutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidChord { reason } => write!(f, "invalid shortcut chord: {reason}"),
            Self::IncompleteProfile => f.write_str("shortcut profile is incomplete"),
            Self::DuplicateAction(action) => {
                write!(f, "duplicate shortcut action: {}", action.id())
            },
            Self::DuplicateChord { first, second, .. } => {
                write!(f, "shortcut chord conflicts between {} and {}", first.id(), second.id())
            },
            Self::ReservedChord { platform, .. } => {
                write!(f, "shortcut chord is reserved on {platform:?}")
            },
            Self::UnavailableAction(action) => {
                write!(f, "shortcut action is unavailable: {}", action.id())
            },
        }
    }
}

impl std::error::Error for ShortcutError {}

/// Validates mappings after key normalization.
pub fn validate_shortcut_mappings(
    mappings: &[ShortcutMapping],
    platform: Platform,
) -> Result<(), ShortcutError> {
    validate_and_normalize_shortcut_mappings(mappings, platform).map(|_| ())
}

/// Validates that one profile contains every available action exactly once.
pub fn validate_complete_shortcut_mappings(
    mappings: &[ShortcutMapping],
    platform: Platform,
) -> Result<(), ShortcutError> {
    validate_complete_and_normalize_shortcut_mappings(mappings, platform).map(|_| ())
}

/// Returns one canonical complete profile suitable for persistence.
pub fn validate_complete_and_normalize_shortcut_mappings(
    mappings: &[ShortcutMapping],
    platform: Platform,
) -> Result<Vec<ShortcutMapping>, ShortcutError> {
    let normalized = validate_and_normalize_shortcut_mappings(mappings, platform)?;
    let available_count = ShortcutAction::ALL.iter().filter(|action| action.is_available()).count();
    if normalized.len() != available_count {
        return Err(ShortcutError::IncompleteProfile);
    }
    let actions: HashSet<_> = normalized.iter().map(|mapping| mapping.action).collect();
    if ShortcutAction::ALL
        .iter()
        .copied()
        .filter(|action| action.is_available())
        .any(|action| !actions.contains(&action))
    {
        return Err(ShortcutError::IncompleteProfile);
    }
    Ok(normalized)
}

/// Returns canonical mappings when every action and chord is safe and unique.
pub fn validate_and_normalize_shortcut_mappings(
    mappings: &[ShortcutMapping],
    platform: Platform,
) -> Result<Vec<ShortcutMapping>, ShortcutError> {
    let mut actions = HashSet::with_capacity(mappings.len());
    let mut chords = HashMap::with_capacity(mappings.len());
    let mut normalized = Vec::with_capacity(mappings.len());

    for mapping in mappings {
        if !mapping.action.is_available() {
            return Err(ShortcutError::UnavailableAction(mapping.action));
        }
        if !actions.insert(mapping.action) {
            return Err(ShortcutError::DuplicateAction(mapping.action));
        }
        let chord = mapping.chord.normalize()?;
        if is_reserved(mapping.action, &chord, platform) {
            return Err(ShortcutError::ReservedChord { platform, chord });
        }
        if let Some(first) = chords.insert(chord.clone(), mapping.action) {
            return Err(ShortcutError::DuplicateChord { first, second: mapping.action, chord });
        }
        normalized.push(ShortcutMapping::new(mapping.action, chord));
    }
    Ok(normalized)
}

/// Canonical single-profile defaults, preserving shipped shortcuts.
pub fn default_shortcut_mappings() -> Vec<ShortcutMapping> {
    use ShortcutAction as Action;

    vec![
        primary(Action::OpenFolder, "o"),
        plain(Action::TogglePlayPause, "space"),
        plain(Action::PlaySelection, "enter"),
        primary(Action::PreviousTrack, "arrowleft"),
        primary(Action::NextTrack, "arrowright"),
        plain(Action::SeekBackward, "arrowleft"),
        plain(Action::SeekForward, "arrowright"),
        plain(Action::ToggleLoop, "l"),
        plain(Action::MoveToTrash, "delete"),
        primary(Action::Refresh, "r"),
        primary(Action::FocusSearch, "f"),
        primary_alt(Action::SetPlaybackModeOneShot, "1"),
        primary_alt(Action::SetPlaybackModeLoopCurrent, "2"),
        primary_alt(Action::SetPlaybackModeSequential, "3"),
        primary_alt(Action::SetPlaybackModeRandom, "4"),
        primary_shift(Action::MarkKeep, "k"),
        primary_shift(Action::MarkMaybe, "m"),
        primary_shift(Action::MarkReject, "r"),
        primary_shift(Action::MarkFavorite, "f"),
        primary_shift(Action::MarkClear, "u"),
        // A-B region selection (PR-089): unprefixed bracket keys place A/B at
        // the playhead; "a" toggles A-B repeat.
        plain(Action::SetAbStart, "["),
        plain(Action::SetAbEnd, "]"),
        plain(Action::ToggleAbRepeat, "a"),
    ]
}

fn plain(action: ShortcutAction, key: &str) -> ShortcutMapping {
    ShortcutMapping::new(action, ShortcutChord::key(key))
}

fn primary(action: ShortcutAction, key: &str) -> ShortcutMapping {
    ShortcutMapping::new(action, ShortcutChord::new(key, true, false, false))
}

fn primary_shift(action: ShortcutAction, key: &str) -> ShortcutMapping {
    ShortcutMapping::new(action, ShortcutChord::new(key, true, true, false))
}

fn primary_alt(action: ShortcutAction, key: &str) -> ShortcutMapping {
    ShortcutMapping::new(action, ShortcutChord::new(key, true, false, true))
}

fn normalize_key(key: &str) -> Option<String> {
    let key = if key == " " { "space" } else { key.trim() };
    let lowered = key.to_lowercase();
    let canonical = match lowered.as_str() {
        "spacebar" => "space",
        "esc" => "escape",
        other => other,
    };
    if canonical.is_empty()
        || matches!(canonical, "alt" | "altgraph" | "control" | "ctrl" | "meta" | "shift" | "super")
    {
        None
    } else {
        Some(canonical.to_string())
    }
}

fn is_reserved(action: ShortcutAction, chord: &ShortcutChord, platform: Platform) -> bool {
    if matches!(chord.key.as_str(), "tab" | "escape")
        || (chord.key == "enter" && action != ShortcutAction::PlaySelection)
        || (chord.primary && !chord.shift && !chord.alt && matches!(chord.key.as_str(), "q" | "w"))
    {
        return true;
    }
    match platform {
        Platform::MacOs => {
            chord.primary
                && !chord.shift
                && !chord.alt
                && matches!(chord.key.as_str(), "h" | "m" | "space")
        },
        Platform::Windows | Platform::Linux => {
            chord.alt && !chord.primary && !chord.shift && chord.key == "f4"
        },
    }
}
