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
/// A URI scheme derives a named provider selector and does not fall back to
/// the registry default when that provider is unavailable.
#[test]
fn test_uri_scheme_selector_is_resolved_without_default_fallback() {
    let config = FileSystemConfig::new(
        ConnectionUri::parse("unregistered-scheme:///resource").expect("URI should parse"),
    );
    let error = FileSystemRegistry::default()
        .resolve_config(&config)
        .expect_err("an unregistered scheme should not use the default");
    assert!(matches!(error, FileSystemRegistryError::Resolution(_)));
}
