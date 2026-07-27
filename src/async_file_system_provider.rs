// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Asynchronous filesystem provider trait object alias and error mapping.

use qubit_spi::AsyncProviderDefinition;
use qubit_spi::error::{
    ProviderError,
    ProviderErrorKind,
};

use qubit_fs::{
    FsError,
    FsErrorKind,
};

use crate::FileSystemSpec;

/// Metadata-bearing asynchronous filesystem provider trait object type.
///
/// Implementations expose filesystem creation behavior and the stable
/// descriptor used when registering them in an
/// [`AsyncFileSystemRegistry`](crate::AsyncFileSystemRegistry).
pub type AsyncFileSystemProvider = dyn AsyncProviderDefinition<FileSystemSpec>;

/// Converts a filesystem creation failure into an SPI leaf provider failure.
///
/// # Arguments
///
/// * `error` - Filesystem error returned while creating a filesystem.
///
/// # Returns
///
/// A classified provider error retaining the original filesystem error as its
/// source.
#[must_use]
pub fn map_provider_error(error: FsError) -> ProviderError {
    let reason = format!("filesystem provider failed: {error}");
    let kind = match error.kind() {
        FsErrorKind::ProviderUnavailable => ProviderErrorKind::Unavailable,
        FsErrorKind::UnsupportedOperation
        | FsErrorKind::UnsupportedCapability
        | FsErrorKind::RequirementNotMet => ProviderErrorKind::Unsupported,
        FsErrorKind::InvalidUri
        | FsErrorKind::InvalidPath
        | FsErrorKind::InvalidOptions => {
            ProviderErrorKind::InvalidConfiguration
        }
        _ => ProviderErrorKind::InitializationFailed,
    };
    ProviderError::with_source(kind, reason, error)
}
