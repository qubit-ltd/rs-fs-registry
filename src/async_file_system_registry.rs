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
    /// # Errors
    ///
    /// Returns a conflict error without mutation when any provider selector is
    /// already registered.
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
    /// # Errors
    ///
    /// Returns a conflict error without mutation when any provider selector is
    /// already registered.
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

    /// Returns the selection used by [`Self::resolve_async`].
    #[must_use]
    pub fn default_selection(&self) -> ProviderSelection {
        self.providers.default_selection()
    }

    /// Replaces the selection used by future default resolutions.
    pub fn set_default_selection(&self, selection: ProviderSelection) {
        self.providers.set_default_selection(selection);
    }

    /// Resolves configuration using its explicit selection or URI scheme.
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
    pub fn resolve_selected_async<'a>(
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
    pub fn resolve_async<'a>(
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
    #[must_use]
    pub fn provider_ids(&self) -> Vec<String> {
        self.providers
            .provider_ids()
            .into_iter()
            .map(|id| id.as_str().to_owned())
            .collect()
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
