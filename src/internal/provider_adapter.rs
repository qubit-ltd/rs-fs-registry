// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provider adapters enforcing filesystem-specific creation invariants.

use qubit_fs::FsError;
use qubit_fs::FsErrorKind;
use qubit_fs::FsOperation;
use qubit_spi::ProviderDescriptor;
use qubit_spi::error::ProviderFailure;

#[cfg(feature = "async")]
use crate::AsyncFileSystemResolution;
use crate::FileSystemResolution;

/// Checks the provider identity returned by one synchronous provider.
///
/// # Parameters
///
/// - `descriptor`: Descriptor captured when the provider was registered.
/// - `resolution`: Resolution returned by the provider.
///
/// # Returns
///
/// The unchanged resolution when its provider identity matches `descriptor`.
///
/// # Errors
///
/// Returns a provider contract failure when the identities differ.
#[inline]
pub(super) fn validate_sync_resolution(
    descriptor: &ProviderDescriptor,
    resolution: FileSystemResolution,
) -> Result<FileSystemResolution, ProviderFailure<FsError>> {
    if resolution.file_system().properties().info().provider_id() == descriptor.id().as_str() {
        Ok(resolution)
    } else {
        Err(provider_identity_mismatch(descriptor))
    }
}

/// Checks the provider identity returned by one asynchronous provider.
///
/// # Parameters
///
/// - `descriptor`: Descriptor captured when the provider was registered.
/// - `resolution`: Resolution returned by the provider.
///
/// # Returns
///
/// The unchanged resolution when its provider identity matches `descriptor`.
///
/// # Errors
///
/// Returns a provider contract failure when the identities differ.
#[inline]
#[cfg(feature = "async")]
pub(super) fn validate_async_resolution(
    descriptor: &ProviderDescriptor,
    resolution: AsyncFileSystemResolution,
) -> Result<AsyncFileSystemResolution, ProviderFailure<FsError>> {
    if resolution.file_system().properties().info().provider_id() == descriptor.id().as_str() {
        Ok(resolution)
    } else {
        Err(provider_identity_mismatch(descriptor))
    }
}

/// Creates a provider-construction contract failure.
///
/// # Parameters
///
/// - `descriptor`: Descriptor whose identity the provider contradicted.
///
/// # Returns
///
/// A failure containing a safe contract-violation error and provider ID.
#[inline]
fn provider_identity_mismatch(descriptor: &ProviderDescriptor) -> ProviderFailure<FsError> {
    ProviderFailure::initialization_failed(
        FsError::new(
            FsErrorKind::ProviderContractViolation,
            FsOperation::Provider,
            "filesystem provider identity does not match its registered descriptor",
        )
        .with_provider(descriptor.id().as_str().to_owned()),
    )
}
