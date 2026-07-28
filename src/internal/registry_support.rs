// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Registry selection validation helpers.

use qubit_spi::ProviderSelection;

use crate::{
    FileSystemConfig,
    FileSystemRegistryError,
    FileSystemRegistryResult,
};

/// Returns the provider selection owned or implied by a configuration.
///
/// # Errors
///
/// Returns [`FileSystemRegistryError::Selection`] when the URI scheme cannot
/// form a provider selection.
#[inline]
pub(crate) fn selection_for_config(
    config: &FileSystemConfig,
) -> FileSystemRegistryResult<ProviderSelection> {
    match config.selection() {
        Some(selection) => Ok(selection.clone()),
        None => ProviderSelection::named(config.uri().scheme().as_str())
            .map_err(FileSystemRegistryError::from),
    }
}

/// Ensures an external selection agrees with a configuration selection.
///
/// # Errors
///
/// Returns [`FileSystemRegistryError::SelectionConflict`] when the
/// configuration embeds a different selection.
#[inline]
pub(crate) fn ensure_selection_matches_config(
    selection: &ProviderSelection,
    config: &FileSystemConfig,
) -> FileSystemRegistryResult<()> {
    if let Some(config_selection) = config.selection()
        && config_selection != selection
    {
        return Err(FileSystemRegistryError::SelectionConflict {
            requested: selection.clone(),
            configured: config_selection.clone(),
        });
    }
    Ok(())
}
