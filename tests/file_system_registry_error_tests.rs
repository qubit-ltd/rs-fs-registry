// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::error::Error;

use qubit_fs_registry::FileSystemRegistryError;
use qubit_spi::ProviderSelection;

#[test]
fn test_selection_conflict_is_contextual_without_a_source() {
    let requested = ProviderSelection::named("memory").expect("requested selection is valid");
    let configured = ProviderSelection::named("local").expect("configured selection is valid");
    let error = FileSystemRegistryError::SelectionConflict {
        requested,
        configured,
    };

    assert!(error.to_string().contains("conflicts"));
    assert!(error.source().is_none());
}
