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
use qubit_fs::Path;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::path::Uri;

/// A configured asynchronous facade paired with its decoded location.
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # #[cfg(feature = "async")]
/// # {
/// use qubit_fs_registry::AsyncFileSystemResolution;
/// # use qubit_fs::AsyncFileSystem;
/// # use qubit_fs::FileSystem;
/// # use qubit_fs::FsError;
/// # use qubit_fs::FsResult;
/// # use qubit_fs::Path;
/// # use qubit_fs::directory::CreateDirectoryOutcome;
/// # use qubit_fs::directory::DeleteOutcome;
/// # use qubit_fs::error::FsErrorKind;
/// # use qubit_fs::error::FsOperation;
/// # use qubit_fs::metadata::FileSystemCapabilities;
/// # use qubit_fs::metadata::FileSystemId;
/// # use qubit_fs::metadata::FileSystemInfo;
/// # use qubit_fs::metadata::FileSystemLimits;
/// # use qubit_fs::metadata::SymlinkPolicy;
/// # use qubit_fs::path::PathConstraints;
/// # use qubit_fs::path::PathSemantics;
/// # use qubit_fs::path::Uri;
/// # use qubit_fs::rename::RenameFailureState;
/// # use qubit_fs::rename::RenameOutcome;
/// # use qubit_fs::spi::AsyncFileSystemSpi;
/// # use qubit_fs::spi::CreateDirectoryRequest;
/// # use qubit_fs::spi::CreateTempDirectoryRequest;
/// # use qubit_fs::spi::CreateTempFileRequest;
/// # use qubit_fs::spi::DeleteDirectoryRequest;
/// # use qubit_fs::spi::DeleteFileRequest;
/// # use qubit_fs::spi::FileSystemSpi;
/// # use qubit_fs::spi::ListRequest;
/// # use qubit_fs::spi::OpenReaderRequest;
/// # use qubit_fs::spi::OpenWriterRequest;
/// # use qubit_fs::spi::OpenedAsyncDirectoryStream;
/// # use qubit_fs::spi::OpenedAsyncReader;
/// # use qubit_fs::spi::OpenedAsyncTempDirectory;
/// # use qubit_fs::spi::OpenedAsyncTempFile;
/// # use qubit_fs::spi::OpenedAsyncWriter;
/// # use qubit_fs::spi::OpenedDirectoryStream;
/// # use qubit_fs::spi::OpenedReader;
/// # use qubit_fs::spi::OpenedTempDirectory;
/// # use qubit_fs::spi::OpenedTempFile;
/// # use qubit_fs::spi::OpenedWriter;
/// # use qubit_fs::spi::ProviderOperations;
/// # use qubit_fs::spi::ProviderProperties;
/// # use qubit_fs::spi::RenameRequest;
/// # use qubit_fs::spi::SpiFuture;
/// # use qubit_fs::spi::SpiRenameFailure;
/// # use qubit_fs::spi::StatRequest;
/// # use qubit_fs::spi::StatResponse;
/// # fn properties(
/// #     provider_id: &'static str,
/// #     scheme: Option<&str>,
/// #     limits: FileSystemLimits,
/// #     path_constraints: PathConstraints,
/// #     path_semantics: PathSemantics,
/// # ) -> ProviderProperties {
/// #     let mut info = FileSystemInfo::new(
/// #         FileSystemId::new("registry-test-fs").expect("valid filesystem ID"),
/// #         provider_id,
/// #         path_semantics,
/// #     );
/// #     if let Some(scheme) = scheme {
/// #         info = info.with_scheme(scheme).expect("valid test scheme");
/// #     }
/// #     ProviderProperties::new(
/// #         info,
/// #         ProviderOperations::new(),
/// #         FileSystemCapabilities::new(),
/// #         limits,
/// #         path_constraints,
/// #         SymlinkPolicy::Reject,
/// #     )
/// #     .expect("valid test properties")
/// # }
/// # fn unused() -> FsError {
/// #     FsError::new(
/// #         FsErrorKind::UnsupportedOperation,
/// #         FsOperation::Other,
/// #         "unused test operation",
/// #     )
/// # }
/// # struct AsyncPropertiesOnlySpi {
/// #     provider_id: &'static str,
/// #     scheme: Option<&'static str>,
/// #     limits: FileSystemLimits,
/// #     path_constraints: PathConstraints,
/// #     path_semantics: PathSemantics,
/// # }
/// # impl AsyncFileSystemSpi for AsyncPropertiesOnlySpi {
/// #     fn properties(&self) -> ProviderProperties {
/// #         properties(
/// #             self.provider_id,
/// #             self.scheme,
/// #             self.limits,
/// #             self.path_constraints.clone(),
/// #             self.path_semantics,
/// #         )
/// #     }
/// #
/// #     fn stat<'a>(&'a self, _: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
/// #         Box::pin(async { Err(unused()) })
/// #     }
/// #
/// #     fn list<'a>(&'a self, _: ListRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncDirectoryStream>> {
/// #         Box::pin(async { Err(unused()) })
/// #     }
/// #
/// #     fn open_reader<'a>(&'a self, _: OpenReaderRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncReader>> {
/// #         Box::pin(async { Err(unused()) })
/// #     }
/// #
/// #     fn open_writer<'a>(&'a self, _: OpenWriterRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncWriter>> {
/// #         Box::pin(async { Err(unused()) })
/// #     }
/// #
/// #     fn create_directory<'a>(
/// #         &'a self,
/// #         _: CreateDirectoryRequest<'a>,
/// #     ) -> SpiFuture<'a, FsResult<CreateDirectoryOutcome>> {
/// #         Box::pin(async { Err(unused()) })
/// #     }
/// #
/// #     fn delete_file<'a>(&'a self, _: DeleteFileRequest<'a>) -> SpiFuture<'a, FsResult<DeleteOutcome>> {
/// #         Box::pin(async { Err(unused()) })
/// #     }
/// #
/// #     fn delete_directory<'a>(&'a self, _: DeleteDirectoryRequest<'a>) -> SpiFuture<'a, FsResult<DeleteOutcome>> {
/// #         Box::pin(async { Err(unused()) })
/// #     }
/// #
/// #     fn rename<'a>(&'a self, _: RenameRequest<'a>) -> SpiFuture<'a, Result<RenameOutcome, SpiRenameFailure>> {
/// #         Box::pin(async { Err(SpiRenameFailure::new(unused(), RenameFailureState::Unchanged)) })
/// #     }
/// #
/// #     fn create_temp_file<'a>(&'a self, _: CreateTempFileRequest) -> SpiFuture<'a, FsResult<OpenedAsyncTempFile>> {
/// #         Box::pin(async { Err(unused()) })
/// #     }
/// #
/// #     fn create_temp_directory<'a>(
/// #         &'a self,
/// #         _: CreateTempDirectoryRequest,
/// #     ) -> SpiFuture<'a, FsResult<OpenedAsyncTempDirectory>> {
/// #         Box::pin(async { Err(unused()) })
/// #     }
/// # }
/// # let filesystem = AsyncFileSystem::from_spi(AsyncPropertiesOnlySpi {
/// #     provider_id: "example", scheme: Some("file"), limits: FileSystemLimits::unknown(),
/// #     path_constraints: PathConstraints::absolute(), path_semantics: PathSemantics::Hierarchical,
/// # })?;
/// let resolution = AsyncFileSystemResolution::try_new(
///     filesystem, Path::parse("/report.csv")?, Uri::parse("file:///report.csv")?,
/// )?;
/// assert_eq!(resolution.path().as_str(), "/report.csv");
/// assert_eq!(resolution.canonical_uri().scheme(), "file");
/// let (filesystem, path, uri) = resolution.into_parts();
/// assert_eq!(filesystem.properties().info().provider_id(), "example");
/// assert_eq!(path.as_str(), "/report.csv");
/// assert_eq!(uri.as_str(), "file:///report.csv");
/// # }
/// # Ok(())
/// # }
/// ```
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
    /// The path must satisfy the facade semantics, constraints and limits.
    /// The facade must advertise the canonical URI scheme; an empty scheme
    /// list is rejected. Validation reads properties only and performs no IO.
    /// URI-to-path interpretation remains the provider's responsibility.
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
    pub fn try_new(file_system: AsyncFileSystem, path: Path, canonical_uri: Uri) -> Result<Self, FsError> {
        let p = file_system.properties();
        p.validate_path(&path, FsOperation::ParsePath)?;
        if !p.info().schemes().iter().any(|s| s == canonical_uri.scheme()) {
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
