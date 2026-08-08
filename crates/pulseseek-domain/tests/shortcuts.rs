use pulseseek_domain::shortcuts::{
    default_shortcut_mappings, validate_complete_shortcut_mappings, validate_shortcut_mappings,
    Platform, ShortcutAction, ShortcutChord, ShortcutError, ShortcutMapping,
};

#[test]
fn action_ids_are_stable_and_round_trip() {
    for action in ShortcutAction::ALL {
        assert_eq!(ShortcutAction::from_id(action.id()), Some(*action));
    }
    assert_eq!(ShortcutAction::from_id("unknown"), None);
    assert_eq!(ShortcutAction::SetAbStart.id(), "set_ab_start");
}

#[test]
fn defaults_cover_existing_commands_refresh_search_modes_and_marks() {
    let defaults = default_shortcut_mappings();
    let actions: Vec<_> = defaults.iter().map(|mapping| mapping.action).collect();

    for action in [
        ShortcutAction::OpenFolder,
        ShortcutAction::TogglePlayPause,
        ShortcutAction::PlaySelection,
        ShortcutAction::PreviousTrack,
        ShortcutAction::NextTrack,
        ShortcutAction::SeekBackward,
        ShortcutAction::SeekForward,
        ShortcutAction::ToggleLoop,
        ShortcutAction::MoveToTrash,
        ShortcutAction::Refresh,
        ShortcutAction::FocusSearch,
        ShortcutAction::SetPlaybackModeOneShot,
        ShortcutAction::SetPlaybackModeLoopCurrent,
        ShortcutAction::SetPlaybackModeSequential,
        ShortcutAction::SetPlaybackModeRandom,
        ShortcutAction::MarkKeep,
        ShortcutAction::MarkMaybe,
        ShortcutAction::MarkReject,
        ShortcutAction::MarkFavorite,
        ShortcutAction::MarkClear,
    ] {
        assert!(actions.contains(&action), "missing default for {action:?}");
    }

    for unavailable in
        [ShortcutAction::SetAbStart, ShortcutAction::SetAbEnd, ShortcutAction::ToggleAbRepeat]
    {
        assert!(!actions.contains(&unavailable), "A-B action must remain unbound");
        assert!(!unavailable.is_available());
    }
    validate_shortcut_mappings(&defaults, Platform::MacOs).expect("mac defaults valid");
    validate_shortcut_mappings(&defaults, Platform::Windows).expect("windows defaults valid");
    validate_shortcut_mappings(&defaults, Platform::Linux).expect("linux defaults valid");
}

#[test]
fn chord_normalization_is_logical_and_deterministic() {
    assert_eq!(
        ShortcutChord::new("  ARROWLEFT  ", true, false, true).normalize().unwrap(),
        ShortcutChord::new("arrowleft", true, false, true)
    );
    assert_eq!(
        ShortcutChord::new(" ", false, false, false).normalize().unwrap(),
        ShortcutChord::new("space", false, false, false)
    );
    assert!(matches!(
        ShortcutChord::new("Control", false, false, false).normalize(),
        Err(ShortcutError::InvalidChord { .. })
    ));
}

#[test]
fn normalized_duplicate_chords_are_rejected() {
    let mappings = vec![
        ShortcutMapping::new(
            ShortcutAction::OpenFolder,
            ShortcutChord::new("K", true, false, false),
        ),
        ShortcutMapping::new(
            ShortcutAction::FocusSearch,
            ShortcutChord::new(" k ", true, false, false),
        ),
    ];

    assert!(matches!(
        validate_shortcut_mappings(&mappings, Platform::Linux),
        Err(ShortcutError::DuplicateChord { .. })
    ));
}

#[test]
fn duplicate_actions_and_unavailable_actions_are_rejected() {
    let duplicate_action = vec![
        ShortcutMapping::new(ShortcutAction::Refresh, ShortcutChord::key("f5")),
        ShortcutMapping::new(ShortcutAction::Refresh, ShortcutChord::key("f6")),
    ];
    assert!(matches!(
        validate_shortcut_mappings(&duplicate_action, Platform::Linux),
        Err(ShortcutError::DuplicateAction(ShortcutAction::Refresh))
    ));

    let unavailable =
        vec![ShortcutMapping::new(ShortcutAction::SetAbStart, ShortcutChord::key("a"))];
    assert!(matches!(
        validate_shortcut_mappings(&unavailable, Platform::Linux),
        Err(ShortcutError::UnavailableAction(ShortcutAction::SetAbStart))
    ));
}

#[test]
fn complete_profile_requires_every_available_action() {
    let partial = vec![ShortcutMapping::new(ShortcutAction::Refresh, ShortcutChord::key("f5"))];

    assert!(matches!(
        validate_complete_shortcut_mappings(&partial, Platform::Linux),
        Err(ShortcutError::IncompleteProfile)
    ));
    validate_complete_shortcut_mappings(&default_shortcut_mappings(), Platform::Linux)
        .expect("defaults form a complete profile");
}

#[test]
fn platform_reserved_shortcuts_are_rejected() {
    let mac_quit = vec![ShortcutMapping::new(
        ShortcutAction::Refresh,
        ShortcutChord::new("q", true, false, false),
    )];
    assert!(matches!(
        validate_shortcut_mappings(&mac_quit, Platform::MacOs),
        Err(ShortcutError::ReservedChord { platform: Platform::MacOs, .. })
    ));
    assert!(matches!(
        validate_shortcut_mappings(&mac_quit, Platform::Windows),
        Err(ShortcutError::ReservedChord { platform: Platform::Windows, .. })
    ));

    let close_window = vec![ShortcutMapping::new(
        ShortcutAction::Refresh,
        ShortcutChord::new("f4", false, false, true),
    )];
    assert!(matches!(
        validate_shortcut_mappings(&close_window, Platform::Windows),
        Err(ShortcutError::ReservedChord { platform: Platform::Windows, .. })
    ));
    assert!(matches!(
        validate_shortcut_mappings(&close_window, Platform::Linux),
        Err(ShortcutError::ReservedChord { platform: Platform::Linux, .. })
    ));

    for key in ["tab", "escape", "enter"] {
        let reserved = vec![ShortcutMapping::new(ShortcutAction::Refresh, ShortcutChord::key(key))];
        assert!(matches!(
            validate_shortcut_mappings(&reserved, Platform::Linux),
            Err(ShortcutError::ReservedChord { .. })
        ));
    }

    let native_play_selection =
        vec![ShortcutMapping::new(ShortcutAction::PlaySelection, ShortcutChord::key("enter"))];
    validate_shortcut_mappings(&native_play_selection, Platform::Linux)
        .expect("Enter remains valid for native play-selection activation");
}
