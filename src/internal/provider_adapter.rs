// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow all -- sync and async validating adapters are one internal
// invariant boundary.
//! Provider adapters enforcing filesystem-specific creation invariants.

use std::sync::Arc;

use qubit_fs::{
    FsError,
    FsErrorKind,
    FsOperation,
};
use qubit_spi::error::ProviderFailure;
use qubit_spi::{
    AsyncServiceProvider,
    ProviderDescriptor,
    ProviderFuture,
    ProviderMetadata,
    ServiceProvider,
};

use crate::{
    AsyncFileSystemProvider,
    AsyncFileSystemResolution,
    FileSystemConfig,
    FileSystemProvider,
    FileSystemResolution,
    FileSystemSpec,
};

/// Synchronous provider wrapper that binds successful output to its descriptor.
pub(crate) struct ValidatingFileSystemProvider {
    descriptor: ProviderDescriptor,
    provider: Arc<FileSystemProvider>,
}

impl ValidatingFileSystemProvider {
    /// Captures the descriptor used for registration and later validation.
    pub(crate) fn new(provider: Arc<FileSystemProvider>) -> Self {
        let descriptor = provider.descriptor();
        Self {
            descriptor,
            provider,
        }
    }
}

impl ProviderMetadata for ValidatingFileSystemProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        self.descriptor.clone()
    }
}

impl ServiceProvider<FileSystemSpec> for ValidatingFileSystemProvider {
    fn create_configured(
        &self,
        config: &FileSystemConfig,
    ) -> Result<FileSystemResolution, ProviderFailure<FsError>> {
        let resolution = self.provider.create_configured(config)?;
        validate_sync_resolution(&self.descriptor, resolution)
    }
}

/// Asynchronous provider wrapper that binds successful output to its
/// descriptor.
pub(crate) struct ValidatingAsyncFileSystemProvider {
    descriptor: ProviderDescriptor,
    provider: Arc<AsyncFileSystemProvider>,
}

impl ValidatingAsyncFileSystemProvider {
    /// Captures the descriptor used for registration and later validation.
    pub(crate) fn new(provider: Arc<AsyncFileSystemProvider>) -> Self {
        let descriptor = provider.descriptor();
        Self {
            descriptor,
            provider,
        }
    }
}

impl ProviderMetadata for ValidatingAsyncFileSystemProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        self.descriptor.clone()
    }
}

impl AsyncServiceProvider<FileSystemSpec>
    for ValidatingAsyncFileSystemProvider
{
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

/// Checks the provider identity returned by one synchronous provider.
fn validate_sync_resolution(
    descriptor: &ProviderDescriptor,
    resolution: FileSystemResolution,
) -> Result<FileSystemResolution, ProviderFailure<FsError>> {
    if resolution.file_system().properties().info().provider_id()
        == descriptor.id().as_str()
    {
        Ok(resolution)
    } else {
        Err(provider_identity_mismatch(descriptor))
    }
}

/// Checks the provider identity returned by one asynchronous provider.
fn validate_async_resolution(
    descriptor: &ProviderDescriptor,
    resolution: AsyncFileSystemResolution,
) -> Result<AsyncFileSystemResolution, ProviderFailure<FsError>> {
    if resolution.file_system().properties().info().provider_id()
        == descriptor.id().as_str()
    {
        Ok(resolution)
    } else {
        Err(provider_identity_mismatch(descriptor))
    }
}

/// Creates a provider-construction contract failure.
fn provider_identity_mismatch(
    descriptor: &ProviderDescriptor,
) -> ProviderFailure<FsError> {
    ProviderFailure::initialization_failed(
        FsError::new(
            FsErrorKind::ProviderContractViolation,
            FsOperation::Provider,
            "filesystem provider identity does not match its registered descriptor",
        )
        .with_provider(descriptor.id().as_str().to_owned()),
    )
}
