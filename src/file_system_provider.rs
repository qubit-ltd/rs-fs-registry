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
/// validates returned identities.
///
/// # Examples
///
/// ```
/// use qubit_fs_registry::FileSystemProvider;
/// use qubit_fs_registry::FileSystemRegistry;
///
/// let registry = FileSystemRegistry::default();
/// fn accepts(_provider: &FileSystemProvider) {}
/// assert!(registry.is_empty());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub type FileSystemProvider = dyn ProviderDefinition<FileSystemSpec>;
