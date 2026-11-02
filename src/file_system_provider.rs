// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Filesystem provider trait object alias.

use qubit_spi::ProviderDefinition;

use crate::FileSystemSpec;

/// Self-described filesystem provider trait object type.
///
/// Implementations expose both filesystem creation behavior and the stable
/// descriptor used when registering them in a
/// [`FileSystemRegistry`](crate::FileSystemRegistry).
///
/// The [`ProviderDefinition`] combines provider metadata with configured
/// creation. [`FileSystemSpec`] fixes the configuration, resolution, and
/// filesystem error types; [`FileSystemRegistry`](crate::FileSystemRegistry)
/// validates returned identities. Applications provide a shared provider
/// implementing the metadata and service-provider contracts described by
/// [`ProviderDefinition`]. The example assumes the registry is not sealed and
/// the provider descriptor does not conflict with an existing registration;
/// see [`FileSystemRegistry`](crate::FileSystemRegistry) for registry rules and
/// [`ProviderDefinition`] for the SPI contract.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
///
/// use qubit_fs_registry::FileSystemProvider;
/// use qubit_fs_registry::FileSystemRegistry;
/// use qubit_fs_registry::FileSystemRegistryResult;
///
/// fn register_provider(
///     registry: &FileSystemRegistry,
///     provider: Arc<FileSystemProvider>,
/// ) -> FileSystemRegistryResult<()> {
///     registry.register_shared(provider)?;
///     assert!(!registry.is_empty());
///     Ok(())
/// }
/// ```
pub type FileSystemProvider = dyn ProviderDefinition<FileSystemSpec>;
