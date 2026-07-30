// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Runtime facade for synchronous filesystem provider factories.
use crate::internal::{
    ValidatingFileSystemProvider,
    ensure_selection_matches_config,
    selection_for_config,
    validate_credentials,
};
use crate::{
    FileSystemConfig,
    FileSystemProvider,
    FileSystemRegistryResult,
    FileSystemResolution,
    FileSystemSpec,
};
use qubit_fs::ConnectionUri;
use qubit_spi::{
    ProviderDefinition,
    ProviderDescriptor,
    ProviderRegistry,
    ProviderSelection,
    ResolvingServiceProvider,
};
use std::sync::Arc;
/// Shared registry of self-described synchronous filesystem providers.
///
/// Clones share the same provider catalog and default selection. Each
/// resolution captures a provider snapshot before creation begins.
#[derive(Clone, Debug, Default)]
pub struct FileSystemRegistry {
    providers: ProviderRegistry<FileSystemSpec>,
}
impl FileSystemRegistry {
    /// Registers a provider factory owned by this registry.
    ///
    /// # Errors
    ///
    /// Returns [`FileSystemRegistryError::Registration`](crate::FileSystemRegistryError::Registration)
    /// when its descriptor conflicts with an existing provider.
    pub fn register<P>(&self, provider: P) -> FileSystemRegistryResult<()>
    where
        P: ProviderDefinition<FileSystemSpec>,
    {
        let provider: Arc<FileSystemProvider> = Arc::new(provider);
        self.register_shared(provider)
    }
    /// Registers a shared provider factory.
    ///
    /// # Errors
    ///
    /// Returns [`FileSystemRegistryError::Registration`](crate::FileSystemRegistryError::Registration)
    /// when its descriptor conflicts with an existing provider.
    pub fn register_shared(
        &self,
        provider: Arc<FileSystemProvider>,
    ) -> FileSystemRegistryResult<()> {
        self.providers
            .register(ValidatingFileSystemProvider::new(provider))
            .map_err(Into::into)
    }
    /// Returns the current default selection.
    #[must_use]
    pub fn default_selection(&self) -> ProviderSelection {
        self.providers.default_selection()
    }
    /// Replaces the selection used by [`Self::resolve_default_config`].
    pub fn set_default_selection(&self, selection: ProviderSelection) {
        self.providers.set_default_selection(selection);
    }
    /// Resolves a provider selection without creating it.
    pub(crate) fn resolve_selected(
        &self,
        selection: &ProviderSelection,
    ) -> FileSystemRegistryResult<ResolvingServiceProvider<FileSystemSpec>>
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
    /// Resolves `config`, preferring its explicit selection over its URI
    /// scheme.
    ///
    /// This method never falls back to the registry default selection.
    ///
    /// # Errors
    ///
    /// Returns a structured error when credential sources conflict, the
    /// selection is invalid or unavailable, or provider creation fails.
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
    /// # Errors
    ///
    /// Returns the same errors as [`Self::resolve_config`].
    pub fn resolve_uri(
        &self,
        uri: &ConnectionUri,
    ) -> FileSystemRegistryResult<FileSystemResolution> {
        self.resolve_config(&FileSystemConfig::new(uri.clone()))
    }
    /// Resolves `config` through `selection`, rejecting a conflicting embedded
    /// selection.
    ///
    /// # Errors
    ///
    /// Returns a structured error when credential sources or selections
    /// conflict, the selection is unavailable, or provider creation fails.
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
    /// # Errors
    ///
    /// Returns the same errors as [`Self::resolve_selected_config`].
    pub fn resolve_default_config(
        &self,
        config: &FileSystemConfig,
    ) -> FileSystemRegistryResult<FileSystemResolution> {
        let selection = self.default_selection();
        self.resolve_selected_config(&selection, config)
    }
}
