// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provider-decoded filesystem resolution.

use std::fmt::{
    Debug,
    Formatter,
    Result as FmtResult,
};
use std::sync::Arc;

use qubit_fs::{
    FsPath,
    FsUri,
};

/// Filesystem object paired with its provider-decoded resource location.
pub struct FileSystemResolution<F: ?Sized> {
    file_system: Arc<F>,
    path: FsPath,
    canonical_uri: FsUri,
}

impl<F: ?Sized> FileSystemResolution<F> {
    /// Creates a complete provider resolution.
    #[inline]
    #[must_use]
    pub fn new(
        file_system: Arc<F>,
        path: FsPath,
        canonical_uri: FsUri,
    ) -> Self {
        Self {
            file_system,
            path,
            canonical_uri,
        }
    }

    /// Returns the configured filesystem object.
    #[inline]
    #[must_use]
    pub fn file_system(&self) -> &Arc<F> {
        &self.file_system
    }

    /// Returns the provider-decoded filesystem-local path.
    #[inline]
    #[must_use]
    pub const fn path(&self) -> &FsPath {
        &self.path
    }

    /// Returns the safe canonical resource URI.
    #[inline]
    #[must_use]
    pub const fn canonical_uri(&self) -> &FsUri {
        &self.canonical_uri
    }

    /// Consumes the resolution into its filesystem and location components.
    #[inline]
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
