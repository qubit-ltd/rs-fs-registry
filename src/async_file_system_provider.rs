// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Asynchronous filesystem provider trait object alias.

use qubit_spi::AsyncProviderDefinition;

use crate::FileSystemSpec;

/// Metadata-bearing asynchronous filesystem provider trait object type.
///
/// Implementations expose filesystem creation behavior and the stable
/// descriptor used when registering them in an
/// [`AsyncFileSystemRegistry`](crate::AsyncFileSystemRegistry).
///
/// The [`AsyncProviderDefinition`] combines provider metadata with configured
/// creation. [`FileSystemSpec`] fixes the configuration, resolution, and
/// filesystem error
/// types; [`AsyncFileSystemRegistry`](crate::AsyncFileSystemRegistry) validates
/// returned identities. Applications provide a shared provider implementing
/// the metadata and service-provider contracts described by
/// [`AsyncProviderDefinition`]. The example assumes the registry is not sealed
/// and the provider descriptor does not conflict with an existing
/// registration; see
/// [`AsyncFileSystemRegistry`](crate::AsyncFileSystemRegistry) for registry
/// rules and [`AsyncProviderDefinition`] for the SPI contract.
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # #[cfg(feature = "async")]
/// # {
/// use std::sync::Arc;
///
/// use qubit_fs_registry::AsyncFileSystemProvider;
/// use qubit_fs_registry::AsyncFileSystemRegistry;
/// use qubit_fs_registry::FileSystemRegistryResult;
///
/// fn register_provider(
///     registry: &AsyncFileSystemRegistry,
///     provider: Arc<AsyncFileSystemProvider>,
/// ) -> FileSystemRegistryResult<()> {
///     registry.register_shared(provider)?;
///     assert!(!registry.is_empty());
///     Ok(())
/// }
/// # }
/// # Ok(())
/// # }
/// ```
pub type AsyncFileSystemProvider = dyn AsyncProviderDefinition<FileSystemSpec>;
