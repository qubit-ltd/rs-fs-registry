// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provider-decoded filesystem resolution.

use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::sync::Arc;

use qubit_fs::{FsPath, FsUri};

/// Filesystem object paired with its provider-decoded resource location.
///
/// # Type Parameters
///
/// * `F` - Concrete or trait-object filesystem type retained by the resolution.
pub struct FileSystemResolution<F: ?Sized> {
    /// Shared configured filesystem instance.
    file_system: Arc<F>,
    /// Provider-decoded filesystem-local path.
    path: FsPath,
    /// Canonical secret-free URI for the resolved resource.
    canonical_uri: FsUri,
}

impl<F: ?Sized> FileSystemResolution<F> {
    /// Creates a complete provider resolution.
    ///
    /// # Parameters
    ///
    /// * `file_system` - Shared configured filesystem instance.
    /// * `path` - Provider-decoded filesystem-local path.
    /// * `canonical_uri` - Canonical secret-free resource URI.
    ///
    /// # Returns
    ///
    /// A resolution retaining all three identity components.
    #[inline(always)]
    #[must_use]
    pub fn new(file_system: Arc<F>, path: FsPath, canonical_uri: FsUri) -> Self {
        Self {
            file_system,
            path,
            canonical_uri,
        }
    }

    /// Returns the configured filesystem object.
    ///
    /// # Returns
    ///
    /// The shared configured filesystem instance.
    #[inline(always)]
    #[must_use]
    pub fn file_system(&self) -> &Arc<F> {
        &self.file_system
    }

    /// Returns the provider-decoded filesystem-local path.
    ///
    /// # Returns
    ///
    /// The path decoded by the selected provider.
    #[inline(always)]
    #[must_use]
    pub const fn path(&self) -> &FsPath {
        &self.path
    }

    /// Returns the safe canonical resource URI.
    ///
    /// # Returns
    ///
    /// The canonical secret-free URI returned by the provider.
    #[inline(always)]
    #[must_use]
    pub const fn canonical_uri(&self) -> &FsUri {
        &self.canonical_uri
    }

    /// Consumes the resolution into its filesystem and location components.
    ///
    /// # Returns
    ///
    /// The filesystem, provider-decoded path, and canonical URI.
    #[inline(always)]
    #[must_use]
    pub fn into_parts(self) -> (Arc<F>, FsPath, FsUri) {
        (self.file_system, self.path, self.canonical_uri)
    }
}

impl<F: ?Sized> Clone for FileSystemResolution<F> {
    fn clone(&self) -> Self {
        Self {
            file_system: self.file_system.clone(),
            path: self.path.clone(),
            canonical_uri: self.canonical_uri.clone(),
        }
    }
}

impl<F: ?Sized> Debug for FileSystemResolution<F> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("FileSystemResolution")
            .field("path", &self.path)
            .field("canonical_uri", &self.canonical_uri)
            .finish_non_exhaustive()
    }
}
