// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Registry selection validation helpers.

use qubit_spi::ProviderSelection;

use crate::FileSystemConfig;
use crate::FileSystemRegistryError;
use crate::FileSystemRegistryResult;

/// Validates that only one credential source occupies the configuration slot.
///
/// The unredacted text is inspected only inside `ConnectionUri`'s closure and
/// is never returned, stored, or formatted.
///
/// # Parameters
///
/// - `config`: Filesystem configuration whose credential sources are checked.
///
/// # Returns
///
/// `Ok(())` when at most one credential source is configured.
///
/// # Errors
///
/// Returns [`FileSystemRegistryError::InvalidConfiguration`] when the URI
/// embeds a secret while an external credential reference is also configured.
#[inline]
pub(crate) fn validate_credentials(config: &FileSystemConfig) -> FileSystemRegistryResult<()> {
    if config.uri().has_embedded_secret() && config.credential().is_some() {
        return Err(FileSystemRegistryError::InvalidConfiguration {
            message: "embedded and referenced credentials conflict",
        });
    }
    Ok(())
}

/// Returns the provider selection owned or implied by a configuration.
///
/// # Parameters
///
/// - `config`: Filesystem configuration to inspect.
///
/// # Returns
///
/// The explicit selection, or a selection derived from the URI scheme.
///
/// # Errors
///
/// Returns [`FileSystemRegistryError::Selection`] when the URI scheme cannot
/// form a provider selection.
#[inline]
pub(crate) fn selection_for_config(config: &FileSystemConfig) -> FileSystemRegistryResult<ProviderSelection> {
    match config.selection() {
        Some(selection) => Ok(selection.clone()),
        None => ProviderSelection::named(config.uri().scheme()).map_err(FileSystemRegistryError::from),
    }
}

/// Ensures an external selection agrees with a configuration selection.
///
/// # Parameters
///
/// - `selection`: Selection requested by the caller or registry default.
/// - `config`: Configuration whose optional selection is checked.
///
/// # Returns
///
/// `Ok(())` when the configuration has no selection or the selections match.
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
