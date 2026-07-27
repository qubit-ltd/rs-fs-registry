// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Registry selection validation and SPI error conversion helpers.

use qubit_spi::ProviderSelection;
use qubit_spi::error::{
    ProviderCreationError,
    ProviderErrorKind,
    ProviderResolutionError,
    ProviderSelectionBuildError,
    RegistrationError,
};

use crate::FileSystemConfig;
use qubit_fs::{
    FsError,
    FsErrorKind,
    FsOperation,
    FsResult,
};

/// Returns the provider selection owned or implied by a configuration.
///
/// # Errors
///
/// Returns [`FsError`] when the URI scheme cannot form a provider selection.
#[inline]
pub(crate) fn selection_for_config(
    config: &FileSystemConfig,
) -> FsResult<ProviderSelection> {
    match config.selection() {
        Some(selection) => Ok(selection.clone()),
        None => ProviderSelection::named(config.uri().scheme().as_str())
            .map_err(map_provider_selection_build_error),
    }
}

/// Ensures an external selection agrees with a configuration selection.
///
/// # Errors
///
/// Returns [`FsError`] when the configuration embeds a different selection.
#[inline]
pub(crate) fn ensure_selection_matches_config(
    selection: &ProviderSelection,
    config: &FileSystemConfig,
) -> FsResult<()> {
    if let Some(config_selection) = config.selection()
        && config_selection != selection
    {
        return Err(FsError::new(
            FsErrorKind::InvalidOptions,
            FsOperation::Provider,
            "configured provider selection conflicts with requested selection",
        ));
    }
    Ok(())
}

/// Maps an SPI registration error into the filesystem error model.
#[inline]
pub(crate) fn map_registration_error(error: RegistrationError) -> FsError {
    let message = error.to_string();
    FsError::with_source(
        FsErrorKind::Conflict,
        FsOperation::Provider,
        &message,
        error,
    )
}

/// Maps an SPI selection error into a filesystem provider error.
#[inline]
pub(crate) fn map_provider_resolution_error(
    error: ProviderResolutionError,
) -> FsError {
    let message = error.to_string();
    FsError::with_source(
        FsErrorKind::ProviderUnavailable,
        FsOperation::Provider,
        &message,
        error,
    )
}

/// Maps invalid provider-selection input into the filesystem error model.
#[inline]
fn map_provider_selection_build_error(
    error: ProviderSelectionBuildError,
) -> FsError {
    let message = error.to_string();
    FsError::with_source(
        FsErrorKind::InvalidOptions,
        FsOperation::Provider,
        &message,
        error,
    )
}

/// Maps an SPI provider-creation error into the filesystem error model.
#[inline]
pub(crate) fn map_provider_creation_error(
    error: ProviderCreationError,
) -> FsError {
    let decisive_attempt = error.decisive_attempt();
    let provider = decisive_attempt.provider_id().clone();
    let kind = match decisive_attempt.error().kind() {
        ProviderErrorKind::Unsupported => FsErrorKind::RequirementNotMet,
        ProviderErrorKind::Unavailable => FsErrorKind::ProviderUnavailable,
        ProviderErrorKind::InvalidConfiguration => FsErrorKind::InvalidOptions,
        ProviderErrorKind::InitializationFailed => FsErrorKind::Other,
        _ => FsErrorKind::Other,
    };
    FsError::with_source(
        kind,
        FsOperation::Provider,
        "filesystem provider creation failed",
        error,
    )
    .with_provider(provider)
}
