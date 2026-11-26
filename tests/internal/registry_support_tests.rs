// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::path::ConnectionUri;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemRegistry;
use qubit_fs_registry::FileSystemRegistryError;
use qubit_spi::ProviderSelection;

use crate::support::provider_fixtures::ObservedProvider;

/// A URI scheme can be valid URI syntax while failing the provider selector
/// grammar; that failure does not fall back to the registry default.
#[test]
fn test_valid_uri_scheme_outside_selector_grammar_does_not_use_default() {
    let provider = ObservedProvider::new("archive");
    let calls = provider.calls.clone();
    let registry = FileSystemRegistry::default();
    registry.register(provider).expect("register provider");
    registry
        .set_default_selection(ProviderSelection::named("archive").expect("selection"))
        .expect("set default");
    let uri = ConnectionUri::parse("archive+:///report").expect("URI should parse");
    let config = FileSystemConfig::new(uri.clone());

    for error in [
        registry.resolve_uri(&uri).expect_err("URI selection is invalid"),
        registry
            .resolve_config(&config)
            .expect_err("config selection is invalid"),
    ] {
        assert!(matches!(error, FileSystemRegistryError::Selection(_)));
        assert_eq!(error.reason_code(), "invalid_selection");
    }
    assert!(calls.lock().expect("calls").is_empty());

    let resolution = registry
        .resolve_selected_config(&ProviderSelection::named("archive").expect("selection"), &config)
        .expect("explicit valid selection should route");
    assert_eq!(resolution.file_system().properties().info().provider_id(), "archive");
    assert_eq!(calls.lock().expect("calls").len(), 1);
}
