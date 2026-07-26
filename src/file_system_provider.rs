// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Filesystem provider trait object alias.

use qubit_spi::ProviderDefinition;

use super::file_system_spec::FileSystemSpec;

/// Self-described filesystem provider trait object type.
///
/// Implementations expose both filesystem creation behavior and the stable
/// descriptor used when registering them in a
/// [`FileSystemRegistry`](crate::FileSystemRegistry).
pub type FileSystemProvider = dyn ProviderDefinition<FileSystemSpec>;
