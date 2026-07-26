use std::{error::Error, fmt, path::PathBuf};

use pulseseek_domain::error::{
    ApplicationError, DiagnosticCode, DiagnosticContext, ErrorCategory, ErrorContract,
};

#[derive(Debug)]
struct FakeAdapterError {
    private_path: PathBuf,
}

impl fmt::Display for FakeAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "failed to read {}", self.private_path.display())
    }
}

impl Error for FakeAdapterError {}

trait FakeFilesystemPort {
    type Error: Error;

    fn read(&self) -> Result<(), Self::Error>;
}

struct FailingFilesystem {
    private_path: PathBuf,
}

impl FakeFilesystemPort for FailingFilesystem {
    type Error = FakeAdapterError;

    fn read(&self) -> Result<(), Self::Error> {
        Err(FakeAdapterError { private_path: self.private_path.clone() })
    }
}

fn browse(
    port: &impl FakeFilesystemPort<Error = FakeAdapterError>,
) -> Result<(), ApplicationError> {
    port.read().map_err(|source| {
        ApplicationError::new(
            ErrorCategory::PermissionDenied,
            DiagnosticContext::new(DiagnosticCode::BrowserRead),
            source,
        )
    })
}

#[test]
fn application_error_maps_adapter_failure_to_safe_contract() {
    let private_path = PathBuf::from("/Users/álîçé/秘密 Sessions/unreleased mix.wav");
    let filesystem = FailingFilesystem { private_path: private_path.clone() };

    let error = browse(&filesystem).expect_err("browsing should fail");
    let descriptor = error.user_descriptor();
    let context = error.diagnostic_context();

    assert_eq!(descriptor.category(), ErrorCategory::PermissionDenied);
    assert_eq!(descriptor.message(), "PulseSeek could not access that item.");
    assert_eq!(context.code(), "browser.read");

    let safe_output =
        format!("{error} {error:?} {descriptor} {descriptor:?} {context} {context:?}");
    assert!(!safe_output.contains(private_path.to_string_lossy().as_ref()));
    assert!(!safe_output.contains("unreleased mix.wav"));
    assert!(!safe_output.contains("álîçé"));
    assert!(!safe_output.contains("秘密 Sessions"));

    let diagnostic_source = error.source().expect("source should be retained").to_string();
    assert!(diagnostic_source.contains(private_path.to_string_lossy().as_ref()));
}

#[test]
fn diagnostic_codes_are_stable_and_operation_specific() {
    assert_eq!(DiagnosticContext::new(DiagnosticCode::BrowserRead).code(), "browser.read");
}
