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
pub type AsyncFileSystemProvider = dyn AsyncProviderDefinition<FileSystemSpec>;
