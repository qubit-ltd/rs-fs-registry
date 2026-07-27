// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Filesystem provider registry.

use std::sync::Arc;

use qubit_spi::{
    ProviderDefinition, ProviderDescriptor, ProviderId, ProviderRegistry, ProviderSelection,
    ResolvingServiceProvider,
};

use crate::internal::{ensure_selection_matches_config, selection_for_config};
use crate::{
    FileSystemConfig, FileSystemProvider, FileSystemRegistryError, FileSystemRegistryResult,
    FileSystemResolution, FileSystemSpec,
};
use qubit_fs::{FileResource, FileSystem, FsUri};

/// Shared runtime registry of self-described filesystem providers.
///
/// Clones observe the same registrations and default provider selection.
#[derive(Debug)]
pub struct FileSystemRegistry {
    /// Typed SPI registry shared by application and downstream consumers.
    providers: ProviderRegistry<FileSystemSpec>,
}

impl FileSystemRegistry {
    /// Registers an owned self-described filesystem provider at runtime.
    ///
    /// # Parameters
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
    /// Returns [`FileSystemRegistryError::Registration`] when the provider ID
    /// or an alias conflicts with an existing registration.
    #[inline(always)]
    pub fn register<P>(&self, provider: P) -> FileSystemRegistryResult<()>
    where
        P: ProviderDefinition<FileSystemSpec>,
    {
        self.providers
            .register(provider)
            .map_err(FileSystemRegistryError::from)
    }

    /// Registers an already shared self-described filesystem provider.
    ///
    /// # Parameters
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
    /// Returns [`FileSystemRegistryError::Registration`] when the provider ID
    /// or an alias conflicts with an existing registration.
    #[inline(always)]
    pub fn register_shared(
        &self,
        provider: Arc<FileSystemProvider>,
    ) -> FileSystemRegistryResult<()> {
        self.providers
            .register_shared(provider)
            .map_err(FileSystemRegistryError::from)
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
    /// # Parameters
    ///
    /// * `selection` - Validated provider target and creation fallback policy.
    #[inline(always)]
    pub fn set_default_selection(&self, selection: ProviderSelection) {
        self.providers.set_default_selection(selection);
    }

    /// Resolves a provider selection without creating a filesystem.
    ///
    /// # Parameters
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
    /// Returns [`FileSystemRegistryError::Resolution`] when the selection
    /// matches no registered provider.
    #[inline(always)]
    pub fn resolve_selected(
        &self,
        selection: &ProviderSelection,
    ) -> FileSystemRegistryResult<ResolvingServiceProvider<FileSystemSpec>> {
        self.providers
            .resolve_selected(selection)
            .map_err(FileSystemRegistryError::from)
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
    /// Returns [`FileSystemRegistryError::Resolution`] when the default
    /// selection matches no registered provider.
    #[inline(always)]
    pub fn resolve(&self) -> FileSystemRegistryResult<ResolvingServiceProvider<FileSystemSpec>> {
        self.providers
            .resolve()
            .map_err(FileSystemRegistryError::from)
    }

    /// Returns provider descriptors in registration order.
    #[inline(always)]
    #[must_use]
    pub fn descriptors(&self) -> Vec<ProviderDescriptor> {
        self.providers.descriptors()
    }

    /// Returns the number of registered providers.
    #[inline(always)]
    #[must_use]
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// Returns whether no provider is registered.
    #[inline(always)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// Resolves a complete configuration into a provider-decoded result.
    ///
    /// Uses the configuration's explicit selection when present; otherwise it
    /// derives a named selection from the URI scheme. This does not use the
    /// registry default selection.
    ///
    /// # Parameters
    ///
    /// * `config` - URI, optional selection, options, and credential reference.
    ///
    /// # Returns
    ///
    /// Filesystem plus provider-decoded path and safe canonical URI.
    ///
    /// # Errors
    ///
    /// Returns [`FileSystemRegistryError::Selection`] when the URI scheme
    /// cannot form a provider selector, [`FileSystemRegistryError::Resolution`]
    /// when no provider matches, or [`FileSystemRegistryError::Creation`] when
    /// provider creation terminates without a filesystem.
    pub fn resolve_config(
        &self,
        config: &FileSystemConfig,
    ) -> FileSystemRegistryResult<FileSystemResolution<dyn FileSystem>> {
        let resolver =
            selection_for_config(config).and_then(|selection| self.resolve_selected(&selection));
        resolver?
            .create_configured(config)
            .map_err(FileSystemRegistryError::from)
    }

    /// Resolves configuration through a supplied provider selection.
    ///
    /// # Parameters
    ///
    /// * `selection` - Provider target and creation fallback policy.
    /// * `config` - Complete filesystem configuration passed to the provider.
    ///
    /// # Returns
    ///
    /// The configured filesystem and its provider-decoded resource location.
    ///
    /// # Errors
    ///
    /// Returns [`FileSystemRegistryError::SelectionConflict`] when the
    /// configuration selection conflicts with `selection`,
    /// [`FileSystemRegistryError::Resolution`] when no provider matches, or
    /// [`FileSystemRegistryError::Creation`] when provider creation terminates
    /// without a filesystem.
    #[inline]
    pub fn resolve_selected_config(
        &self,
        selection: &ProviderSelection,
        config: &FileSystemConfig,
    ) -> FileSystemRegistryResult<FileSystemResolution<dyn FileSystem>> {
        ensure_selection_matches_config(selection, config)?;
        self.resolve_selected(selection)?
            .create_configured(config)
            .map_err(FileSystemRegistryError::from)
    }

    /// Resolves configuration through the current default provider selection.
    ///
    /// # Parameters
    ///
    /// * `config` - Complete filesystem configuration passed to the provider.
    ///
    /// # Returns
    ///
    /// The configured filesystem and its provider-decoded resource location.
    ///
    /// # Errors
    ///
    /// Returns [`FileSystemRegistryError::SelectionConflict`] when the
    /// configuration selection conflicts with the default selection,
    /// [`FileSystemRegistryError::Resolution`] when no provider matches, or
    /// [`FileSystemRegistryError::Creation`] when provider creation terminates
    /// without a filesystem.
    #[inline]
    pub fn resolve_default_config(
        &self,
        config: &FileSystemConfig,
    ) -> FileSystemRegistryResult<FileSystemResolution<dyn FileSystem>> {
        let selection = self.default_selection();
        self.resolve_selected_config(&selection, config)
    }

    /// Creates a filesystem from the complete configuration.
    ///
    /// Selection follows [`Self::resolve_config`].
    ///
    /// # Parameters
    ///
    /// * `config` - Complete filesystem configuration used for selection and
    ///   provider creation.
    ///
    /// # Returns
    ///
    /// Shared configured filesystem.
    ///
    /// # Errors
    /// Returns the same [`FileSystemRegistryError`] variants as
    /// [`Self::resolve_config`].
    pub fn file_system(
        &self,
        config: &FileSystemConfig,
    ) -> FileSystemRegistryResult<Arc<dyn FileSystem>> {
        let resolution = self.resolve_config(config)?;
        let (file_system, _, _) = resolution.into_parts();
        Ok(file_system)
    }

    /// Resolves a complete configuration into a bound file resource.
    ///
    /// Selection follows [`Self::resolve_config`].
    ///
    /// # Parameters
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
    /// Returns the same [`FileSystemRegistryError`] variants as
    /// [`Self::resolve_config`].
    #[inline]
    pub fn resource(&self, config: &FileSystemConfig) -> FileSystemRegistryResult<FileResource> {
        let resolution = self.resolve_config(config)?;
        let (fs, path, canonical_uri) = resolution.into_parts();
        Ok(FileResource::from_resolved(fs, path, canonical_uri))
    }

    /// Creates a filesystem using empty options and no credential reference.
    ///
    /// Uses a named selection derived from the URI scheme.
    ///
    /// # Parameters
    ///
    /// * `uri` - Resource URI used for provider selection.
    ///
    /// # Returns
    ///
    /// Shared configured filesystem.
    ///
    /// # Errors
    /// Returns the same [`FileSystemRegistryError`] variants as
    /// [`Self::resolve_config`].
    #[inline]
    pub fn file_system_uri(&self, uri: &FsUri) -> FileSystemRegistryResult<Arc<dyn FileSystem>> {
        self.file_system(&FileSystemConfig::new(uri.clone()))
    }

    /// Resolves a URI-only convenience configuration into a resource.
    ///
    /// Uses a named selection derived from the URI scheme.
    ///
    /// # Parameters
    ///
    /// * `uri` - Resource URI used for provider selection.
    ///
    /// # Returns
    ///
    /// Bound resource with its provider-decoded path and canonical URI.
    ///
    /// # Errors
    /// Returns the same [`FileSystemRegistryError`] variants as
    /// [`Self::resolve_config`].
    #[inline]
    pub fn resource_uri(&self, uri: &FsUri) -> FileSystemRegistryResult<FileResource> {
        self.resource(&FileSystemConfig::new(uri.clone()))
    }

    /// Returns registered provider IDs in registration order.
    ///
    /// # Returns
    ///
    /// Canonical provider IDs.
    #[inline(always)]
    #[must_use]
    pub fn provider_ids(&self) -> Vec<ProviderId> {
        self.providers.provider_ids()
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
