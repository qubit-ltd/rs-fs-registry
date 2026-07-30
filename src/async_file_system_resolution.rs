// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow all -- resolution behavior is covered by registry
// integration tests.
//! Provider-decoded asynchronous filesystem resolution.
use qubit_fs::{
    AsyncFileSystem,
    FsError,
    FsErrorKind,
    FsOperation,
    Path,
    Uri,
};
use std::fmt::{
    Debug,
    Formatter,
    Result as FmtResult,
};
/// A configured asynchronous facade paired with its decoded location.
#[derive(Clone)]
pub struct AsyncFileSystemResolution {
    file_system: AsyncFileSystem,
    path: Path,
    canonical_uri: Uri,
}
impl AsyncFileSystemResolution {
    /// Validates and creates a resolution from one provider result.
    ///
    /// The path must satisfy the facade constraints and limits. When the
    /// facade advertises schemes, the canonical URI scheme must be one of
    /// them.
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
        if !p.info().schemes().is_empty()
            && !p
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
    #[must_use]
    pub const fn file_system(&self) -> &AsyncFileSystem {
        &self.file_system
    }
    /// Returns the provider-decoded path.
    #[must_use]
    pub const fn path(&self) -> &Path {
        &self.path
    }
    /// Returns the secret-free canonical URI.
    #[must_use]
    pub const fn canonical_uri(&self) -> &Uri {
        &self.canonical_uri
    }
    /// Splits this resolution into owned components.
    #[must_use]
    pub fn into_parts(self) -> (AsyncFileSystem, Path, Uri) {
        (self.file_system, self.path, self.canonical_uri)
    }
}
impl Debug for AsyncFileSystemResolution {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("AsyncFileSystemResolution")
            .field("path", &self.path)
            .field("canonical_uri", &self.canonical_uri)
            .finish_non_exhaustive()
    }
}
