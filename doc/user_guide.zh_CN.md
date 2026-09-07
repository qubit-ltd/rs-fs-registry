# qubit-fs-registry 用户手册

[English](user_guide.md) · [README](../README.zh_CN.md) · [API 文档](https://docs.rs/qubit-fs-registry)

## 手册目标与读者

本手册面向需要将 `qubit-fs` 绑定到运行时注册文件系统 provider 的应用和 provider 作者，覆盖当前
`qubit-fs-registry` 0.3 API，包括同步与异步 resolution。

## 概念模型

```text
FileSystemConfig
  ├─ ConnectionUri
  ├─ 可选 ProviderSelection
  ├─ 非敏感 options 与 metadata
  └─ 可选 CredentialRef
          │
          ▼
已注册 provider
          │
          ▼
resolution = filesystem + decoded path + canonical URI
```

`FileSystemRegistry` 创建同步解析结果；`AsyncFileSystemRegistry` 创建异步解析结果。
两者都支持注册提供者、查询描述符与目录大小，以及按相同规则从 URI 选择提供者。
异步配置方法接收配置的所有权，并返回持有解析所需数据的 future。

## 实战场景

某应用在启动时选择本地文件系统提供者，再打开报表 URI，使报表处理代码无需了解工厂实现。
解析成功后，应用得到文件系统门面、逻辑路径和规范 URI。逻辑路径已通过门面声明的
`PathSemantics`、路径形式与大小限制校验。这些校验不调用 `stat`，不访问后端，也不证明目标存在；
需要资源状态时，应用再显式调用 `stat` 等操作。

## 安装与最小配置

```bash
cargo add qubit-fs@0.3 qubit-fs-registry@0.2
cargo add qubit-fs-local@0.2 --features registry
```

需要显式选择提供者或使用底层注册目录类型时，执行 `cargo add qubit-spi@0.11` 添加直接依赖；
本 crate 不重新导出这些属于 SPI 的类型。

## 核心工作流

请在空工作目录中运行；程序会自行创建 `report.csv`，并验证文件大小。

```rust
use qubit_fs::metadata::FileSystemId;
use qubit_fs::path::ConnectionUri;
use qubit_fs_local::LocalFileSystemProvider;
use qubit_fs_local::LocalResourcePolicy;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemRegistry;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::current_dir()?;
    std::fs::write(root.join("report.csv"), b"name,total\nexample,42\n")?;
    let registry = FileSystemRegistry::default();
    registry.register(LocalFileSystemProvider::rooted(
        FileSystemId::new("reports")?, &root, LocalResourcePolicy::unbounded(),
    )?)?;
    let config = FileSystemConfig::new(ConnectionUri::parse("file:///report.csv")?);
    let resolution = registry.resolve_config(&config)?;
    let metadata = resolution.file_system().stat(resolution.path())?;
    assert_eq!(metadata.len(), Some(22));
    assert_eq!(resolution.canonical_uri().as_str(), "file:///report.csv");
    println!("{}: {} bytes", resolution.canonical_uri(), metadata.len().unwrap());
    Ok(())
}
```

下游代码使用解析结果中的 `resolution.file_system()` 和 `resolution.path()` 执行操作，
无需再次解码 URI。

同步与异步解析采用相同的路径校验规则。提供者负责将 URI 解码为逻辑路径；
返回解析结果前，路径必须通过已配置门面的约束校验。

## 进阶用法

### 选择规则的优先级

| 入口 | selection 规则 |
| --- | --- |
| `resolve_config` | config 的 selection；没有时从 URI scheme 构造 named selection。 |
| `resolve_uri` | 从 URI scheme 构造 named selection。 |
| `resolve_selected_config` | 调用方提供的 selection；内嵌不同 selection 时出错。 |
| `resolve_default_config` | 当前 registry 默认 selection；内嵌不同 selection 时出错。 |

因此，`resolve_config` 不会回退到 registry 默认 selection。只有 selection 应由调用方而非 URI
配置决定时，才使用 explicit/default 入口。

当 URI scheme 用作 named selection 时，会通过 `ProviderSelection::named` 校验。selector 必须是
非空 ASCII token，首尾必须为字母或数字，正文只能包含小写 ASCII 字母、数字、`-`、`_`、`.` 和
`+`。selector 解析会去除首尾空白并将 ASCII 字母转为小写；`/`、`:`、空白、非 ASCII 字符及其他
标点会被拒绝。selection 只使用解析后的 scheme，不会把 authority、userinfo、query 或原始 URI
文本重新当作 selector 解析。

registry 默认入口以原子 catalog 快照解析：default selection 与其候选 provider handle 在同一个
快照中读取。并发注册或替换默认 selection 只影响后续快照，不会让一次 default resolution 混用
两个 catalog 版本。
在 SPI 层，这个操作由 `ProviderRegistry::resolve_default_snapshot()` 提供；每次 default
resolution 只调用一次该快照入口。

### 凭据与异步 resolution

`CredentialRef` 只能引用 provider 可识别的来源：`DefaultChain`、profile、环境变量名称或外部
provider ID。不得将 secret material 放入其中。registry 也会在 provider 创建前拒绝相互冲突的
credential 配置。此时返回 `FileSystemRegistryError::CredentialSourceConflict`；当 embedded URI
secret 与外部 `CredentialRef` 占用同一个 slot 时，稳定的 `reason_code()` 为
`credential_source_conflict`。reason code 可安全写入结构化诊断，不包含 URI、reference
或 secret 内容。

对于异步 provider，使用 `AsyncFileSystemRegistry` 注册，并 await 其接收 owned config 的
`resolve_config`、`resolve_uri`、`resolve_selected_config` 或 `resolve_default_config` future。所得
`AsyncFileSystemResolution` 同样包含 filesystem/path/canonical-URI。

## 错误与诊断

registry 操作返回 `FileSystemRegistryResult`，并在 `FileSystemRegistryError` 中保留结构化的注册、
selection、resolution 和 provider 创建诊断。provider 被选中后创建仍可能失败；应检查 typed error，
而非将其替换为笼统消息。registry error 可转换为 `FsError`，同时保留 typed registry error 作为 source。
格式化 registry error 只会在适用时包含安全的 selector 和 provider 上下文；registry 的 `Display` 与 `Debug`
使用 `qubit_redact::Redactor::standard()` 提供的不可变内置策略，不读取或跟随之后替换的进程级
application-default redactor。它们不会递归展开 provider `source()`，也不会把内部 message 作为未脱敏
文本输出。需要结构化错误处理时，应显式使用 typed `Error::source()` 链。

provider 创建失败会保留 SPI 分类：`Unsupported`、`Unavailable`、`InvalidConfiguration` 或
`InitializationFailed`。默认 `FallbackPolicy::OnAbsence` 只在 `Unsupported` 与 `Unavailable` 后继续；
`Never` 始终停止，`OnAnyError` 在所有叶失败后继续。named selection 不会 fallback。provider 尚未被
调用前产生的 resolution error 不会创建 provider attempt。

canonical URI 是选中 provider 针对本次 resolution 生成的无凭据定位结果，不是通用 URI 规范化结果，
也不替代 connection URI。其 scheme 必须由返回的 filesystem facade 声明；authority、path 规范化和
URI 到 path 的语义仍由 provider 负责。不要将其视为跨 provider 的全局 identity。

## 排障

| 现象 | 检查项 |
| --- | --- |
| URI 没有 provider 可解析 | 注册 provider，并使用与其 selection 兼容的 URI scheme。 |
| `resolve_config` 忽略默认值 | 这是预期行为；提供 config selection，或使用 `resolve_default_config`。 |
| 出现 selection conflict | 移除不同的内嵌 selection，或使用由配置决定的 `resolve_config`。 |
| 凭据配置被拒绝 | 仅使用 `CredentialRef` 引用；移除内嵌/query 凭据和 secret-like options。 |
| 无法使用 selection 类型 | 直接添加 `qubit-spi` 依赖。 |

## 限制与最佳实践

- registry 不实现存储后端；已注册 provider 负责创建文件系统门面。
- provider 特有的 URI 解码、路径规则、capability 和 secret 来源解释仍是 provider 的职责。
- 保持配置非敏感。`CredentialRef` 是引用边界，而不是 secret 存储。

## 提供者接入实战

文件系统示例应在空工作目录中运行；程序会自行创建报表文件和子目录。
使用选择规则时，执行 `cargo add qubit-spi@0.11` 添加直接依赖。

### 相同 URI，不同根目录

描述符 ID 标识已注册的工厂，别名用于路由选择；返回门面中的 provider ID 必须与描述符 ID
一致。文件系统 ID 标识配置后的具体文件系统，URI 协议则声明可处理的位置格式，三者不能混为一谈。
下面两个根目录具有相同规范 URI，但内容不同，因此业务代码应始终一起保留门面和路径。

```rust
use qubit_fs::metadata::FileSystemId;
use qubit_fs::path::ConnectionUri;
use qubit_fs::read::ReadOptions;
use qubit_fs_local::LocalFileSystemProvider;
use qubit_fs_local::LocalResourcePolicy;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemRegistry;
use qubit_spi::ProviderDescriptor;
use qubit_spi::ProviderId;
use qubit_spi::ProviderSelection;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::current_dir()?;
    let registry = FileSystemRegistry::default();
    for (id, contents) in [("first", "first report"), ("second", "second report")] {
        let directory = root.join(id);
        std::fs::create_dir_all(&directory)?;
        std::fs::write(directory.join("report.csv"), contents)?;
        registry.register(LocalFileSystemProvider::rooted_with_descriptor(
            ProviderDescriptor::new(ProviderId::new(id)?),
            FileSystemId::new(id)?, &directory, LocalResourcePolicy::unbounded(),
        )?)?;
    }
    let uri = ConnectionUri::parse("file:///report.csv")?;
    let first = registry.resolve_config(&FileSystemConfig::new(uri.clone())
        .with_selection(ProviderSelection::named("first")?))?;
    let second = registry.resolve_config(&FileSystemConfig::new(uri)
        .with_selection(ProviderSelection::named("second")?))?;
    assert_eq!(first.canonical_uri(), second.canonical_uri());
    assert_ne!(first.file_system().properties().info().id(), second.file_system().properties().info().id());
    assert_eq!(first.file_system().read_all(first.path(), ReadOptions::default(), 64)?, b"first report");
    assert_eq!(second.file_system().read_all(second.path(), ReadOptions::default(), 64)?, b"second report");
    Ok(())
}
```

### 实现提供者

接入点是 `ProviderMetadata` 和 `ServiceProvider<FileSystemSpec>`。本例组合已有本地提供者，
复用其 URI 解码、凭据检查和路径规则，避免重复实现百分号解码。
新后端应先构造门面，再调用 `FileSystemResolution::try_new(facade, decoded_path, canonical_uri)`。
构造器校验路径语义与限制，并要求门面声明规范 URI 的协议；空协议集合也会失败。
注册表随后校验提供者身份。

```rust
use qubit_fs::FsError;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::path::ConnectionUri;
use qubit_fs_local::LocalFileSystemProvider;
use qubit_fs_local::LocalResourcePolicy;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemRegistry;
use qubit_fs_registry::FileSystemResolution;
use qubit_fs_registry::FileSystemSpec;
use qubit_spi::ProviderDescriptor;
use qubit_spi::ProviderId;
use qubit_spi::ProviderMetadata;
use qubit_spi::ServiceProvider;
use qubit_spi::error::ProviderFailure;

struct ReportProvider { local: LocalFileSystemProvider }
impl ProviderMetadata for ReportProvider {
    fn descriptor(&self) -> ProviderDescriptor { self.local.descriptor() }
}
impl ServiceProvider<FileSystemSpec> for ReportProvider {
    fn create_configured(&self, config: &FileSystemConfig)
        -> Result<FileSystemResolution, ProviderFailure<FsError>>
    {
        // Reuse the local adapter's credential checks and percent-decoding rules.
        self.local.create_configured(config)
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = ReportProvider { local: LocalFileSystemProvider::rooted_with_descriptor(
        ProviderDescriptor::new(ProviderId::new("report-storage")?).with_aliases(["file"])?,
        FileSystemId::new("report-root")?, &std::env::current_dir()?, LocalResourcePolicy::unbounded(),
    )? };
    let registry = FileSystemRegistry::default();
    registry.register(provider)?;
    let resolution = registry.resolve_uri(&ConnectionUri::parse("file:///not-created.csv")?)?;
    assert_eq!(resolution.file_system().properties().info().provider_id(), "report-storage");
    // Resolution validates properties; it does not require the file to exist.
    assert_eq!(resolution.path().as_str(), "/not-created.csv");
    Ok(())
}
```

### 观察回退决策

以下两个提供者有意返回失败，使策略差异可以直接验证。`OnAbsence` 遇到无效配置就停止；
`OnAnyError` 会继续调用第二个提供者。按实际调用顺序读取 attempts，并检查决定最终结果的失败，
不要匹配显示字符串。后续提供者若成功，则直接返回其解析结果，不返回错误聚合。

```rust
use qubit_fs::FsError;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::path::ConnectionUri;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemRegistry;
use qubit_fs_registry::FileSystemRegistryError;
use qubit_fs_registry::FileSystemResolution;
use qubit_fs_registry::FileSystemSpec;
use qubit_spi::FallbackPolicy;
use qubit_spi::ProviderDescriptor;
use qubit_spi::ProviderId;
use qubit_spi::ProviderMetadata;
use qubit_spi::ProviderSelection;
use qubit_spi::ServiceProvider;
use qubit_spi::error::ProviderFailure;
use qubit_spi::error::ProviderFailureKind;

struct Rejected { id: &'static str, invalid: bool }
impl ProviderMetadata for Rejected {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(ProviderId::new(self.id).expect("static ID"))
    }
}
impl ServiceProvider<FileSystemSpec> for Rejected {
    fn create_configured(&self, _: &FileSystemConfig)
        -> Result<FileSystemResolution, ProviderFailure<FsError>>
    {
        if self.invalid {
            Err(ProviderFailure::invalid_configuration(FsError::new(
                FsErrorKind::InvalidOptions, FsOperation::Provider, "invalid provider options",
            )))
        } else {
            Err(ProviderFailure::unavailable(FsError::new(
                FsErrorKind::ProviderUnavailable, FsOperation::Provider, "provider unavailable",
            )))
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = FileSystemRegistry::default();
    registry.register(Rejected { id: "first", invalid: true })?;
    registry.register(Rejected { id: "second", invalid: false })?;
    let config = FileSystemConfig::new(ConnectionUri::parse("file:///report.csv")?);
    for (policy, count, decisive) in [
        (FallbackPolicy::OnAbsence, 1, "first"),
        (FallbackPolicy::OnAnyError, 2, "second"),
    ] {
        let selection = ProviderSelection::chain(["first", "second"])?.with_fallback_policy(policy);
        let error = registry.resolve_selected_config(&selection, &config).expect_err("fixture providers fail");
        let FileSystemRegistryError::Creation(creation) = error else { panic!("creation aggregate expected") };
        assert_eq!(creation.attempts().len(), count);
        assert_eq!(creation.attempts()[0].failure().kind(), ProviderFailureKind::InvalidConfiguration);
        assert_eq!(creation.decisive_attempt().provider_id().as_str(), decisive);
    }
    Ok(())
}
```

### 引用凭据来源

URI 中只有用户名时不算内嵌秘密；密码和敏感查询参数会与外部引用冲突。
下面的程序无需连接存储服务，就能验证准确的错误边界。

```rust
use qubit_fs::path::ConnectionUri;
use qubit_fs_registry::CredentialRef;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemRegistry;
use qubit_fs_registry::FileSystemRegistryError;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = FileSystemRegistry::default();
    let config = FileSystemConfig::new(ConnectionUri::parse("s3://user:example-secret@bucket/key")?)
        .with_credential(CredentialRef::DefaultChain);
    let error = registry.resolve_config(&config).expect_err("conflicting sources");
    assert!(matches!(error, FileSystemRegistryError::CredentialSourceConflict));
    assert_eq!(error.reason_code(), "credential_source_conflict");
    assert!(!format!("{error:?}").contains("example-secret"));
    let username = FileSystemConfig::new(ConnectionUri::parse("s3://user@bucket/key")?)
        .with_credential(CredentialRef::DefaultChain);
    // Credentials are valid; an empty registry now fails in the resolution stage.
    assert!(matches!(registry.resolve_config(&username), Err(FileSystemRegistryError::Resolution(_))));
    Ok(())
}
```

### 异步解析的所有权

```bash
cargo add qubit-fs-registry@0.2 --features async
cargo add futures@0.3
```

`futures` 只为本例提供执行器，不是注册表的运行时依赖。上面的本地提供者只实现同步接口。
下面是完整的内存示例，仅提供属性用于演示异步创建契约；SPI 操作有意返回不支持，
因为解析过程无需文件 IO。返回 future 前已取得目录快照，首次轮询才开始创建提供者，
注册表销毁后 future 仍可完成。注册表克隆共享注册信息和默认值，后续修改只影响后续快照。

<!-- registry-example: async -->
```rust
use qubit_fs::AsyncFileSystem;
use qubit_fs::FileSystem;
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
use qubit_fs::path::Uri;
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

初始默认选择为 `auto()`，策略为 `OnAbsence`，但不会改变 config/URI 入口的路由规则。
注册和替换默认值是同步操作，提供者工作在目录锁之外执行。注册表不自动把阻塞操作转换成异步，
也没有配置后文件系统缓存。外部凭据引用不能单独证明资源可共享；提供者必须先明确主体、权限范围
和凭据轮换规则，才能安全复用认证资源。

## 延伸阅读

- [README](../README.zh_CN.md)
- [English user guide](user_guide.md)
- [API 文档](https://docs.rs/qubit-fs-registry)

- [Design / 设计](file_system_registry_design.md) · [中文设计](file_system_registry_design.zh_CN.md)
- [Migration / 迁移](registry_contract_migration.md) · [中文迁移](registry_contract_migration.zh_CN.md)
