// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// Shared, properties-only fixtures for the Resolution rustdoc examples.
// These implementations satisfy the real filesystem SPI contracts while
// returning unsupported-operation errors for methods the examples never call.

mod common {
    use qubit_fs::FsError;
    use qubit_fs::error::FsErrorKind;
    use qubit_fs::error::FsOperation;
    use qubit_fs::metadata::FileSystemCapabilities;
    use qubit_fs::metadata::FileSystemId;
    use qubit_fs::metadata::FileSystemInfo;
    use qubit_fs::metadata::FileSystemLimits;
    use qubit_fs::metadata::SymlinkPolicy;
    use qubit_fs::path::PathConstraints;
    use qubit_fs::path::PathSemantics;
    use qubit_fs::spi::ProviderOperations;
    use qubit_fs::spi::ProviderProperties;

    pub fn properties(
        provider_id: &'static str,
        scheme: Option<&str>,
        limits: FileSystemLimits,
        path_constraints: PathConstraints,
        path_semantics: PathSemantics,
    ) -> ProviderProperties {
        let mut info = FileSystemInfo::new(
            FileSystemId::new("registry-test-fs").expect("valid filesystem ID"),
            provider_id,
            path_semantics,
        );
        if let Some(scheme) = scheme {
            info = info.with_scheme(scheme).expect("valid test scheme");
        }
        ProviderProperties::new(
            info,
            ProviderOperations::new(),
            FileSystemCapabilities::new(),
            limits,
            path_constraints,
            SymlinkPolicy::Reject,
        )
        .expect("valid test properties")
    }

    pub fn unused() -> FsError {
        FsError::new(
            FsErrorKind::UnsupportedOperation,
            FsOperation::Other,
            "unused test operation",
        )
    }
}

mod sync {
    use qubit_fs::FileSystem;
    use qubit_fs::FsError;
    use qubit_fs::FsResult;
    use qubit_fs::directory::CreateDirectoryOutcome;
    use qubit_fs::directory::DeleteOutcome;
    use qubit_fs::metadata::FileSystemLimits;
    use qubit_fs::path::PathConstraints;
    use qubit_fs::path::PathSemantics;
    use qubit_fs::rename::RenameFailureState;
    use qubit_fs::rename::RenameOutcome;
    use qubit_fs::spi::CreateDirectoryRequest;
    use qubit_fs::spi::CreateTempDirectoryRequest;
    use qubit_fs::spi::CreateTempFileRequest;
    use qubit_fs::spi::DeleteDirectoryRequest;
    use qubit_fs::spi::DeleteFileRequest;
    use qubit_fs::spi::FileSystemSpi;
    use qubit_fs::spi::ListRequest;
    use qubit_fs::spi::OpenReaderRequest;
    use qubit_fs::spi::OpenWriterRequest;
    use qubit_fs::spi::OpenedDirectoryStream;
    use qubit_fs::spi::OpenedReader;
    use qubit_fs::spi::OpenedTempDirectory;
    use qubit_fs::spi::OpenedTempFile;
    use qubit_fs::spi::OpenedWriter;
    use qubit_fs::spi::ProviderProperties;
    use qubit_fs::spi::RenameRequest;
    use qubit_fs::spi::SpiRenameFailure;
    use qubit_fs::spi::StatRequest;
    use qubit_fs::spi::StatResponse;

    use super::common::properties;
    use super::common::unused;

    struct SyncPropertiesOnlySpi {
        provider_id: &'static str,
        scheme: Option<&'static str>,
        limits: FileSystemLimits,
        path_constraints: PathConstraints,
        path_semantics: PathSemantics,
    }
    impl FileSystemSpi for SyncPropertiesOnlySpi {
        fn properties(&self) -> ProviderProperties {
            properties(
                self.provider_id,
                self.scheme,
                self.limits,
                self.path_constraints.clone(),
                self.path_semantics,
            )
        }
        fn stat(&self, _: StatRequest<'_>) -> FsResult<StatResponse> {
            Err(unused())
        }
        fn list(&self, _: ListRequest<'_>) -> FsResult<OpenedDirectoryStream> {
            Err(unused())
        }
        fn open_reader(&self, _: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
            Err(unused())
        }
        fn open_writer(&self, _: OpenWriterRequest<'_>) -> FsResult<OpenedWriter> {
            Err(unused())
        }
        fn create_directory(&self, _: CreateDirectoryRequest<'_>) -> FsResult<CreateDirectoryOutcome> {
            Err(unused())
        }
        fn delete_file(&self, _: DeleteFileRequest<'_>) -> FsResult<DeleteOutcome> {
            Err(unused())
        }
        fn delete_directory(&self, _: DeleteDirectoryRequest<'_>) -> FsResult<DeleteOutcome> {
            Err(unused())
        }
        fn rename(&self, _: RenameRequest<'_>) -> Result<RenameOutcome, SpiRenameFailure> {
            Err(SpiRenameFailure::new(unused(), RenameFailureState::Unchanged))
        }
        fn create_temp_file(&self, _: CreateTempFileRequest) -> FsResult<OpenedTempFile> {
            Err(unused())
        }
        fn create_temp_directory(&self, _: CreateTempDirectoryRequest) -> FsResult<OpenedTempDirectory> {
            Err(unused())
        }
    }
    pub fn sync_file_system() -> Result<FileSystem, FsError> {
        FileSystem::from_spi(SyncPropertiesOnlySpi {
            provider_id: "example",
            scheme: Some("file"),
            limits: FileSystemLimits::unknown(),
            path_constraints: PathConstraints::absolute(),
            path_semantics: PathSemantics::Hierarchical,
        })
    }
}

pub use sync::sync_file_system;

#[cfg(feature = "async")]
mod r#async {
    use qubit_fs::AsyncFileSystem;
    use qubit_fs::FsError;
    use qubit_fs::FsResult;
    use qubit_fs::directory::CreateDirectoryOutcome;
    use qubit_fs::directory::DeleteOutcome;
    use qubit_fs::metadata::FileSystemLimits;
    use qubit_fs::path::PathConstraints;
    use qubit_fs::path::PathSemantics;
    use qubit_fs::rename::RenameFailureState;
    use qubit_fs::rename::RenameOutcome;
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
    use qubit_fs::spi::OpenedAsyncDirectoryStream;
    use qubit_fs::spi::OpenedAsyncReader;
    use qubit_fs::spi::OpenedAsyncTempDirectory;
    use qubit_fs::spi::OpenedAsyncTempFile;
    use qubit_fs::spi::OpenedAsyncWriter;
    use qubit_fs::spi::OpenedDirectoryStream;
    use qubit_fs::spi::OpenedReader;
    use qubit_fs::spi::OpenedTempDirectory;
    use qubit_fs::spi::OpenedTempFile;
    use qubit_fs::spi::OpenedWriter;
    use qubit_fs::spi::ProviderProperties;
    use qubit_fs::spi::RenameRequest;
    use qubit_fs::spi::SpiFuture;
    use qubit_fs::spi::SpiRenameFailure;
    use qubit_fs::spi::StatRequest;
    use qubit_fs::spi::StatResponse;

    use super::common::properties;
    use super::common::unused;

    struct AsyncPropertiesOnlySpi {
        provider_id: &'static str,
        scheme: Option<&'static str>,
        limits: FileSystemLimits,
        path_constraints: PathConstraints,
        path_semantics: PathSemantics,
    }
    impl AsyncFileSystemSpi for AsyncPropertiesOnlySpi {
        fn properties(&self) -> ProviderProperties {
            properties(
                self.provider_id,
                self.scheme,
                self.limits,
                self.path_constraints.clone(),
                self.path_semantics,
            )
        }
        fn stat<'a>(&'a self, _: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
            Box::pin(async { Err(unused()) })
        }
        fn list<'a>(&'a self, _: ListRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncDirectoryStream>> {
            Box::pin(async { Err(unused()) })
        }
        fn open_reader<'a>(&'a self, _: OpenReaderRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncReader>> {
            Box::pin(async { Err(unused()) })
        }
        fn open_writer<'a>(&'a self, _: OpenWriterRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncWriter>> {
            Box::pin(async { Err(unused()) })
        }
        fn create_directory<'a>(
            &'a self,
            _: CreateDirectoryRequest<'a>,
        ) -> SpiFuture<'a, FsResult<CreateDirectoryOutcome>> {
            Box::pin(async { Err(unused()) })
        }
        fn delete_file<'a>(&'a self, _: DeleteFileRequest<'a>) -> SpiFuture<'a, FsResult<DeleteOutcome>> {
            Box::pin(async { Err(unused()) })
        }
        fn delete_directory<'a>(&'a self, _: DeleteDirectoryRequest<'a>) -> SpiFuture<'a, FsResult<DeleteOutcome>> {
            Box::pin(async { Err(unused()) })
        }
        fn rename<'a>(&'a self, _: RenameRequest<'a>) -> SpiFuture<'a, Result<RenameOutcome, SpiRenameFailure>> {
            Box::pin(async { Err(SpiRenameFailure::new(unused(), RenameFailureState::Unchanged)) })
        }
        fn create_temp_file<'a>(&'a self, _: CreateTempFileRequest) -> SpiFuture<'a, FsResult<OpenedAsyncTempFile>> {
            Box::pin(async { Err(unused()) })
        }
        fn create_temp_directory<'a>(
            &'a self,
            _: CreateTempDirectoryRequest,
        ) -> SpiFuture<'a, FsResult<OpenedAsyncTempDirectory>> {
            Box::pin(async { Err(unused()) })
        }
    }
    pub fn async_file_system() -> Result<AsyncFileSystem, FsError> {
        AsyncFileSystem::from_spi(AsyncPropertiesOnlySpi {
            provider_id: "example",
            scheme: Some("file"),
            limits: FileSystemLimits::unknown(),
            path_constraints: PathConstraints::absolute(),
            path_semantics: PathSemantics::Hierarchical,
        })
    }
}

#[cfg(feature = "async")]
pub use r#async::async_file_system;
