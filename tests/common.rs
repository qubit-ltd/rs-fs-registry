// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow test-file-name -- shared integration-test fixture module.

#[cfg(feature = "async")]
use std::future::Future;
#[cfg(feature = "async")]
use std::pin::pin;
#[cfg(feature = "async")]
use std::task::Context;
#[cfg(feature = "async")]
use std::task::Poll;

#[cfg(feature = "async")]
use qubit_fs::AsyncFileSystem;
use qubit_fs::CreateDirectoryOutcome;
use qubit_fs::DeleteOutcome;
use qubit_fs::FileSystem;
use qubit_fs::FileSystemCapabilities;
use qubit_fs::FileSystemId;
use qubit_fs::FileSystemInfo;
use qubit_fs::FileSystemLimits;
use qubit_fs::FileSystemProperties;
use qubit_fs::FsError;
use qubit_fs::FsErrorKind;
use qubit_fs::FsOperation;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::PathConstraints;
use qubit_fs::PathSemantics;
use qubit_fs::RenameFailureState;
use qubit_fs::RenameOutcome;
use qubit_fs::SymlinkPolicy;
use qubit_fs::Uri;
#[cfg(feature = "async")]
use qubit_fs::spi::AsyncFileSystemSpi;
use qubit_fs::spi::CreateDirectoryRequest;
use qubit_fs::spi::CreateTempDirectoryRequest;
use qubit_fs::spi::CreateTempFileRequest;
use qubit_fs::spi::DeleteDirectoryRequest;
use qubit_fs::spi::DeleteFileRequest;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::ListRequest;
use qubit_fs::spi::OpenReaderRequest;
use qubit_fs::spi::OpenWriterRequest;
use qubit_fs::spi::RenameRequest;
#[cfg(feature = "async")]
use qubit_fs::spi::SpiFuture;
#[cfg(feature = "async")]
use qubit_fs::spi::OpenedAsyncDirectoryStream;
#[cfg(feature = "async")]
use qubit_fs::spi::OpenedAsyncReader;
#[cfg(feature = "async")]
use qubit_fs::spi::OpenedAsyncTempDirectory;
#[cfg(feature = "async")]
use qubit_fs::spi::OpenedAsyncTempFile;
#[cfg(feature = "async")]
use qubit_fs::spi::OpenedAsyncWriter;
use qubit_fs::spi::SpiRenameFailure;
use qubit_fs::spi::OpenedDirectoryStream;
use qubit_fs::spi::OpenedReader;
use qubit_fs::spi::OpenedWriter;
use qubit_fs::spi::OpenedTempDirectory;
use qubit_fs::spi::OpenedTempFile;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;
#[cfg(feature = "async")]
use qubit_fs_registry::AsyncFileSystemResolution;
use qubit_fs_registry::FileSystemResolution;

/// Polls a test future to completion without an asynchronous runtime.
///
/// This helper is intended only for deterministic registry futures that do
/// not depend on an external reactor.
///
/// # Parameters
///
/// - `future`: Future to poll on the current test thread.
///
/// # Returns
///
/// The future's completed output.
#[cfg(feature = "async")]
pub(crate) fn block_on<F: Future>(future: F) -> F::Output {
    let waker = std::task::Waker::noop();
    let mut context = Context::from_waker(waker);
    let mut future = pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

/// Creates a synchronous resolution fixture for `provider_id`.
///
/// # Parameters
///
/// - `provider_id`: Provider identity embedded in the fixture properties.
///
/// # Returns
///
/// A validated synchronous resolution fixture.
///
/// # Panics
///
/// Panics when the fixed fixture path, URI, or filesystem properties violate
/// their constructors' contracts.
pub(crate) fn sync_resolution(
    provider_id: &'static str,
) -> FileSystemResolution {
    let file_system = FileSystem::from_spi(SyncPropertiesOnlySpi {
        provider_id,
        scheme: Some("registry-test"),
        limits: FileSystemLimits::unknown(),
        path_constraints: PathConstraints::absolute(),
    })
    .expect("valid test facade");
    FileSystemResolution::try_new(
        file_system,
        Path::parse("/resource").expect("valid test path"),
        Uri::parse("registry-test:///resource").expect("valid canonical URI"),
    )
    .expect("valid test resolution")
}

/// Creates a synchronous resolution with explicit URI schemes.
///
/// # Parameters
///
/// - `provider_id`: Provider identity embedded in the fixture properties.
/// - `scheme`: URI scheme advertised by the filesystem.
/// - `canonical_uri`: Canonical URI returned by the provider.
///
/// # Returns
///
/// A validated resolution, or the path/URI validation error.
///
/// # Panics
///
/// Panics when `provider_id`, `scheme`, or `canonical_uri` cannot construct the
/// test fixture.
pub(crate) fn sync_resolution_with_scheme(
    provider_id: &'static str,
    scheme: &'static str,
    canonical_uri: &str,
) -> Result<FileSystemResolution, FsError> {
    let file_system = FileSystem::from_spi(SyncPropertiesOnlySpi {
        provider_id,
        scheme: Some(scheme),
        limits: FileSystemLimits::unknown(),
        path_constraints: PathConstraints::absolute(),
    })
    .expect("valid test facade");
    FileSystemResolution::try_new(
        file_system,
        Path::parse("/resource").expect("valid test path"),
        Uri::parse(canonical_uri).expect("valid canonical URI"),
    )
}

/// Creates a synchronous resolution with the supplied path validation
/// properties.
///
/// # Parameters
///
/// - `provider_id`: Provider identity embedded in the fixture properties.
/// - `path`: Provider-decoded path to validate.
/// - `limits`: Filesystem limits applied to the path.
/// - `path_constraints`: Structural path constraints to enforce.
///
/// # Returns
///
/// A validated resolution, or the path validation error.
///
/// # Panics
///
/// Panics when `provider_id` or `path` cannot construct the test fixture.
pub(crate) fn sync_resolution_with_path_properties(
    provider_id: &'static str,
    path: &str,
    limits: FileSystemLimits,
    path_constraints: PathConstraints,
) -> Result<FileSystemResolution, FsError> {
    let file_system = FileSystem::from_spi(SyncPropertiesOnlySpi {
        provider_id,
        scheme: None,
        limits,
        path_constraints,
    })
    .expect("valid test facade");
    FileSystemResolution::try_new(
        file_system,
        Path::parse(path).expect("valid test path"),
        Uri::parse("registry-test:///resource").expect("valid canonical URI"),
    )
}

/// Creates an asynchronous resolution fixture for `provider_id`.
///
/// # Parameters
///
/// - `provider_id`: Provider identity embedded in the fixture properties.
///
/// # Returns
///
/// A validated asynchronous resolution fixture.
///
/// # Panics
///
/// Panics when the fixed fixture path, URI, or filesystem properties violate
/// their constructors' contracts.
#[cfg(feature = "async")]
pub(crate) fn async_resolution(
    provider_id: &'static str,
) -> AsyncFileSystemResolution {
    let file_system = AsyncFileSystem::from_spi(AsyncPropertiesOnlySpi {
        provider_id,
        scheme: Some("registry-test"),
        limits: FileSystemLimits::unknown(),
        path_constraints: PathConstraints::absolute(),
    })
    .expect("valid test facade");
    AsyncFileSystemResolution::try_new(
        file_system,
        Path::parse("/resource").expect("valid test path"),
        Uri::parse("registry-test:///resource").expect("valid canonical URI"),
    )
    .expect("valid test resolution")
}

/// Creates an asynchronous resolution with explicit URI schemes.
///
/// # Parameters
///
/// - `provider_id`: Provider identity embedded in the fixture properties.
/// - `scheme`: URI scheme advertised by the filesystem.
/// - `canonical_uri`: Canonical URI returned by the provider.
///
/// # Returns
///
/// A validated resolution, or the path/URI validation error.
///
/// # Panics
///
/// Panics when `provider_id`, `scheme`, or `canonical_uri` cannot construct the
/// test fixture.
#[cfg(feature = "async")]
pub(crate) fn async_resolution_with_scheme(
    provider_id: &'static str,
    scheme: &'static str,
    canonical_uri: &str,
) -> Result<AsyncFileSystemResolution, FsError> {
    let file_system = AsyncFileSystem::from_spi(AsyncPropertiesOnlySpi {
        provider_id,
        scheme: Some(scheme),
        limits: FileSystemLimits::unknown(),
        path_constraints: PathConstraints::absolute(),
    })
    .expect("valid test facade");
    AsyncFileSystemResolution::try_new(
        file_system,
        Path::parse("/resource").expect("valid test path"),
        Uri::parse(canonical_uri).expect("valid canonical URI"),
    )
}

/// Creates an asynchronous resolution with the supplied path validation
/// properties.
///
/// # Parameters
///
/// - `provider_id`: Provider identity embedded in the fixture properties.
/// - `path`: Provider-decoded path to validate.
/// - `limits`: Filesystem limits applied to the path.
/// - `path_constraints`: Structural path constraints to enforce.
///
/// # Returns
///
/// A validated resolution, or the path validation error.
///
/// # Panics
///
/// Panics when `provider_id` or `path` cannot construct the test fixture.
#[cfg(feature = "async")]
pub(crate) fn async_resolution_with_path_properties(
    provider_id: &'static str,
    path: &str,
    limits: FileSystemLimits,
    path_constraints: PathConstraints,
) -> Result<AsyncFileSystemResolution, FsError> {
    let file_system = AsyncFileSystem::from_spi(AsyncPropertiesOnlySpi {
        provider_id,
        scheme: None,
        limits,
        path_constraints,
    })
    .expect("valid test facade");
    AsyncFileSystemResolution::try_new(
        file_system,
        Path::parse(path).expect("valid test path"),
        Uri::parse("registry-test:///resource").expect("valid canonical URI"),
    )
}

/// Builds filesystem properties for a test-only SPI implementation.
///
/// # Parameters
///
/// - `provider_id`: Provider identity exposed by the filesystem.
/// - `scheme`: Optional URI scheme exposed by the filesystem.
/// - `limits`: Limits exposed by the filesystem.
/// - `path_constraints`: Path constraints exposed by the filesystem.
///
/// # Returns
///
/// Valid filesystem properties for the requested fixture values.
///
/// # Panics
///
/// Panics when the requested identifiers, scheme, or properties are invalid.
fn properties(
    provider_id: &'static str,
    scheme: Option<&str>,
    limits: FileSystemLimits,
    path_constraints: PathConstraints,
) -> FileSystemProperties {
    let mut info = FileSystemInfo::new(
        FileSystemId::new("registry-test-fs").expect("valid filesystem ID"),
        provider_id,
        PathSemantics::Hierarchical,
    );
    if let Some(scheme) = scheme {
        info = info.with_scheme(scheme).expect("valid test scheme");
    }
    FileSystemProperties::new(
        info,
        FileSystemCapabilities::new(),
        limits,
        path_constraints,
        SymlinkPolicy::Reject,
    )
    .expect("valid test properties")
}

/// Creates the sentinel error returned by unsupported fixture operations.
///
/// # Returns
///
/// An unsupported-operation error for test-only filesystem calls.
fn unused() -> FsError {
    FsError::new(
        FsErrorKind::UnsupportedOperation,
        FsOperation::Other,
        "unused test operation",
    )
}

struct SyncPropertiesOnlySpi {
    provider_id: &'static str,
    scheme: Option<&'static str>,
    limits: FileSystemLimits,
    path_constraints: PathConstraints,
}

impl FileSystemSpi for SyncPropertiesOnlySpi {
    fn properties(&self) -> FileSystemProperties {
        properties(
            self.provider_id,
            self.scheme,
            self.limits,
            self.path_constraints.clone(),
        )
    }

    fn stat(
        &self,
        _: StatRequest<'_>,
    ) -> FsResult<StatResponse> {
        Err(unused())
    }

    fn list(
        &self,
        _: ListRequest<'_>,
    ) -> FsResult<OpenedDirectoryStream> {
        Err(unused())
    }

    fn open_reader(
        &self,
        _: OpenReaderRequest<'_>,
    ) -> FsResult<OpenedReader> {
        Err(unused())
    }

    fn open_writer(
        &self,
        _: OpenWriterRequest<'_>,
    ) -> FsResult<OpenedWriter> {
        Err(unused())
    }

    fn create_directory(
        &self,
        _: CreateDirectoryRequest<'_>,
    ) -> FsResult<CreateDirectoryOutcome> {
        Err(unused())
    }

    fn delete_file(&self, _: DeleteFileRequest<'_>) -> FsResult<DeleteOutcome> {
        Err(unused())
    }

    fn delete_directory(
        &self,
        _: DeleteDirectoryRequest<'_>,
    ) -> FsResult<DeleteOutcome> {
        Err(unused())
    }

    fn rename(
        &self,
        _: RenameRequest<'_>,
    ) -> Result<RenameOutcome, SpiRenameFailure> {
        Err(SpiRenameFailure::new(
            unused(),
            RenameFailureState::Unchanged,
        ))
    }

    fn create_temp_file(
        &self,
        _: CreateTempFileRequest,
    ) -> FsResult<OpenedTempFile> {
        Err(unused())
    }

    fn create_temp_directory(
        &self,
        _: CreateTempDirectoryRequest,
    ) -> FsResult<OpenedTempDirectory> {
        Err(unused())
    }
}

#[cfg(feature = "async")]
struct AsyncPropertiesOnlySpi {
    provider_id: &'static str,
    scheme: Option<&'static str>,
    limits: FileSystemLimits,
    path_constraints: PathConstraints,
}

#[cfg(feature = "async")]
impl AsyncFileSystemSpi for AsyncPropertiesOnlySpi {
    fn properties(&self) -> FileSystemProperties {
        properties(
            self.provider_id,
            self.scheme,
            self.limits,
            self.path_constraints.clone(),
        )
    }

    fn stat<'a>(
        &'a self,
        _: StatRequest<'a>,
    ) -> SpiFuture<'a, FsResult<StatResponse>> {
        Box::pin(async { Err(unused()) })
    }

    fn list<'a>(
        &'a self,
        _: ListRequest<'a>,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncDirectoryStream>>
    {
        Box::pin(async { Err(unused()) })
    }

    fn open_reader<'a>(
        &'a self,
        _: OpenReaderRequest<'a>,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncReader>> {
        Box::pin(async { Err(unused()) })
    }

    fn open_writer<'a>(
        &'a self,
        _: OpenWriterRequest<'a>,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncWriter>> {
        Box::pin(async { Err(unused()) })
    }

    fn create_directory<'a>(
        &'a self,
        _: CreateDirectoryRequest<'a>,
    ) -> SpiFuture<'a, FsResult<CreateDirectoryOutcome>> {
        Box::pin(async { Err(unused()) })
    }

    fn delete_file<'a>(
        &'a self,
        _: DeleteFileRequest<'a>,
    ) -> SpiFuture<'a, FsResult<DeleteOutcome>> {
        Box::pin(async { Err(unused()) })
    }

    fn delete_directory<'a>(
        &'a self,
        _: DeleteDirectoryRequest<'a>,
    ) -> SpiFuture<'a, FsResult<DeleteOutcome>> {
        Box::pin(async { Err(unused()) })
    }

    fn rename<'a>(
        &'a self,
        _: RenameRequest<'a>,
    ) -> SpiFuture<'a, Result<RenameOutcome, SpiRenameFailure>> {
        Box::pin(async {
            Err(SpiRenameFailure::new(
                unused(),
                RenameFailureState::Unchanged,
            ))
        })
    }

    fn create_temp_file<'a>(
        &'a self,
        _: CreateTempFileRequest,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncTempFile>> {
        Box::pin(async { Err(unused()) })
    }

    fn create_temp_directory<'a>(
        &'a self,
        _: CreateTempDirectoryRequest,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncTempDirectory>> {
        Box::pin(async { Err(unused()) })
    }
}
