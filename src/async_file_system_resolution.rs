// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provider-decoded asynchronous filesystem resolution.

use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

use qubit_fs::AsyncFileSystem;
use qubit_fs::FsError;
use qubit_fs::FsErrorKind;
use qubit_fs::FsOperation;
use qubit_fs::Path;
use qubit_fs::Uri;

/// A configured asynchronous facade paired with its decoded location.
#[derive(Clone)]
#[must_use]
pub struct AsyncFileSystemResolution {
    /// Configured asynchronous filesystem facade.
    file_system: AsyncFileSystem,
    /// Provider-decoded path within the filesystem.
    path: Path,
    /// Secret-free URI describing the canonical resolved location.
    canonical_uri: Uri,
}

impl AsyncFileSystemResolution {
    /// Validates and creates a resolution from one provider result.
    ///
    /// The path must satisfy the facade constraints and limits. When the
    /// facade advertises schemes, the canonical URI scheme must be one of
    /// them.
    ///
    /// # Parameters
    ///
    /// - `file_system`: Configured asynchronous filesystem from the provider.
    /// - `path`: Provider-decoded path to validate.
    /// - `canonical_uri`: Secret-free canonical URI to validate.
    ///
    /// # Returns
    ///
    /// A validated resolution containing all three components.
    ///
    /// # Errors
    ///
    /// Returns an [`FsError`] when the path violates facade constraints or
    /// limits, or when the canonical URI scheme is unsupported.
    pub fn try_new(
        file_system: AsyncFileSystem,
        path: Path,
        canonical_uri: Uri,
    ) -> Result<Self, FsError> {
        let p = file_system.properties();
        p.path_constraints().validate(&path)?;
        p.limits().validate_path(
            &path,
            p.info().path_semantics(),
            FsOperation::ParsePath,
        )?;
        if !p
            .info()
            .schemes()
            .iter()
            .any(|s| s == canonical_uri.scheme())
        {
            return Err(FsError::new(
                FsErrorKind::InvalidUri,
                FsOperation::Provider,
                "canonical URI scheme is not supported by the filesystem",
            ));
        }
        Ok(Self {
            file_system,
            path,
            canonical_uri,
        })
    }
    /// Returns the configured facade.
    ///
    /// # Returns
    ///
    /// The configured asynchronous filesystem.
    #[inline(always)]
    #[must_use]
    pub const fn file_system(&self) -> &AsyncFileSystem {
        &self.file_system
    }
    /// Returns the provider-decoded path.
    ///
    /// # Returns
    ///
    /// The validated provider-decoded path.
    #[inline(always)]
    #[must_use]
    pub const fn path(&self) -> &Path {
        &self.path
    }
    /// Returns the secret-free canonical URI.
    ///
    /// # Returns
    ///
    /// The validated canonical URI.
    #[inline(always)]
    #[must_use]
    pub const fn canonical_uri(&self) -> &Uri {
        &self.canonical_uri
    }
    /// Splits this resolution into owned components.
    ///
    /// # Returns
    ///
    /// The asynchronous filesystem, decoded path, and canonical URI in that
    /// order.
    #[inline(always)]
    #[must_use]
    pub fn into_parts(self) -> (AsyncFileSystem, Path, Uri) {
        (self.file_system, self.path, self.canonical_uri)
    }
}

impl Debug for AsyncFileSystemResolution {
    /// Formats the safe location fields without exposing filesystem internals.
    ///
    /// # Parameters
    ///
    /// - `f`: Destination formatter.
    ///
    /// # Returns
    ///
    /// The formatter result.
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("AsyncFileSystemResolution")
            .field("path", &self.path)
            .field("canonical_uri", &self.canonical_uri)
            .finish_non_exhaustive()
    }
}
