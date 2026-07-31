// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Synchronous provider wrapper enforcing filesystem-specific invariants.

use std::sync::Arc;

use qubit_fs::FsError;
use qubit_spi::error::ProviderFailure;
use qubit_spi::{
    ProviderDescriptor,
    ProviderMetadata,
    ServiceProvider,
};

use super::provider_adapter::validate_sync_resolution;
use crate::{
    FileSystemConfig,
    FileSystemProvider,
    FileSystemResolution,
    FileSystemSpec,
};

/// Synchronous provider wrapper that binds successful output to its descriptor.
pub(crate) struct ValidatingFileSystemProvider {
    /// Descriptor captured at registration time.
    descriptor: ProviderDescriptor,
    /// Provider whose resolutions are validated.
    provider: Arc<FileSystemProvider>,
}

impl ValidatingFileSystemProvider {
    /// Captures the descriptor used for registration and later validation.
    ///
    /// # Parameters
    ///
    /// - `provider`: Shared provider to wrap.
    ///
    /// # Returns
    ///
    /// A validating wrapper bound to the provider's current descriptor.
    ///
    /// # Panics
    ///
    /// Propagates a panic raised while obtaining the provider descriptor.
    #[inline]
    pub(crate) fn new(provider: Arc<FileSystemProvider>) -> Self {
        let descriptor = provider.descriptor();
        Self {
            descriptor,
            provider,
        }
    }
}

impl ProviderMetadata for ValidatingFileSystemProvider {
    /// Returns the descriptor captured before registration.
    ///
    /// # Returns
    ///
    /// A clone of the captured provider descriptor.
    #[inline(always)]
    fn descriptor(&self) -> ProviderDescriptor {
        self.descriptor.clone()
    }
}

impl ServiceProvider<FileSystemSpec> for ValidatingFileSystemProvider {
    /// Delegates creation and validates the resulting filesystem identity.
    ///
    /// # Parameters
    ///
    /// - `config`: Complete filesystem configuration passed to the provider.
    ///
    /// # Returns
    ///
    /// The provider resolution when its identity matches the descriptor.
    ///
    /// # Errors
    ///
    /// Returns the provider's creation failure or a contract violation for a
    /// mismatched filesystem identity.
    #[inline]
    fn create_configured(
        &self,
        config: &FileSystemConfig,
    ) -> Result<FileSystemResolution, ProviderFailure<FsError>> {
        let resolution = self.provider.create_configured(config)?;
        validate_sync_resolution(&self.descriptor, resolution)
    }
}
