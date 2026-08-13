//! Application service for one complete configurable-shortcut profile.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use pulseseek_cache::shortcut_mappings::{ShortcutMappingsCachePort, ShortcutMappingsError};
use pulseseek_domain::error::{ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory};
use pulseseek_domain::shortcuts::{
    default_shortcut_mappings, legacy_default_shortcut_mappings,
    validate_and_normalize_shortcut_mappings, Platform, ShortcutAction, ShortcutChord,
    ShortcutError, ShortcutMapping,
};

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ShortcutMappingData {
    pub action_id: String,
    pub key: String,
    pub primary: bool,
    pub shift: bool,
    pub alt: bool,
}

impl From<&ShortcutMapping> for ShortcutMappingData {
    fn from(mapping: &ShortcutMapping) -> Self {
        Self {
            action_id: mapping.action.id().to_string(),
            key: mapping.chord.key.clone(),
            primary: mapping.chord.primary,
            shift: mapping.chord.shift,
            alt: mapping.chord.alt,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct ShortcutMappingsData {
    pub mappings: Vec<ShortcutMappingData>,
    pub unavailable_action_ids: Vec<String>,
}

pub trait ShortcutMappingsService: Send + Sync {
    fn load(&self) -> Result<ShortcutMappingsData, ApplicationError>;
    fn save(
        &self,
        mappings: Vec<ShortcutMappingData>,
    ) -> Result<ShortcutMappingsData, ApplicationError>;
    fn reset(&self) -> Result<ShortcutMappingsData, ApplicationError>;
}

pub struct NativeShortcutMappingsService {
    cache: Arc<dyn ShortcutMappingsCachePort>,
}

impl NativeShortcutMappingsService {
    pub fn new(cache: Arc<dyn ShortcutMappingsCachePort>) -> Self {
        Self { cache }
    }
}

impl ShortcutMappingsService for NativeShortcutMappingsService {
    fn load(&self) -> Result<ShortcutMappingsData, ApplicationError> {
        let mappings = self.cache.load_shortcut_mappings().map_err(cache_error)?;
        if mappings.is_empty() {
            return Ok(profile(default_shortcut_mappings()));
        }
        if mappings == legacy_default_shortcut_mappings() {
            return Ok(profile(default_shortcut_mappings()));
        }
        validate_complete(mappings).map(profile)
    }

    fn save(
        &self,
        mappings: Vec<ShortcutMappingData>,
    ) -> Result<ShortcutMappingsData, ApplicationError> {
        let mappings = parse_complete(mappings)?;
        self.cache.replace_shortcut_mappings(&mappings, target_platform()).map_err(cache_error)?;
        Ok(profile(mappings))
    }

    fn reset(&self) -> Result<ShortcutMappingsData, ApplicationError> {
        self.cache.reset_shortcut_mappings(target_platform()).map_err(cache_error)?;
        Ok(profile(default_shortcut_mappings()))
    }
}

pub struct InMemoryShortcutMappingsService {
    mappings: Mutex<Vec<ShortcutMapping>>,
}

impl InMemoryShortcutMappingsService {
    pub fn new() -> Self {
        Self { mappings: Mutex::new(default_shortcut_mappings()) }
    }
}

impl Default for InMemoryShortcutMappingsService {
    fn default() -> Self {
        Self::new()
    }
}

impl ShortcutMappingsService for InMemoryShortcutMappingsService {
    fn load(&self) -> Result<ShortcutMappingsData, ApplicationError> {
        Ok(profile(self.mappings.lock().expect("shortcut mappings lock poisoned").clone()))
    }

    fn save(
        &self,
        mappings: Vec<ShortcutMappingData>,
    ) -> Result<ShortcutMappingsData, ApplicationError> {
        let mappings = parse_complete(mappings)?;
        *self.mappings.lock().expect("shortcut mappings lock poisoned") = mappings.clone();
        Ok(profile(mappings))
    }

    fn reset(&self) -> Result<ShortcutMappingsData, ApplicationError> {
        let mappings = default_shortcut_mappings();
        *self.mappings.lock().expect("shortcut mappings lock poisoned") = mappings.clone();
        Ok(profile(mappings))
    }
}

fn parse_complete(
    mappings: Vec<ShortcutMappingData>,
) -> Result<Vec<ShortcutMapping>, ApplicationError> {
    let mappings = mappings
        .into_iter()
        .map(|mapping| {
            let action = ShortcutAction::from_id(&mapping.action_id).ok_or_else(|| {
                validation_error(
                    ErrorCategory::InvalidInput,
                    ShortcutServiceError::UnknownAction(mapping.action_id.clone()),
                )
            })?;
            Ok(ShortcutMapping::new(
                action,
                ShortcutChord::new(mapping.key, mapping.primary, mapping.shift, mapping.alt),
            ))
        })
        .collect::<Result<Vec<_>, ApplicationError>>()?;
    validate_complete(mappings)
}

fn validate_complete(
    mappings: Vec<ShortcutMapping>,
) -> Result<Vec<ShortcutMapping>, ApplicationError> {
    let normalized = validate_and_normalize_shortcut_mappings(&mappings, target_platform())
        .map_err(shortcut_validation_error)?;
    let actions: HashSet<_> = normalized.iter().map(|mapping| mapping.action).collect();
    let complete = ShortcutAction::ALL
        .iter()
        .copied()
        .filter(|action| action.is_available())
        .all(|action| actions.contains(&action));
    if !complete || normalized.len() != available_action_count() {
        return Err(validation_error(
            ErrorCategory::InvalidInput,
            ShortcutServiceError::IncompleteProfile,
        ));
    }
    Ok(normalized)
}

fn profile(mappings: Vec<ShortcutMapping>) -> ShortcutMappingsData {
    ShortcutMappingsData {
        mappings: mappings.iter().map(ShortcutMappingData::from).collect(),
        unavailable_action_ids: ShortcutAction::ALL
            .iter()
            .copied()
            .filter(|action| !action.is_available())
            .map(|action| action.id().to_string())
            .collect(),
    }
}

fn available_action_count() -> usize {
    ShortcutAction::ALL.iter().filter(|action| action.is_available()).count()
}

fn shortcut_validation_error(error: ShortcutError) -> ApplicationError {
    let category = match error {
        ShortcutError::DuplicateAction(_) | ShortcutError::DuplicateChord { .. } => {
            ErrorCategory::Conflict
        },
        _ => ErrorCategory::InvalidInput,
    };
    validation_error(category, error)
}

fn cache_error(error: ShortcutMappingsError) -> ApplicationError {
    validation_error(ErrorCategory::Unavailable, error)
}

fn validation_error(
    category: ErrorCategory,
    error: impl Error + Send + Sync + 'static,
) -> ApplicationError {
    ApplicationError::new(
        category,
        DiagnosticContext::new(DiagnosticCode::ShortcutPreferences),
        error,
    )
}

#[derive(Debug)]
enum ShortcutServiceError {
    UnknownAction(String),
    IncompleteProfile,
}

impl fmt::Display for ShortcutServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownAction(action) => write!(formatter, "unknown shortcut action: {action}"),
            Self::IncompleteProfile => formatter.write_str("shortcut profile is incomplete"),
        }
    }
}

impl Error for ShortcutServiceError {}

#[cfg(target_os = "macos")]
const fn target_platform() -> Platform {
    Platform::MacOs
}

#[cfg(target_os = "windows")]
const fn target_platform() -> Platform {
    Platform::Windows
}

#[cfg(target_os = "linux")]
const fn target_platform() -> Platform {
    Platform::Linux
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use pulseseek_cache::shortcut_mappings::{ShortcutMappingsCachePort, ShortcutMappingsError};
    use pulseseek_domain::error::{DiagnosticCode, ErrorCategory, ErrorContract};
    use pulseseek_domain::shortcuts::{
        default_shortcut_mappings, Platform, ShortcutAction, ShortcutMapping,
    };

    use super::{
        InMemoryShortcutMappingsService, NativeShortcutMappingsService, ShortcutMappingData,
        ShortcutMappingsService,
    };

    #[derive(Default)]
    struct RecordingCache {
        mappings: Mutex<Vec<ShortcutMapping>>,
        replacements: Mutex<Vec<Vec<ShortcutMapping>>>,
        resets: Mutex<usize>,
    }

    impl ShortcutMappingsCachePort for RecordingCache {
        fn replace_shortcut_mappings(
            &self,
            mappings: &[ShortcutMapping],
            _platform: Platform,
        ) -> Result<(), ShortcutMappingsError> {
            self.replacements.lock().unwrap().push(mappings.to_vec());
            *self.mappings.lock().unwrap() = mappings.to_vec();
            Ok(())
        }

        fn load_shortcut_mappings(&self) -> Result<Vec<ShortcutMapping>, ShortcutMappingsError> {
            Ok(self.mappings.lock().unwrap().clone())
        }

        fn reset_shortcut_mappings(
            &self,
            _platform: Platform,
        ) -> Result<(), ShortcutMappingsError> {
            *self.resets.lock().unwrap() += 1;
            *self.mappings.lock().unwrap() = default_shortcut_mappings();
            Ok(())
        }
    }

    struct BrokenCache;

    impl ShortcutMappingsCachePort for BrokenCache {
        fn replace_shortcut_mappings(
            &self,
            _mappings: &[ShortcutMapping],
            _platform: Platform,
        ) -> Result<(), ShortcutMappingsError> {
            Err(ShortcutMappingsError::WorkerStopped)
        }

        fn load_shortcut_mappings(&self) -> Result<Vec<ShortcutMapping>, ShortcutMappingsError> {
            Err(ShortcutMappingsError::WorkerStopped)
        }

        fn reset_shortcut_mappings(
            &self,
            _platform: Platform,
        ) -> Result<(), ShortcutMappingsError> {
            Err(ShortcutMappingsError::WorkerStopped)
        }
    }

    fn default_data() -> Vec<ShortcutMappingData> {
        default_shortcut_mappings().iter().map(ShortcutMappingData::from).collect()
    }

    #[test]
    fn native_load_returns_defaults_when_cache_is_empty() {
        let service = NativeShortcutMappingsService::new(Arc::new(RecordingCache::default()));

        let profile = service.load().expect("load defaults");

        assert_eq!(profile.mappings, default_data());
        assert_eq!(profile.unavailable_action_ids, Vec::<String>::new());
    }

    #[test]
    fn native_save_parses_normalizes_and_atomically_replaces_complete_profile() {
        let cache = Arc::new(RecordingCache::default());
        let service = NativeShortcutMappingsService::new(cache.clone());
        let mut mappings = default_data();
        mappings[0].key = " P ".to_string();

        let profile = service.save(mappings).expect("save profile");

        assert_eq!(profile.mappings[0].key, "p");
        let replacements = cache.replacements.lock().unwrap();
        assert_eq!(replacements.len(), 1);
        assert_eq!(replacements[0][0].chord.key, "p");
    }

    #[test]
    fn native_save_rejects_incomplete_profile_without_persisting() {
        let cache = Arc::new(RecordingCache::default());
        let service = NativeShortcutMappingsService::new(cache.clone());
        let mut mappings = default_data();
        mappings.pop();

        let error = service.save(mappings).expect_err("incomplete profile fails");

        assert_eq!(error.user_descriptor().category(), ErrorCategory::InvalidInput);
        assert!(cache.replacements.lock().unwrap().is_empty());
    }

    #[test]
    fn native_save_rejects_duplicate_ab_action_without_persisting() {
        let cache = Arc::new(RecordingCache::default());
        let service = NativeShortcutMappingsService::new(cache.clone());
        let mut mappings = default_data();
        mappings.push(ShortcutMappingData {
            action_id: ShortcutAction::SetAbStart.id().to_string(),
            key: "a".to_string(),
            primary: true,
            shift: true,
            alt: false,
        });

        let error = service.save(mappings).expect_err("duplicate action fails");

        assert_eq!(error.user_descriptor().category(), ErrorCategory::Conflict);
        assert!(cache.replacements.lock().unwrap().is_empty());
    }

    #[test]
    fn native_reset_persists_and_returns_canonical_defaults() {
        let cache = Arc::new(RecordingCache::default());
        let service = NativeShortcutMappingsService::new(cache.clone());

        let profile = service.reset().expect("reset profile");

        assert_eq!(profile.mappings, default_data());
        assert_eq!(*cache.resets.lock().unwrap(), 1);
    }

    #[test]
    fn native_cache_failure_uses_shortcut_preferences_diagnostic() {
        let service = NativeShortcutMappingsService::new(Arc::new(BrokenCache));

        let error = service.load().expect_err("cache failure");

        assert_eq!(error.user_descriptor().category(), ErrorCategory::Unavailable);
        assert_eq!(error.diagnostic_context().code(), DiagnosticCode::ShortcutPreferences.as_str());
    }

    #[test]
    fn in_memory_fallback_saves_and_resets_session_profile() {
        let service = InMemoryShortcutMappingsService::new();
        let mut mappings = default_data();
        mappings[0].key = "p".to_string();

        service.save(mappings).expect("save profile");
        assert_eq!(service.load().unwrap().mappings[0].key, "p");
        assert_eq!(service.reset().unwrap().mappings, default_data());
    }
}
