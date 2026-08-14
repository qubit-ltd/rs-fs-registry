// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Runtime facade for synchronous filesystem provider factories.

use std::sync::Arc;

use qubit_fs::ConnectionUri;
use qubit_spi::ProviderDefinition;
use qubit_spi::ProviderDescriptor;
use qubit_spi::ProviderId;
use qubit_spi::ProviderRegistry;
use qubit_spi::ProviderSelection;
use qubit_spi::ResolvingServiceProvider;

use crate::FileSystemConfig;
use crate::FileSystemProvider;
use crate::FileSystemRegistryResult;
use crate::FileSystemResolution;
use crate::FileSystemSpec;
use crate::internal::ValidatingFileSystemProvider;
use crate::internal::ensure_selection_matches_config;
use crate::internal::selection_for_config;
use crate::internal::validate_credentials;

/// Shared registry of self-described synchronous filesystem providers.
///
/// Clones share the same provider catalog and default selection. Each
/// resolution captures a provider snapshot before creation begins.
#[derive(Clone, Debug, Default)]
pub struct FileSystemRegistry {
    /// Shared SPI registry storing synchronous providers and default
    /// selection.
    providers: ProviderRegistry<FileSystemSpec>,
}

impl FileSystemRegistry {
    /// Registers a provider factory owned by this registry.
    ///
    /// # Type Parameters
    ///
    /// - `P`: Concrete synchronous provider definition to register.
    ///
    /// # Parameters
    ///
    /// - `provider`: Provider definition whose ownership moves into the
    ///   registry.
    ///
    /// # Returns
    ///
    /// `Ok(())` when registration succeeds.
    ///
    /// # Errors
    ///
    /// Returns [`FileSystemRegistryError::Registration`](crate::FileSystemRegistryError::Registration)
    /// when its descriptor conflicts with an existing provider.
    ///
    /// # Panics
    ///
    /// Propagates a panic raised while obtaining the provider descriptor.
    #[inline(always)]
    pub fn register<P>(&self, provider: P) -> FileSystemRegistryResult<()>
    where
        P: ProviderDefinition<FileSystemSpec>,
    {
        let provider: Arc<FileSystemProvider> = Arc::new(provider);
        self.register_shared(provider)
    }
    /// Registers a shared provider factory.
    ///
    /// # Parameters
    ///
    /// - `provider`: Shared provider definition to register.
    ///
    /// # Returns
    ///
    /// `Ok(())` when registration succeeds.
    ///
    /// # Errors
    ///
    /// Returns [`FileSystemRegistryError::Registration`](crate::FileSystemRegistryError::Registration)
    /// when its descriptor conflicts with an existing provider.
    ///
    /// # Panics
    ///
    /// Propagates a panic raised while obtaining the provider descriptor.
    #[inline(always)]
    pub fn register_shared(
        &self,
        provider: Arc<FileSystemProvider>,
    ) -> FileSystemRegistryResult<()> {
        self.providers
            .register(ValidatingFileSystemProvider::new(provider))
            .map_err(Into::into)
    }
    /// Returns the current default selection.
    ///
    /// # Returns
    ///
    /// A snapshot of the current default provider selection.
    #[inline(always)]
    #[must_use]
    pub fn default_selection(&self) -> ProviderSelection {
        self.providers.default_selection()
    }
    /// Replaces the selection used by [`Self::resolve_default_config`].
    ///
    /// # Parameters
    ///
    /// - `selection`: Provider selection to install as the default.
    #[inline(always)]
    pub fn set_default_selection(&self, selection: ProviderSelection) {
        self.providers.set_default_selection(selection);
    }
    /// Returns descriptors in registration order.
    ///
    /// # Returns
    ///
    /// Snapshots of all registered descriptors in registration order.
    #[inline(always)]
    #[must_use]
    pub fn descriptors(&self) -> Vec<ProviderDescriptor> {
        self.providers.descriptors()
    }
    /// Returns canonical provider IDs in registration order.
    #[inline(always)]
    #[must_use]
    pub fn provider_ids(&self) -> Vec<ProviderId> {
        self.providers.provider_ids()
    }
    /// Returns the registered provider count.
    ///
    /// # Returns
    ///
    /// The number of registered providers.
    #[inline(always)]
    #[must_use]
    pub fn len(&self) -> usize {
        self.providers.len()
    }
    /// Returns whether no provider is registered.
    ///
    /// # Returns
    ///
    /// `true` when the registry contains no providers.
    #[inline(always)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
    /// Resolves `config`, preferring its explicit selection over its URI
    /// scheme.
    ///
    /// This method never falls back to the registry default selection.
    ///
    /// # Parameters
    ///
    /// - `config`: Complete provider configuration to resolve.
    ///
    /// # Returns
    ///
    /// The configured filesystem, decoded path, and canonical URI.
    ///
    /// # Errors
    ///
    /// Returns a structured error when credential sources conflict, the
    /// selection is invalid or unavailable, or provider creation fails.
    #[inline]
    pub fn resolve_config(
        &self,
        config: &FileSystemConfig,
    ) -> FileSystemRegistryResult<FileSystemResolution> {
        validate_credentials(config)?;
        let selection = selection_for_config(config)?;
        self.resolve_selected(&selection)?
            .create_configured(config)
            .map_err(Into::into)
    }
    /// Resolves a URI-only configuration through its scheme-derived selection.
    ///
    /// # Parameters
    ///
    /// - `uri`: Connection URI used to create the configuration.
    ///
    /// # Returns
    ///
    /// The configured filesystem, decoded path, and canonical URI.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::resolve_config`].
    #[inline(always)]
    pub fn resolve_uri(
        &self,
        uri: &ConnectionUri,
    ) -> FileSystemRegistryResult<FileSystemResolution> {
        self.resolve_config(&FileSystemConfig::new(uri.clone()))
    }
    /// Resolves `config` through `selection`, rejecting a conflicting embedded
    /// selection.
    ///
    /// # Parameters
    ///
    /// - `selection`: Explicit provider selection to resolve.
    /// - `config`: Complete provider configuration to resolve.
    ///
    /// # Returns
    ///
    /// The configured filesystem, decoded path, and canonical URI.
    ///
    /// # Errors
    ///
    /// Returns a structured error when credential sources or selections
    /// conflict, the selection is unavailable, or provider creation fails.
    #[inline]
    pub fn resolve_selected_config(
        &self,
        selection: &ProviderSelection,
        config: &FileSystemConfig,
    ) -> FileSystemRegistryResult<FileSystemResolution> {
        validate_credentials(config)?;
        ensure_selection_matches_config(selection, config)?;
        self.resolve_selected(selection)?
            .create_configured(config)
            .map_err(Into::into)
    }
    /// Resolves `config` through the current default selection.
    ///
    /// # Parameters
    ///
    /// - `config`: Complete provider configuration to resolve.
    ///
    /// # Returns
    ///
    /// The configured filesystem, decoded path, and canonical URI.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::resolve_selected_config`].
    #[inline(always)]
    pub fn resolve_default_config(
        &self,
        config: &FileSystemConfig,
    ) -> FileSystemRegistryResult<FileSystemResolution> {
        let selection = self.default_selection();
        self.resolve_selected_config(&selection, config)
    }

    /// Resolves a provider selection without creating it.
    ///
    /// # Parameters
    ///
    /// - `selection`: Provider selection to resolve.
    ///
    /// # Returns
    ///
    /// An owned snapshot of the selected provider chain.
    ///
    /// # Errors
    ///
    /// Returns a resolution error when the selection matches no registered
    /// provider.
    #[inline(always)]
    pub(crate) fn resolve_selected(
        &self,
        selection: &ProviderSelection,
    ) -> FileSystemRegistryResult<ResolvingServiceProvider<FileSystemSpec>>
    {
        self.providers
            .resolve_selected(selection)
            .map_err(Into::into)
    }
}
