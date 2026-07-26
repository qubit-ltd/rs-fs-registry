// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Filesystem provider registry.

use std::sync::Arc;

use qubit_spi::error::{
    ProviderCreationError,
    ProviderErrorKind,
    ProviderResolutionError,
    ProviderSelectionBuildError,
    RegistrationError,
};
use qubit_spi::{
    ProviderDefinition,
    ProviderRegistry,
    ProviderSelection,
    ResolvingServiceProvider,
};

use crate::FileSystemResolution;
use crate::{
    FileSystemConfig,
    FileSystemProvider,
    FileSystemSpec,
};
use qubit_fs::{
    FileLocation,
    FileResource,
    FileSystem,
    FsError,
    FsErrorKind,
    FsOperation,
    FsResult,
    FsUri,
};

/// Shared runtime registry of self-described filesystem providers.
///
/// Clones observe the same registrations and default provider selection.
pub struct FileSystemRegistry {
    /// Typed SPI registry shared by application and downstream consumers.
    providers: ProviderRegistry<FileSystemSpec>,
}

impl FileSystemRegistry {
    /// Registers an owned self-described filesystem provider at runtime.
    ///
    /// # Arguments
    ///
    /// * `provider` - Provider definition moved into shared registry storage.
    ///
    /// # Returns
    ///
    /// `Ok(())` after the provider descriptor and implementation are
    /// registered.
    ///
    /// # Errors
    ///
    /// Returns [`FsError`] when the provider ID or an alias conflicts with an
    /// existing registration.
    #[inline(always)]
    pub fn register<P>(&self, provider: P) -> FsResult<()>
    where
        P: ProviderDefinition<FileSystemSpec>,
    {
        self.providers
            .register(provider)
            .map_err(map_registration_error)
    }

    /// Registers an already shared self-described filesystem provider.
    ///
    /// # Arguments
    ///
    /// * `provider` - Shared provider definition retained by the registry.
    ///
    /// # Returns
    ///
    /// `Ok(())` after the provider descriptor and implementation are
    /// registered.
    ///
    /// # Errors
    ///
    /// Returns [`FsError`] when the provider ID or an alias conflicts with an
    /// existing registration.
    #[inline(always)]
    pub fn register_shared(
        &self,
        provider: Arc<FileSystemProvider>,
    ) -> FsResult<()> {
        self.providers
            .register_shared(provider)
            .map_err(map_registration_error)
    }

    /// Returns the selection used by [`Self::resolve`].
    ///
    /// # Returns
    ///
    /// The registry's current default provider selection.
    #[inline(always)]
    #[must_use]
    pub fn default_selection(&self) -> ProviderSelection {
        self.providers.default_selection()
    }

    /// Replaces the selection used by future default resolutions.
    ///
    /// # Arguments
    ///
    /// * `selection` - Validated provider target and creation fallback policy.
    #[inline(always)]
    pub fn set_default_selection(&self, selection: ProviderSelection) {
        self.providers.set_default_selection(selection);
    }

    /// Resolves a provider selection without creating a filesystem.
    ///
    /// # Arguments
    ///
    /// * `selection` - Provider target and creation fallback policy.
    ///
    /// # Returns
    ///
    /// A composing service provider containing a point-in-time candidate
    /// snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`FsError`] when the selection matches no registered provider.
    #[inline(always)]
    pub fn resolve_selected(
        &self,
        selection: &ProviderSelection,
    ) -> FsResult<ResolvingServiceProvider<FileSystemSpec>> {
        self.providers
            .resolve_selected(selection)
            .map_err(map_provider_resolution_error)
    }

    /// Resolves the registry's current default selection without creating a
    /// filesystem.
    ///
    /// # Returns
    ///
    /// A composing service provider containing a point-in-time candidate
    /// snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`FsError`] when the default selection matches no registered
    /// provider.
    #[inline(always)]
    pub fn resolve(
        &self,
    ) -> FsResult<ResolvingServiceProvider<FileSystemSpec>> {
        self.providers
            .resolve()
            .map_err(map_provider_resolution_error)
    }

    /// Resolves a complete configuration into a provider-decoded result.
    ///
    /// # Arguments
    ///
    /// * `config` - URI, optional selection, options, and credential reference.
    ///
    /// # Returns
    ///
    /// Filesystem plus provider-decoded path and safe canonical URI.
    ///
    /// # Errors
    ///
    /// Returns [`FsError`] when provider resolution or creation fails.
    pub fn resolve_config(
        &self,
        config: &FileSystemConfig,
    ) -> FsResult<FileSystemResolution<dyn FileSystem>> {
        let selection = match config.selection() {
            Some(selection) => selection.clone(),
            None => ProviderSelection::named(config.uri().scheme().as_str())
                .map_err(map_provider_selection_build_error)?,
        };
        self.resolve_selected(&selection)?
            .create_configured(config)
            .map_err(map_provider_creation_error)
    }

    /// Creates a filesystem from the complete configuration.
    ///
    /// # Errors
    /// Returns a provider resolution or creation error.
    pub fn file_system(
        &self,
        config: &FileSystemConfig,
    ) -> FsResult<Arc<dyn FileSystem>> {
        Ok(self.resolve_config(config)?.file_system().clone())
    }

    /// Resolves a complete configuration into a bound file resource.
    ///
    /// # Arguments
    ///
    /// * `config` - URI, selection, options, and credential reference.
    ///
    /// # Returns
    ///
    /// A file resource containing the matching filesystem and filesystem-local
    /// path.
    ///
    /// # Errors
    ///
    /// Returns [`FsError`] when provider resolution or creation fails.
    #[inline]
    pub fn resource(
        &self,
        config: &FileSystemConfig,
    ) -> FsResult<FileResource> {
        let resolution = self.resolve_config(config)?;
        let (fs, path, canonical_uri) = resolution.into_parts();
        let location = FileLocation::new(fs.info().id().clone(), path)
            .with_uri(canonical_uri);
        Ok(FileResource::from_location(fs, location))
    }

    /// Creates a filesystem using empty options and no credential reference.
    ///
    /// # Errors
    /// Returns a provider resolution or creation error.
    #[inline]
    pub fn file_system_uri(
        &self,
        uri: &FsUri,
    ) -> FsResult<Arc<dyn FileSystem>> {
        self.file_system(&FileSystemConfig::new(uri.clone()))
    }

    /// Resolves a URI-only convenience configuration into a resource.
    ///
    /// # Errors
    /// Returns a provider resolution, creation, or path-decoding error.
    #[inline]
    pub fn resource_uri(&self, uri: &FsUri) -> FsResult<FileResource> {
        self.resource(&FileSystemConfig::new(uri.clone()))
    }

    /// Returns registered provider IDs in registration order.
    ///
    /// # Returns
    ///
    /// Canonical provider IDs.
    #[inline]
    #[must_use]
    pub fn provider_ids(&self) -> Vec<String> {
        self.providers
            .provider_ids()
            .into_iter()
            .map(|id| id.as_str().to_owned())
            .collect()
    }
}

impl Clone for FileSystemRegistry {
    /// Clones the registry while retaining the same shared SPI state.
    ///
    /// # Returns
    ///
    /// Another registry handle observing the same providers and default
    /// selection.
    #[inline(always)]
    fn clone(&self) -> Self {
        Self {
            providers: self.providers.clone(),
        }
    }
}

impl Default for FileSystemRegistry {
    /// Creates an empty runtime filesystem-provider registry.
    ///
    /// # Returns
    ///
    /// A registry with automatic selection as its default.
    #[inline(always)]
    fn default() -> Self {
        Self {
            providers: ProviderRegistry::default(),
        }
    }
}

/// Maps an SPI registration error into the filesystem error model.
///
/// # Arguments
///
/// * `error` - SPI selector-conflict diagnostic.
///
/// # Returns
///
/// A filesystem provider error preserving the original source.
#[inline]
pub(super) fn map_registration_error(error: RegistrationError) -> FsError {
    let message = error.to_string();
    FsError::with_source(
        FsErrorKind::Conflict,
        FsOperation::Provider,
        &message,
        error,
    )
}

/// Maps an SPI selection error into a filesystem provider error.
///
/// # Arguments
///
/// * `error` - Failure produced before any provider creates a service.
///
/// # Returns
///
/// A provider-unavailable filesystem error preserving the selection failure.
#[inline]
pub(super) fn map_provider_resolution_error(
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
///
/// # Arguments
///
/// * `error` - Failure produced while validating a provider selector.
///
/// # Returns
///
/// A provider-unavailable filesystem error preserving the validation failure.
#[inline]
pub(super) fn map_provider_selection_build_error(
    error: ProviderSelectionBuildError,
) -> FsError {
    let message = error.to_string();
    FsError::with_source(
        FsErrorKind::ProviderUnavailable,
        FsOperation::Provider,
        &message,
        error,
    )
}

/// Maps an SPI provider-creation error into the filesystem error model.
///
/// # Arguments
///
/// * `error` - Direct or aggregate provider creation failure.
///
/// # Returns
///
/// A filesystem provider error classified from the retained SPI diagnostics.
#[inline]
pub(super) fn map_provider_creation_error(
    error: ProviderCreationError,
) -> FsError {
    let decisive_attempt = error.decisive_attempt();
    let provider = decisive_attempt.provider_id().clone();
    let kind = match decisive_attempt.error().kind() {
        ProviderErrorKind::Unsupported | ProviderErrorKind::Unavailable => {
            FsErrorKind::ProviderUnavailable
        }
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
