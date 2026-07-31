// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Asynchronous provider wrapper enforcing filesystem-specific invariants.

use std::sync::Arc;

use qubit_fs::FsError;
use qubit_spi::error::ProviderFailure;
use qubit_spi::{
    AsyncServiceProvider,
    ProviderDescriptor,
    ProviderFuture,
    ProviderMetadata,
};

use super::provider_adapter::validate_async_resolution;
use crate::{
    AsyncFileSystemProvider,
    AsyncFileSystemResolution,
    FileSystemConfig,
    FileSystemSpec,
};

/// Asynchronous provider wrapper that binds successful output to its
/// descriptor.
pub(crate) struct ValidatingAsyncFileSystemProvider {
    /// Descriptor captured at registration time.
    descriptor: ProviderDescriptor,
    /// Provider whose resolutions are validated.
    provider: Arc<AsyncFileSystemProvider>,
}

impl ValidatingAsyncFileSystemProvider {
    /// Captures the descriptor used for registration and later validation.
    ///
    /// # Parameters
    ///
    /// - `provider`: Shared asynchronous provider to wrap.
    ///
    /// # Returns
    ///
    /// A validating wrapper bound to the provider's current descriptor.
    ///
    /// # Panics
    ///
    /// Propagates a panic raised while obtaining the provider descriptor.
    #[inline]
    pub(crate) fn new(provider: Arc<AsyncFileSystemProvider>) -> Self {
        let descriptor = provider.descriptor();
        Self {
            descriptor,
            provider,
        }
    }
}

impl ProviderMetadata for ValidatingAsyncFileSystemProvider {
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

impl AsyncServiceProvider<FileSystemSpec>
    for ValidatingAsyncFileSystemProvider
{
    /// Delegates creation and validates the resulting filesystem identity.
    ///
    /// # Parameters
    ///
    /// - `config`: Complete filesystem configuration passed to the provider.
    ///
    /// # Returns
    ///
    /// A future yielding the provider resolution when its identity matches the
    /// descriptor.
    ///
    /// # Errors
    ///
    /// The future yields the provider's creation failure or a contract
    /// violation for a mismatched filesystem identity.
    #[inline]
    fn create_configured<'a>(
        &'a self,
        config: &'a FileSystemConfig,
    ) -> ProviderFuture<
        'a,
        Result<AsyncFileSystemResolution, ProviderFailure<FsError>>,
    > {
        Box::pin(async move {
            let resolution = self.provider.create_configured(config).await?;
            validate_async_resolution(&self.descriptor, resolution)
        })
    }
}
