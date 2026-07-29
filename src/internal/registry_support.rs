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
use qubit_fs::Uri;

/// Validates that only one credential source occupies the configuration slot.
///
/// The unredacted text is inspected only inside `ConnectionUri`'s closure and
/// is never returned, stored, or formatted.
pub(crate) fn validate_credentials(
    config: &FileSystemConfig,
) -> FileSystemRegistryResult<()> {
    let embedded_secret = config.uri().expose_unredacted(|raw| {
        let (before_query, query) = raw
            .split_once('?')
            .map_or((raw, None), |(head, query)| (head, Some(query)));
        let authority = before_query.split_once("://").map(|(_, rest)| {
            rest.split_once('/')
                .map_or(rest, |(authority, _)| authority)
        });
        let has_password = authority
            .and_then(|authority| {
                authority.rsplit_once('@').map(|(userinfo, _)| userinfo)
            })
            .is_some_and(|userinfo| userinfo.contains(':'));
        let has_sensitive_query = query.is_some_and(|query| {
            Uri::parse(&format!("scheme:/?{query}")).is_err()
        });
        has_password || has_sensitive_query
    });
    if embedded_secret && config.credential().is_some() {
        return Err(FileSystemRegistryError::InvalidConfiguration {
            message: "embedded and referenced credentials conflict",
        });
    }
    Ok(())
}

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
        None => {
            let scheme = config.uri().expose_unredacted(|raw| {
                raw.split_once(':')
                    .map_or("", |(scheme, _)| scheme)
                    .to_owned()
            });
            ProviderSelection::named(&scheme)
                .map_err(FileSystemRegistryError::from)
        }
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
