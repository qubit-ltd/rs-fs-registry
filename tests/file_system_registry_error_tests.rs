use qubit_fs::{FsError, FsErrorKind, FsOperation};
use qubit_fs_registry::FileSystemRegistryError;
use qubit_spi::ProviderSelection;
use std::error::Error;
#[test]
fn test_selection_conflict_converts_to_invalid_options() {
    let error = FileSystemRegistryError::SelectionConflict {
        requested: ProviderSelection::named("requested").expect("valid selector"),
        configured: ProviderSelection::named("configured").expect("valid selector"),
    };
    assert!(error.source().is_none());
    let fs_error: FsError = error.into();
    assert_eq!(fs_error.kind(), FsErrorKind::InvalidOptions);
    assert_eq!(fs_error.operation(), FsOperation::Provider);
}
#[test]
fn test_invalid_configuration_never_has_a_source() {
    let error = FileSystemRegistryError::InvalidConfiguration {
        message: "embedded and referenced credentials conflict",
    };
    assert!(
        error
            .to_string()
            .contains("invalid filesystem configuration")
    );
    assert!(error.source().is_none());
}

#[test]
fn test_error_display_and_debug_do_not_expose_provider_or_selection_payloads() {
    let error = FileSystemRegistryError::SelectionConflict {
        requested: ProviderSelection::named("production-secret-provider").expect("valid selector"),
        configured: ProviderSelection::named("other-secret-provider").expect("valid selector"),
    };
    for rendered in [format!("{error}"), format!("{error:?}")] {
        assert!(!rendered.contains("production-secret-provider"));
        assert!(!rendered.contains("other-secret-provider"));
    }
}
