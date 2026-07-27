// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Error model for filesystem provider registry operations.

use std::{
    error::Error,
    fmt,
};

use qubit_fs::FsError;
use qubit_spi::error::{
    ProviderCreationError,
    ProviderResolutionError,
    ProviderSelectionBuildError,
    RegistrationError,
};
use qubit_spi::ProviderSelection;

/// Error returned by filesystem-provider registration, selection, and creation.
#[derive(Debug)]
#[non_exhaustive]
pub enum FileSystemRegistryError {
    /// A provider descriptor could not be registered.
    Registration(RegistrationError),
    /// A provider selection could not be constructed from configuration.
    Selection(ProviderSelectionBuildError),
    /// A caller-supplied selection conflicts with the configuration selection.
    SelectionConflict {
        /// Selection requested by the caller or registry default.
        requested: ProviderSelection,
        /// Different selection embedded in the configuration.
        configured: ProviderSelection,
    },
    /// A selection did not resolve to registered providers.
    Resolution(ProviderResolutionError),
    /// Every selected provider failed to create a filesystem.
    Creation(ProviderCreationError<FsError>),
}

impl fmt::Display for FileSystemRegistryError {
    /// Formats the registry failure with its preserved SPI context.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Registration(error) => write!(formatter, "provider registration failed: {error}"),
            Self::Selection(error) => write!(formatter, "provider selection is invalid: {error}"),
            Self::SelectionConflict { requested, configured } => write!(
                formatter,
                "configured provider selection {configured:?} conflicts with requested selection {requested:?}",
            ),
            Self::Resolution(error) => write!(formatter, "provider resolution failed: {error}"),
            Self::Creation(error) => write!(formatter, "filesystem provider creation failed: {error}"),
        }
    }
}

impl Error for FileSystemRegistryError {
    /// Returns the underlying SPI error when one exists.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Registration(error) => Some(error),
            Self::Selection(error) => Some(error),
            Self::SelectionConflict { .. } => None,
            Self::Resolution(error) => Some(error),
            Self::Creation(error) => Some(error),
        }
    }
}

impl From<RegistrationError> for FileSystemRegistryError {
    /// Wraps an SPI registration failure without losing its type.
    fn from(error: RegistrationError) -> Self {
        Self::Registration(error)
    }
}

impl From<ProviderSelectionBuildError> for FileSystemRegistryError {
    /// Wraps an SPI selection-construction failure without losing its type.
    fn from(error: ProviderSelectionBuildError) -> Self {
        Self::Selection(error)
    }
}

impl From<ProviderResolutionError> for FileSystemRegistryError {
    /// Wraps an SPI resolution failure without losing its type.
    fn from(error: ProviderResolutionError) -> Self {
        Self::Resolution(error)
    }
}

impl From<ProviderCreationError<FsError>> for FileSystemRegistryError {
    /// Wraps the typed provider-creation aggregate without losing leaf errors.
    fn from(error: ProviderCreationError<FsError>) -> Self {
        Self::Creation(error)
    }
}
