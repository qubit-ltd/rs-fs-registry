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
/// A URI scheme can be valid URI syntax while failing the provider selector
/// grammar; that failure does not fall back to the registry default.
#[test]
fn test_valid_uri_scheme_outside_selector_grammar_does_not_use_default() {
    let config = FileSystemConfig::new(
        ConnectionUri::parse("invalid-:///resource").expect("URI should parse"),
    );
    let error = FileSystemRegistry::default()
        .resolve_config(&config)
        .expect_err("an invalid selector should not use the default");
    assert!(matches!(error, FileSystemRegistryError::Selection(_)));
}
