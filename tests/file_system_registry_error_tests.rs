// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::error::Error;

use qubit_fs::FsError;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::path::ConnectionUri;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemRegistry;
use qubit_fs_registry::FileSystemRegistryError;
use qubit_spi::FallbackPolicy;
use qubit_spi::ProviderDescriptor;
use qubit_spi::ProviderId;
use qubit_spi::ProviderMetadata;
use qubit_spi::ProviderSelection;
use qubit_spi::ServiceProvider;
use qubit_spi::error::ProviderCreationError;
use qubit_spi::error::ProviderFailure;
use qubit_spi::error::ProviderFailureKind;

use crate::file_system_registry_tests::FailingProvider;
/// Selection conflicts convert to filesystem invalid-options errors.
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
/// Invalid registry configurations do not expose an underlying source error.
#[test]
fn test_invalid_configuration_never_has_a_source() {
    let error = FileSystemRegistryError::InvalidConfiguration {
        message: "embedded and referenced credentials conflict",
    };
    assert!(error.to_string().contains("code=invalid_configuration"));
    assert!(error.source().is_none());
    let fs_error: FsError = error.into();
    assert_eq!(fs_error.kind(), FsErrorKind::InvalidOptions);
    assert_eq!(fs_error.operation(), FsOperation::Provider);
}

/// Invalid configuration diagnostics mask embedded credential-like text.
#[test]
fn test_invalid_configuration_display_does_not_expose_embedded_secret() {
    let error = FileSystemRegistryError::InvalidConfiguration {
        message: "connection failed for token=raw-secret-value",
    };

    let display = error.to_string();
    assert!(!display.contains("raw-secret-value"));
    assert!(display.contains("<redacted>"));
    let debug = format!("{error:?}");
    assert!(!debug.contains("raw-secret-value"));
    assert!(debug.contains("<redacted>"));
}

/// Credential source conflicts expose a stable reason code and only safe
/// structured diagnostic text.
#[test]
fn test_credential_source_conflict_has_safe_reason_code() {
    let error = FileSystemRegistryError::CredentialSourceConflict;

    assert_eq!(error.reason_code(), "credential_source_conflict");
    assert!(error.to_string().contains("code=credential_source_conflict"));
    assert!(error.source().is_none());
}

/// Typed registry errors expose stable category codes independent of text.
#[test]
fn test_registry_error_reason_codes_preserve_typed_categories() {
    let registration = {
        let registry = FileSystemRegistry::default();
        registry
            .register(FailingProvider::new("duplicate"))
            .expect("register first provider");
        registry
            .register(FailingProvider::new("duplicate"))
            .expect_err("duplicate provider must fail")
    };
    let resolution = FileSystemRegistry::default()
        .resolve_config(&FileSystemConfig::new(
            ConnectionUri::parse("missing:///resource").expect("URI should parse"),
        ))
        .expect_err("missing provider must fail resolution");
    let selection = FileSystemRegistry::default()
        .resolve_config(&FileSystemConfig::new(
            ConnectionUri::parse("invalid-:///resource").expect("URI should parse"),
        ))
        .expect_err("invalid selector must fail selection");
    let no_candidates = FileSystemRegistry::default()
        .resolve_selected_config(
            &ProviderSelection::chain_allowing_missing(["missing"]).expect("selector should parse"),
            &FileSystemConfig::new(ConnectionUri::parse("file:///resource").expect("URI should parse")),
        )
        .expect_err("lenient missing provider must have no candidates");
    let empty_registry = FileSystemRegistry::default()
        .resolve_selected_config(
            &ProviderSelection::auto(),
            &FileSystemConfig::new(ConnectionUri::parse("file:///resource").expect("URI should parse")),
        )
        .expect_err("automatic selection from an empty registry must fail");
    let selection_conflict = FileSystemRegistryError::SelectionConflict {
        requested: ProviderSelection::named("requested").expect("valid selector"),
        configured: ProviderSelection::named("configured").expect("valid selector"),
    };

    assert_eq!(registration.reason_code(), "registration_conflict");
    assert_eq!(resolution.reason_code(), "unknown_providers");
    assert_eq!(selection.reason_code(), "invalid_selection");
    assert_eq!(no_candidates.reason_code(), "no_candidates");
    assert_eq!(empty_registry.reason_code(), "empty_registry");
    assert_eq!(selection_conflict.reason_code(), "selection_conflict");
}

/// Registry error formatting retains safe provider and selection context.
#[test]
fn test_error_display_and_debug_include_safe_provider_and_selection_context() {
    let error = FileSystemRegistryError::SelectionConflict {
        requested: ProviderSelection::named("production-secret-provider").expect("valid selector"),
        configured: ProviderSelection::named("other-secret-provider").expect("valid selector"),
    };
    let display = format!("{error}");
    assert!(display.contains("production-secret-provider"));
    assert!(display.contains("other-secret-provider"));
    let debug = format!("{error:?}");
    assert!(debug.contains("production-secret-provider"));
    assert!(debug.contains("other-secret-provider"));
}

/// Registration, resolution, and selection failures retain sources while
/// converting to provider-neutral filesystem errors.
#[test]
fn test_registry_error_variants_format_and_convert_safely() {
    let registration = {
        let registry = FileSystemRegistry::default();
        registry
            .register(FailingProvider::new("duplicate"))
            .expect("register first provider");
        registry
            .register(FailingProvider::new("duplicate"))
            .expect_err("duplicate provider must fail")
    };
    let resolution = FileSystemRegistry::default()
        .resolve_config(&FileSystemConfig::new(
            ConnectionUri::parse("missing:///resource").expect("URI should parse"),
        ))
        .expect_err("missing provider must fail resolution");
    let selection = FileSystemRegistry::default()
        .resolve_config(&FileSystemConfig::new(
            ConnectionUri::parse("invalid-:///resource").expect("URI should parse"),
        ))
        .expect_err("invalid scheme must fail selection");
    let creation = {
        let registry = FileSystemRegistry::default();
        registry
            .register(FailingProvider::new("creation"))
            .expect("register provider");
        registry
            .resolve_config(&FileSystemConfig::new(
                ConnectionUri::parse("creation:///resource").expect("URI should parse"),
            ))
            .expect_err("unavailable provider must fail creation")
    };

    for (error, expected_kind, expected_provider) in [
        (registration, FsErrorKind::Conflict, None),
        (resolution, FsErrorKind::ProviderUnavailable, None),
        (selection, FsErrorKind::InvalidUri, None),
        (creation, FsErrorKind::ProviderUnavailable, Some("creation")),
    ] {
        assert!(error.source().is_some());
        assert!(!format!("{error}").is_empty());
        assert!(!format!("{error:?}").is_empty());
        let fs_error: FsError = error.into();
        assert_eq!(fs_error.kind(), expected_kind);
        assert_eq!(fs_error.operation(), FsOperation::Provider);
        assert_eq!(fs_error.provider(), expected_provider);
        assert!(std::error::Error::source(&fs_error).is_some());
    }
}

/// Resolution diagnostics expose bounded selector context without raw error
/// formatting that could include untrusted input.
#[test]
fn test_resolution_display_is_bounded_and_structured() {
    let selection = ProviderSelection::chain([
        "missing-1",
        "missing-2",
        "missing-3",
        "missing-4",
        "missing-5",
        "missing-6",
        "missing-7",
        "missing-8",
        "missing-9",
    ])
    .expect("selectors should be valid");
    let error = FileSystemRegistry::default()
        .resolve_selected_config(
            &selection,
            &FileSystemConfig::new(ConnectionUri::parse("file:///resource").expect("URI should parse")),
        )
        .expect_err("missing providers must fail resolution");

    assert_eq!(error.reason_code(), "unknown_providers");
    let display = error.to_string();
    assert!(display.contains("code=unknown_providers"));
    assert!(display.contains("selector_count=9"));
    assert!(display.contains("selector=missing-1"));
    assert!(display.contains("selector=missing-8"));
    assert!(!display.contains("selector=missing-9"));
}

/// Creation diagnostics contain aggregate metadata and leaf categories while
/// omitting the FsError message and source chain from ordinary formatting.
#[test]
fn test_creation_display_is_structured_without_leaf_message() {
    let registry = FileSystemRegistry::default();
    registry
        .register(InvalidConfigurationProvider::new("first"))
        .expect("register first provider");
    registry
        .register(FailingProvider::new("second"))
        .expect("register second provider");
    let selection = ProviderSelection::chain(["first", "second"])
        .expect("selection should parse")
        .with_fallback_policy(FallbackPolicy::OnAnyError);
    let error = registry
        .resolve_selected_config(
            &selection,
            &FileSystemConfig::new(ConnectionUri::parse("first:///resource").expect("URI should parse")),
        )
        .expect_err("provider must fail creation");

    assert_eq!(error.reason_code(), "provider_creation_failed");
    let display = error.to_string();
    assert!(display.contains("code=provider_creation_failed"));
    assert!(display.contains("attempt_count=2"));
    assert!(display.contains("provider=second"));
    assert!(display.contains("failure_kind="));
    assert!(display.contains("fs_error_kind="));
    assert!(!display.contains("token=raw-secret"));
    let debug = format!("{error:?}");
    assert!(debug.contains("code=provider_creation_failed"));
    assert!(debug.contains("attempt_count=2"));
    assert!(!debug.contains("token=raw-secret"));

    let creation = error
        .source()
        .expect("creation error should retain source")
        .downcast_ref::<ProviderCreationError<FsError>>()
        .expect("source should retain typed creation aggregate");
    assert_eq!(creation.attempts().len(), 2);
    let mut attempts = creation.attempts().iter();
    assert_eq!(
        attempts
            .next()
            .expect("first attempt should be retained")
            .failure()
            .kind(),
        ProviderFailureKind::InvalidConfiguration,
    );
    assert_eq!(
        attempts
            .next()
            .expect("second attempt should be retained")
            .failure()
            .kind(),
        ProviderFailureKind::Unavailable,
    );
}

struct InvalidConfigurationProvider {
    id: &'static str,
}

impl InvalidConfigurationProvider {
    fn new(id: &'static str) -> Self {
        Self { id }
    }
}

impl ProviderMetadata for InvalidConfigurationProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(ProviderId::new(self.id).expect("provider id"))
    }
}

impl ServiceProvider<qubit_fs_registry::FileSystemSpec> for InvalidConfigurationProvider {
    fn create_configured(
        &self,
        _: &FileSystemConfig,
    ) -> Result<qubit_fs_registry::FileSystemResolution, ProviderFailure<FsError>> {
        Err(ProviderFailure::invalid_configuration(FsError::new(
            FsErrorKind::InvalidOptions,
            FsOperation::Provider,
            "token=raw-secret",
        )))
    }
}
