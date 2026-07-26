// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Runtime facade for asynchronous filesystem providers.

use std::sync::Arc;

use qubit_spi::{
    AsyncProviderDefinition,
    AsyncProviderRegistry,
    AsyncResolvingServiceProvider,
    ProviderDescriptor,
    ProviderId,
    ProviderSelection,
};

use super::file_system_registry::{
    map_provider_creation_error,
    map_provider_resolution_error,
    map_provider_selection_build_error,
    map_registration_error,
};
use crate::{
    AsyncFileSystemProvider,
    FileSystemConfig,
    FileSystemResolution,
    FileSystemSpec,
};
use qubit_fs::{
    AsyncFileResource,
    AsyncFileSystem,
    FileLocation,
    FsFuture,
    FsResult,
    FsUri,
};

/// Shared asynchronous-filesystem Registry facade.
///
/// All catalog operations are synchronous. The returned futures only await
/// provider creation after the underlying SPI Registry has released its lock.
pub struct AsyncFileSystemRegistry {
    /// Typed SPI Registry shared by application and downstream consumers.
    providers: AsyncProviderRegistry<FileSystemSpec>,
}

impl AsyncFileSystemRegistry {
    /// Registers an owned asynchronous filesystem provider.
    ///
    /// # Parameters
    ///
    /// * `provider` - Provider definition moved into shared registry storage.
    ///
    /// # Errors
    ///
    /// Returns a conflict error without mutation when any provider selector is
    /// already registered.
    #[inline(always)]
    pub fn register<P>(&self, provider: P) -> FsResult<()>
    where
        P: AsyncFileSystemProvider,
    {
        self.providers
            .register(provider)
            .map_err(map_registration_error)
    }

    /// Registers an already shared asynchronous filesystem provider.
    ///
    /// # Parameters
    ///
    /// * `provider` - Shared provider definition retained by the registry.
    ///
    /// # Errors
    ///
    /// Returns a conflict error without mutation when any provider selector is
    /// already registered.
    #[inline(always)]
    pub fn register_shared(
        &self,
        provider: Arc<dyn AsyncFileSystemProvider>,
    ) -> FsResult<()> {
        let provider: Arc<dyn AsyncProviderDefinition<FileSystemSpec>> =
            provider;
        self.providers
            .register_shared(provider)
            .map_err(map_registration_error)
    }

    /// Returns the selection used by [`Self::resolve_default_config_async`].
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
    /// # Parameters
    ///
    /// * `selection` - Validated provider target and fallback policy.
    #[inline(always)]
    pub fn set_default_selection(&self, selection: ProviderSelection) {
        self.providers.set_default_selection(selection);
    }

    /// Resolves a provider selection without creating a filesystem.
    ///
    /// # Parameters
    ///
    /// * `selection` - Provider target and fallback policy.
    ///
    /// # Returns
    ///
    /// A point-in-time asynchronous provider candidate snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when the selection matches no registered provider.
    #[inline(always)]
    pub fn resolve_selected(
        &self,
        selection: &ProviderSelection,
    ) -> FsResult<AsyncResolvingServiceProvider<FileSystemSpec>> {
        self.providers
            .resolve_selected(selection)
            .map_err(map_provider_resolution_error)
    }

    /// Resolves the current default selection without creating a filesystem.
    ///
    /// # Returns
    ///
    /// A point-in-time asynchronous provider candidate snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when the default selection matches no provider.
    #[inline(always)]
    pub fn resolve(
        &self,
    ) -> FsResult<AsyncResolvingServiceProvider<FileSystemSpec>> {
        self.providers
            .resolve()
            .map_err(map_provider_resolution_error)
    }

    /// Returns provider descriptors in registration order.
    ///
    /// # Returns
    ///
    /// Owned descriptor snapshots in successful registration order.
    #[inline(always)]
    #[must_use]
    pub fn descriptors(&self) -> Vec<ProviderDescriptor> {
        self.providers.descriptors()
    }

    /// Returns the number of registered providers.
    ///
    /// # Returns
    ///
    /// The number of successful registrations.
    #[inline(always)]
    #[must_use]
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// Returns whether no provider is registered.
    ///
    /// # Returns
    ///
    /// `true` when the provider catalog is empty.
    #[inline(always)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// Resolves configuration using its explicit selection or URI scheme.
    ///
    /// # Parameters
    ///
    /// * `config` - URI, optional selection, options, and credential reference.
    ///
    /// # Returns
    ///
    /// A future yielding the configured filesystem and provider-decoded
    /// resource location.
    ///
    /// # Errors
    ///
    /// The future returns an error when selection validation, provider
    /// resolution, or asynchronous creation fails.
    pub fn resolve_config_async<'a>(
        &'a self,
        config: &'a FileSystemConfig,
    ) -> FsFuture<'a, FileSystemResolution<dyn AsyncFileSystem>> {
        let resolver = match config.selection() {
            Some(selection) => self
                .providers
                .resolve_selected(selection)
                .map_err(map_provider_resolution_error),
            None => ProviderSelection::named(config.uri().scheme().as_str())
                .map_err(map_provider_selection_build_error)
                .and_then(|selection| {
                    self.providers
                        .resolve_selected(&selection)
                        .map_err(map_provider_resolution_error)
                }),
        };
        Box::pin(async move {
            resolver?
                .create_configured(config)
                .await
                .map_err(map_provider_creation_error)
        })
    }

    /// Resolves configuration through a supplied selection.
    ///
    /// # Parameters
    ///
    /// * `selection` - Provider target and fallback policy.
    /// * `config` - Complete filesystem configuration passed to the provider.
    ///
    /// # Returns
    ///
    /// A future yielding the configured filesystem and provider-decoded
    /// resource location.
    ///
    /// # Errors
    ///
    /// The future returns an error when provider resolution or creation fails.
    #[inline]
    pub fn resolve_selected_config_async<'a>(
        &'a self,
        selection: &ProviderSelection,
        config: &'a FileSystemConfig,
    ) -> FsFuture<'a, FileSystemResolution<dyn AsyncFileSystem>> {
        let resolver = self
            .providers
            .resolve_selected(selection)
            .map_err(map_provider_resolution_error);
        Box::pin(async move {
            resolver?
                .create_configured(config)
                .await
                .map_err(map_provider_creation_error)
        })
    }

    /// Resolves configuration through the current default selection.
    ///
    /// # Parameters
    ///
    /// * `config` - Complete filesystem configuration passed to the provider.
    ///
    /// # Returns
    ///
    /// A future yielding the configured filesystem and provider-decoded
    /// resource location.
    ///
    /// # Errors
    ///
    /// The future returns an error when default provider resolution or
    /// creation fails.
    #[inline]
    pub fn resolve_default_config_async<'a>(
        &'a self,
        config: &'a FileSystemConfig,
    ) -> FsFuture<'a, FileSystemResolution<dyn AsyncFileSystem>> {
        let resolver = self
            .providers
            .resolve()
            .map_err(map_provider_resolution_error);
        Box::pin(async move {
            resolver?
                .create_configured(config)
                .await
                .map_err(map_provider_creation_error)
        })
    }

    /// Creates an asynchronous filesystem from complete configuration.
    ///
    /// # Parameters
    ///
    /// * `config` - Complete configuration used to select and create a
    ///   filesystem.
    ///
    /// # Returns
    ///
    /// A future yielding the shared asynchronous filesystem.
    ///
    /// # Errors
    ///
    /// The future returns an error when provider resolution or creation fails.
    #[inline]
    pub fn file_system_async<'a>(
        &'a self,
        config: &'a FileSystemConfig,
    ) -> FsFuture<'a, Arc<dyn AsyncFileSystem>> {
        Box::pin(async move {
            Ok(self
                .resolve_config_async(config)
                .await?
                .file_system()
                .clone())
        })
    }

    /// Resolves complete configuration into a bound asynchronous resource.
    ///
    /// # Parameters
    ///
    /// * `config` - Complete configuration used to select and create a
    ///   filesystem.
    ///
    /// # Returns
    ///
    /// A future yielding a resource bound to its provider-decoded path.
    ///
    /// # Errors
    ///
    /// The future returns an error when provider resolution or creation fails.
    #[inline]
    pub fn resource_async<'a>(
        &'a self,
        config: &'a FileSystemConfig,
    ) -> FsFuture<'a, AsyncFileResource> {
        Box::pin(async move {
            let resolution = self.resolve_config_async(config).await?;
            let (fs, path, canonical_uri) = resolution.into_parts();
            let location = FileLocation::new(fs.info().id().clone(), path)
                .with_uri(canonical_uri);
            Ok(AsyncFileResource::from_location(fs, location))
        })
    }

    /// Creates an asynchronous filesystem from URI-only configuration.
    ///
    /// # Parameters
    ///
    /// * `uri` - Resource URI used with empty options and no credentials.
    ///
    /// # Returns
    ///
    /// A future yielding the shared asynchronous filesystem.
    ///
    /// # Errors
    ///
    /// The future returns an error when provider resolution or creation fails.
    #[inline]
    pub fn file_system_uri_async<'a>(
        &'a self,
        uri: &'a FsUri,
    ) -> FsFuture<'a, Arc<dyn AsyncFileSystem>> {
        Box::pin(async move {
            self.file_system_async(&FileSystemConfig::new(uri.clone()))
                .await
        })
    }

    /// Resolves URI-only configuration into a bound asynchronous resource.
    ///
    /// # Parameters
    ///
    /// * `uri` - Resource URI used with empty options and no credentials.
    ///
    /// # Returns
    ///
    /// A future yielding a resource bound to its provider-decoded path.
    ///
    /// # Errors
    ///
    /// The future returns an error when provider resolution or creation fails.
    #[inline]
    pub fn resource_uri_async<'a>(
        &'a self,
        uri: &'a FsUri,
    ) -> FsFuture<'a, AsyncFileResource> {
        Box::pin(async move {
            self.resource_async(&FileSystemConfig::new(uri.clone()))
                .await
        })
    }

    /// Returns canonical provider IDs in registration order.
    ///
    /// # Returns
    ///
    /// Owned canonical provider IDs in successful registration order.
    #[inline(always)]
    #[must_use]
    pub fn provider_ids(&self) -> Vec<ProviderId> {
        self.providers.provider_ids()
    }
}

impl Clone for AsyncFileSystemRegistry {
    /// Clones the Registry while retaining the same shared SPI state.
    fn clone(&self) -> Self {
        Self {
            providers: self.providers.clone(),
        }
    }
}

impl Default for AsyncFileSystemRegistry {
    /// Creates an empty asynchronous-filesystem Registry.
    fn default() -> Self {
        Self {
            providers: AsyncProviderRegistry::default(),
        }
    }
}
