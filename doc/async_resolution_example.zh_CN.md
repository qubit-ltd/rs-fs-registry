# 完整异步解析示例

[返回用户指南](user_guide.zh_CN.md) · [English](async_resolution_example.md)

```bash
cargo add qubit-fs@0.3 qubit-spi@0.14
cargo add qubit-fs-registry@0.8 --features async
cargo add futures@0.3
```

示例直接使用文件系统和 SPI API，因此必须显式声明这两个依赖；传递依赖不能代替调用方的直接依赖。文件系统、SPI 和注册表的版本要求以本 package 的 manifest 为准；`futures` 只提供执行器，不是注册表的运行时依赖。这个完整的内存 provider 示例只演示异步创建与解析；由于示例不执行文件 IO，其 SPI 方法会返回不支持错误。

<!-- registry-example: async -->
```rust
use qubit_fs::AsyncFileSystem;
use qubit_fs::FsError;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::directory::CreateDirectoryOutcome;
use qubit_fs::directory::DeleteOutcome;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::SymlinkPolicy;
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
use qubit_fs::spi::ListRequest;
use qubit_fs::spi::OpenReaderRequest;
use qubit_fs::spi::OpenWriterRequest;
use qubit_fs::spi::OpenedAsyncDirectoryStream;
use qubit_fs::spi::OpenedAsyncReader;
use qubit_fs::spi::OpenedAsyncTempDirectory;
use qubit_fs::spi::OpenedAsyncTempFile;
use qubit_fs::spi::OpenedAsyncWriter;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::RenameRequest;
use qubit_fs::spi::SpiFuture;
use qubit_fs::spi::SpiRenameFailure;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;
fn properties(
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
fn unused() -> FsError {
    FsError::new(
        FsErrorKind::UnsupportedOperation,
        FsOperation::Other,
        "unused test operation",
    )
}
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
use qubit_fs_registry::AsyncFileSystemRegistry;
use qubit_fs_registry::AsyncFileSystemResolution;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemSpec;
use qubit_fs::path::ConnectionUri;
use qubit_spi::AsyncServiceProvider;
use qubit_spi::ProviderDescriptor;
use qubit_spi::ProviderId;
use qubit_spi::ProviderMetadata;
use qubit_spi::ProviderFuture;
use qubit_spi::error::ProviderFailure;

// A properties-only in-memory example: it demonstrates resolution, not file IO.
struct MemoryProvider { filesystem: AsyncFileSystem }
impl ProviderMetadata for MemoryProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(ProviderId::new("example").expect("static ID"))
            .with_aliases(["memory"]).expect("static alias")
    }
}
impl AsyncServiceProvider<FileSystemSpec> for MemoryProvider {
    fn create_configured<'a>(&'a self, config: &'a FileSystemConfig)
        -> ProviderFuture<'a, Result<AsyncFileSystemResolution, ProviderFailure<FsError>>>
    {
        Box::pin(async move {
            let uri = config.uri().try_to_uri().map_err(ProviderFailure::invalid_configuration)?;
            if uri.as_str() != "memory:///report.csv" || !config.options().is_empty()
                || !config.metadata().is_empty() || config.credential().is_some() {
                return Err(ProviderFailure::invalid_configuration(FsError::new(
                    FsErrorKind::InvalidOptions, FsOperation::Provider, "unsupported example configuration",
                )));
            }
            AsyncFileSystemResolution::try_new(self.filesystem.clone(),
                Path::parse("/report.csv").expect("static path"), uri)
                .map_err(ProviderFailure::initialization_failed)
        })
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let filesystem = AsyncFileSystem::from_spi(AsyncPropertiesOnlySpi {
        provider_id: "example", scheme: Some("memory"), limits: FileSystemLimits::unknown(),
        path_constraints: PathConstraints::absolute(), path_semantics: PathSemantics::Hierarchical,
    })?;
    let registry = AsyncFileSystemRegistry::default();
    registry.register(MemoryProvider { filesystem })?;
    let future = registry.resolve_uri(ConnectionUri::parse("memory:///report.csv")?);
    drop(registry);
    let resolution = futures::executor::block_on(future)?;
    assert_eq!(resolution.path().as_str(), "/report.csv");
    assert_eq!(resolution.canonical_uri().as_str(), "memory:///report.csv");
    Ok(())
}
```
