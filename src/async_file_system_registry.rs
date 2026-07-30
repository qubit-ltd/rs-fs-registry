// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Runtime facade for asynchronous filesystem provider factories.

use std::{
    future::Future,
    sync::Arc,
};

use qubit_fs::ConnectionUri;
use qubit_spi::{
    AsyncProviderDefinition,
    AsyncProviderRegistry,
    AsyncResolvingServiceProvider,
    ProviderDescriptor,
    ProviderSelection,
};

use crate::internal::{
    ValidatingAsyncFileSystemProvider,
    ensure_selection_matches_config,
    selection_for_config,
    validate_credentials,
};
use crate::{
    AsyncFileSystemProvider,
    AsyncFileSystemResolution,
    FileSystemConfig,
    FileSystemRegistryResult,
    FileSystemSpec,
};

/// Shared registry of self-described asynchronous filesystem providers.
///
/// Clones share the same provider catalog and default selection. Each
/// resolution captures a provider snapshot before returning its future.
#[derive(Clone, Debug, Default)]
pub struct AsyncFileSystemRegistry {
    /// Shared SPI registry storing asynchronous providers and default
    /// selection.
    providers: AsyncProviderRegistry<FileSystemSpec>,
}

impl AsyncFileSystemRegistry {
    /// Registers an asynchronous provider factory owned by this registry.
    ///
    /// # Type Parameters
    ///
    /// - `P`: Concrete asynchronous provider definition to register.
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
    #[inline(always)]
    pub fn register<P>(&self, provider: P) -> FileSystemRegistryResult<()>
    where
        P: AsyncProviderDefinition<FileSystemSpec>,
    {
        let provider: Arc<AsyncFileSystemProvider> = Arc::new(provider);
        self.register_shared(provider)
    }
    /// Registers a shared asynchronous provider factory.
    ///
    /// # Parameters
    ///
    /// - `provider`: Shared asynchronous provider definition to register.
    ///
    /// # Returns
    ///
    /// `Ok(())` when registration succeeds.
    ///
    /// # Errors
    ///
    /// Returns [`FileSystemRegistryError::Registration`](crate::FileSystemRegistryError::Registration)
    /// when its descriptor conflicts with an existing provider.
    #[inline(always)]
    pub fn register_shared(
        &self,
        provider: Arc<AsyncFileSystemProvider>,
    ) -> FileSystemRegistryResult<()> {
        self.providers
            .register(ValidatingAsyncFileSystemProvider::new(provider))
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
    /// Resolves owned config without borrowing the registry while awaiting
    /// creation.
    ///
    /// Validation and provider snapshotting occur before this method returns;
    /// the resulting future owns both the configuration and snapshot.
    ///
    /// # Parameters
    ///
    /// - `config`: Owned provider configuration to resolve.
    ///
    /// # Returns
    ///
    /// A `Send + 'static` future yielding the configured asynchronous
    /// filesystem, decoded path, and canonical URI.
    ///
    /// # Errors
    ///
    /// The returned future yields a structured error when credential sources
    /// conflict, the selection is invalid or unavailable, or creation fails.
    #[inline]
    pub fn resolve_config(
        &self,
        config: FileSystemConfig,
    ) -> impl Future<
        Output = FileSystemRegistryResult<AsyncFileSystemResolution>,
    > + Send
    + 'static {
        let snapshot = validate_credentials(&config)
            .and_then(|()| selection_for_config(&config))
            .and_then(|s| self.resolve_selected(&s));
        async move {
            snapshot?
                .create_configured(&config)
                .await
                .map_err(Into::into)
        }
    }
    /// Resolves an owned URI-only configuration through its scheme-derived
    /// selection.
    ///
    /// # Parameters
    ///
    /// - `uri`: Owned connection URI used to create the configuration.
    ///
    /// # Returns
    ///
    /// A `Send + 'static` resolution future.
    ///
    /// # Errors
    ///
    /// The returned future yields the same errors as [`Self::resolve_config`].
    #[inline(always)]
    pub fn resolve_uri(
        &self,
        uri: ConnectionUri,
    ) -> impl Future<
        Output = FileSystemRegistryResult<AsyncFileSystemResolution>,
    > + Send
    + 'static {
        self.resolve_config(FileSystemConfig::new(uri))
    }
    /// Resolves owned config through an explicit selection.
    ///
    /// # Parameters
    ///
    /// - `selection`: Owned provider selection to resolve.
    /// - `config`: Owned provider configuration to resolve.
    ///
    /// # Returns
    ///
    /// A `Send + 'static` resolution future.
    ///
    /// # Errors
    ///
    /// The returned future yields a structured error when credential sources
    /// or selections conflict, the selection is unavailable, or creation
    /// fails.
    #[inline]
    pub fn resolve_selected_config(
        &self,
        selection: ProviderSelection,
        config: FileSystemConfig,
    ) -> impl Future<
        Output = FileSystemRegistryResult<AsyncFileSystemResolution>,
    > + Send
    + 'static {
        let snapshot = validate_credentials(&config)
            .and_then(|()| ensure_selection_matches_config(&selection, &config))
            .and_then(|()| self.resolve_selected(&selection));
        async move {
            snapshot?
                .create_configured(&config)
                .await
                .map_err(Into::into)
        }
    }
    /// Resolves owned config through the current default selection.
    ///
    /// # Parameters
    ///
    /// - `config`: Owned provider configuration to resolve.
    ///
    /// # Returns
    ///
    /// A `Send + 'static` resolution future.
    ///
    /// # Errors
    ///
    /// The returned future yields the same errors as
    /// [`Self::resolve_selected_config`].
    #[inline(always)]
    pub fn resolve_default_config(
        &self,
        config: FileSystemConfig,
    ) -> impl Future<
        Output = FileSystemRegistryResult<AsyncFileSystemResolution>,
    > + Send
    + 'static {
        self.resolve_selected_config(self.default_selection(), config)
    }

    /// Resolves a selection to an owned provider snapshot.
    ///
    /// # Parameters
    ///
    /// - `selection`: Provider selection to resolve.
    ///
    /// # Returns
    ///
    /// An owned snapshot of the selected asynchronous provider chain.
    ///
    /// # Errors
    ///
    /// Returns a resolution error when the selection matches no registered
    /// provider.
    #[inline(always)]
    pub(crate) fn resolve_selected(
        &self,
        selection: &ProviderSelection,
    ) -> FileSystemRegistryResult<AsyncResolvingServiceProvider<FileSystemSpec>>
    {
        self.providers
            .resolve_selected(selection)
            .map_err(Into::into)
    }
}
