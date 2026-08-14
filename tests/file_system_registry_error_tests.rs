// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::error::Error;

use qubit_fs::ConnectionUri;
use qubit_fs::FsError;
use qubit_fs::FsErrorKind;
use qubit_fs::FsOperation;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemRegistry;
use qubit_fs_registry::FileSystemRegistryError;
use qubit_spi::ProviderSelection;

use crate::file_system_registry_tests::FailingProvider;
/// Selection conflicts convert to filesystem invalid-options errors.
#[test]
fn test_selection_conflict_converts_to_invalid_options() {
    let error = FileSystemRegistryError::SelectionConflict {
        requested: ProviderSelection::named("requested")
            .expect("valid selector"),
        configured: ProviderSelection::named("configured")
            .expect("valid selector"),
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
    assert!(
        error
            .to_string()
            .contains("invalid filesystem configuration")
    );
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

/// Registry error formatting retains safe provider and selection context.
#[test]
fn test_error_display_and_debug_include_safe_provider_and_selection_context() {
    let error = FileSystemRegistryError::SelectionConflict {
        requested: ProviderSelection::named("production-secret-provider")
            .expect("valid selector"),
        configured: ProviderSelection::named("other-secret-provider")
            .expect("valid selector"),
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
            ConnectionUri::parse("missing:///resource")
                .expect("URI should parse"),
        ))
        .expect_err("missing provider must fail resolution");
    let selection = FileSystemRegistry::default()
        .resolve_config(&FileSystemConfig::new(
            ConnectionUri::parse("invalid-:///resource")
                .expect("URI should parse"),
        ))
        .expect_err("invalid scheme must fail selection");
    let creation = {
        let registry = FileSystemRegistry::default();
        registry
            .register(FailingProvider::new("creation"))
            .expect("register provider");
        registry
            .resolve_config(&FileSystemConfig::new(
                ConnectionUri::parse("creation:///resource")
                    .expect("URI should parse"),
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
