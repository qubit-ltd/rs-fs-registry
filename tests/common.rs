// qubit-style: allow test-file-name -- shared integration-test fixture module.
// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::spi::{
    AsyncFileSystemSpi,
    CreateDirectoryRequest,
    CreateTempDirectoryRequest,
    CreateTempFileRequest,
    DeleteDirectoryRequest,
    DeleteFileRequest,
    FileSystemSpi,
    ListRequest,
    OpenReaderRequest,
    OpenWriterRequest,
    RenameRequest,
    SpiFuture,
    SpiRenameFailure,
    StatRequest,
};
use qubit_fs::{
    AsyncFileSystem,
    CreateDirectoryOutcome,
    DeleteOutcome,
    FileSystem,
    FileSystemCapabilities,
    FileSystemId,
    FileSystemInfo,
    FileSystemLimits,
    FileSystemProperties,
    FsError,
    FsErrorKind,
    FsOperation,
    FsResult,
    Path,
    PathConstraints,
    PathSemantics,
    RenameFailureState,
    RenameOutcome,
    Uri,
};
use qubit_fs_registry::{
    AsyncFileSystemResolution,
    FileSystemResolution,
};

pub(crate) fn sync_resolution(
    provider_id: &'static str,
) -> FileSystemResolution {
    let file_system =
        FileSystem::from_spi(SyncPropertiesOnlySpi { provider_id })
            .expect("valid test facade");
    FileSystemResolution::try_new(
        file_system,
        Path::parse("/resource").expect("valid test path"),
        Uri::parse("registry-test:///resource").expect("valid canonical URI"),
    )
    .expect("valid test resolution")
}

pub(crate) fn async_resolution(
    provider_id: &'static str,
) -> AsyncFileSystemResolution {
    let file_system =
        AsyncFileSystem::from_spi(AsyncPropertiesOnlySpi { provider_id })
            .expect("valid test facade");
    AsyncFileSystemResolution::try_new(
        file_system,
        Path::parse("/resource").expect("valid test path"),
        Uri::parse("registry-test:///resource").expect("valid canonical URI"),
    )
    .expect("valid test resolution")
}

fn properties(provider_id: &'static str) -> FileSystemProperties {
    FileSystemProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("registry-test-fs").expect("valid filesystem ID"),
            provider_id,
            PathSemantics::Hierarchical,
        ),
        FileSystemCapabilities::new(),
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
    )
    .expect("valid test properties")
}

fn unused() -> FsError {
    FsError::new(
        FsErrorKind::UnsupportedOperation,
        FsOperation::Other,
        "unused test operation",
    )
}

struct SyncPropertiesOnlySpi {
    provider_id: &'static str,
}

impl FileSystemSpi for SyncPropertiesOnlySpi {
    fn properties(&self) -> FileSystemProperties {
        properties(self.provider_id)
    }

    fn stat(
        &self,
        _: StatRequest<'_>,
    ) -> FsResult<qubit_fs::spi::StatResponse> {
        Err(unused())
    }

    fn list(
        &self,
        _: ListRequest<'_>,
    ) -> FsResult<qubit_fs::spi::OpenedDirectoryStream> {
        Err(unused())
    }

    fn open_reader(
        &self,
        _: OpenReaderRequest<'_>,
    ) -> FsResult<qubit_fs::spi::OpenedReader> {
        Err(unused())
    }

    fn open_writer(
        &self,
        _: OpenWriterRequest<'_>,
    ) -> FsResult<qubit_fs::spi::OpenedWriter> {
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
    ) -> FsResult<qubit_fs::spi::OpenedTempFile> {
        Err(unused())
    }

    fn create_temp_directory(
        &self,
        _: CreateTempDirectoryRequest,
    ) -> FsResult<qubit_fs::spi::OpenedTempDirectory> {
        Err(unused())
    }
}

struct AsyncPropertiesOnlySpi {
    provider_id: &'static str,
}

impl AsyncFileSystemSpi for AsyncPropertiesOnlySpi {
    fn properties(&self) -> FileSystemProperties {
        properties(self.provider_id)
    }

    fn stat<'a>(
        &'a self,
        _: StatRequest<'a>,
    ) -> SpiFuture<'a, FsResult<qubit_fs::spi::StatResponse>> {
        Box::pin(async { Err(unused()) })
    }

    fn list<'a>(
        &'a self,
        _: ListRequest<'a>,
    ) -> SpiFuture<'a, FsResult<qubit_fs::spi::OpenedAsyncDirectoryStream>>
    {
        Box::pin(async { Err(unused()) })
    }

    fn open_reader<'a>(
        &'a self,
        _: OpenReaderRequest<'a>,
    ) -> SpiFuture<'a, FsResult<qubit_fs::spi::OpenedAsyncReader>> {
        Box::pin(async { Err(unused()) })
    }

    fn open_writer<'a>(
        &'a self,
        _: OpenWriterRequest<'a>,
    ) -> SpiFuture<'a, FsResult<qubit_fs::spi::OpenedAsyncWriter>> {
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
    ) -> SpiFuture<'a, FsResult<qubit_fs::spi::OpenedAsyncTempFile>> {
        Box::pin(async { Err(unused()) })
    }

    fn create_temp_directory<'a>(
        &'a self,
        _: CreateTempDirectoryRequest,
    ) -> SpiFuture<'a, FsResult<qubit_fs::spi::OpenedAsyncTempDirectory>> {
        Box::pin(async { Err(unused()) })
    }
}
