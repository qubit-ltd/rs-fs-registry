// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Runtime facade for asynchronous filesystem provider factories.
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
use qubit_fs::ConnectionUri;
use qubit_spi::{
    AsyncProviderDefinition,
    AsyncProviderRegistry,
    AsyncResolvingServiceProvider,
    ProviderDescriptor,
    ProviderSelection,
};
use std::{
    future::Future,
    sync::Arc,
};
/// Shared registry of self-described asynchronous filesystem providers.
#[derive(Clone, Debug, Default)]
pub struct AsyncFileSystemRegistry {
    providers: AsyncProviderRegistry<FileSystemSpec>,
}
impl AsyncFileSystemRegistry {
    /// Registers an asynchronous provider factory.
    pub fn register<P>(&self, provider: P) -> FileSystemRegistryResult<()>
    where
        P: AsyncProviderDefinition<FileSystemSpec>,
    {
        let provider: Arc<AsyncFileSystemProvider> = Arc::new(provider);
        self.register_shared(provider)
    }
    /// Registers a shared asynchronous provider factory.
    pub fn register_shared(
        &self,
        provider: Arc<AsyncFileSystemProvider>,
    ) -> FileSystemRegistryResult<()> {
        self.providers
            .register(ValidatingAsyncFileSystemProvider::new(provider))
            .map_err(Into::into)
    }
    /// Returns the current default selection.
    #[must_use]
    pub fn default_selection(&self) -> ProviderSelection {
        self.providers.default_selection()
    }
    /// Replaces the default selection.
    pub fn set_default_selection(&self, selection: ProviderSelection) {
        self.providers.set_default_selection(selection);
    }
    /// Resolves a selection to an owned provider snapshot.
    pub(crate) fn resolve_selected(
        &self,
        selection: &ProviderSelection,
    ) -> FileSystemRegistryResult<AsyncResolvingServiceProvider<FileSystemSpec>>
    {
        self.providers
            .resolve_selected(selection)
            .map_err(Into::into)
    }
    /// Returns descriptors in registration order.
    #[must_use]
    pub fn descriptors(&self) -> Vec<ProviderDescriptor> {
        self.providers.descriptors()
    }
    /// Returns the registered provider count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.providers.len()
    }
    /// Returns whether no provider is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
    /// Resolves owned config without borrowing the registry while awaiting
    /// creation.
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
    /// Resolves an owned URI-only configuration.
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
    /// Resolves owned config through the default selection.
    pub fn resolve_default_config(
        &self,
        config: FileSystemConfig,
    ) -> impl Future<
        Output = FileSystemRegistryResult<AsyncFileSystemResolution>,
    > + Send
    + 'static {
        self.resolve_selected_config(self.default_selection(), config)
    }
}
