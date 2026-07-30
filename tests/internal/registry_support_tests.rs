// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::ConnectionUri;
use qubit_fs_registry::{
    FileSystemConfig,
    FileSystemRegistry,
    FileSystemRegistryError,
};
/// Invalid URI schemes fail instead of falling back to the default provider.
#[test]
fn test_invalid_uri_scheme_is_rejected_without_default_fallback() {
    let config = FileSystemConfig::new(
        ConnectionUri::parse("invalid-:///resource").expect("URI should parse"),
    );
    let error = FileSystemRegistry::default()
        .resolve_config(&config)
        .expect_err("invalid scheme should not use default");
    assert!(matches!(error, FileSystemRegistryError::Selection(_)));
}
