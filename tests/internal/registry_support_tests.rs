// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::{
    FsErrorKind,
    FsUri,
};
use qubit_fs_registry::{
    FileSystemConfig,
    FileSystemRegistry,
};
use qubit_spi::ProviderSelection;

/// Verifies explicit selection bypasses URI-scheme selector construction.
#[test]
fn test_explicit_selection_takes_precedence_over_invalid_uri_scheme() {
    let registry = FileSystemRegistry::default();
    let config = FileSystemConfig::new(
        FsUri::parse("mock-:///resource").expect("URI should parse"),
    )
    .with_selection(
        ProviderSelection::named("missing").expect("selection should parse"),
    );

    let error = registry
        .resolve_config(&config)
        .expect_err("explicit selection should be resolved before URI scheme");

    assert_eq!(FsErrorKind::ProviderUnavailable, error.kind());
}
