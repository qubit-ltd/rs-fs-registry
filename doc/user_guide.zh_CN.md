# qubit-fs-registry 用户手册

[English](user_guide.md) · [README](../README.zh_CN.md) · [API 文档](https://docs.rs/qubit-fs-registry)

## 手册目标与读者

本手册面向需要将 `qubit-fs` 绑定到运行时注册文件系统 provider 的应用和 provider 作者，覆盖当前
`qubit-fs-registry` 0.1 API，包括同步与异步 resolution。

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

`FileSystemRegistry` 创建同步 resolution；`AsyncFileSystemRegistry` 创建异步 resolution。
两者都提供注册、descriptor、catalog 大小、URI 便捷方法和相同的 selection 规则。异步配置方法
接收配置的所有权，并返回用于 resolution 的 future。

## 贯穿场景

某应用在启动时选择本地文件系统 provider，再打开一个报表 URI，而不让报表处理代码耦合于
provider factory。成功标准是边界返回可用于 `stat` 的文件系统与逻辑路径，并可取得 canonical URI
用于安全标识。

## 安装与最小配置

```bash
cargo add qubit-fs qubit-fs-registry
cargo add qubit-fs-local --features registry
```

需要创建显式 SPI selection 或使用底层 provider catalog 类型的 provider crate，必须直接依赖
`qubit-spi`；本 crate 不重新导出这些 SPI 所有的类型。

## 核心工作流

```rust
use qubit_fs::{ConnectionUri, FsResult};
use qubit_fs_local::LocalFileSystemProvider;
use qubit_fs_registry::{FileSystemConfig, FileSystemRegistry};

fn inspect_report() -> FsResult<()> {
    let registry = FileSystemRegistry::default();
    registry.register(LocalFileSystemProvider::new())?;

    let config = FileSystemConfig::new(ConnectionUri::parse("file:///tmp/report.csv")?);
    let resolution = registry.resolve_config(&config)?;
    let metadata = resolution.file_system().stat(resolution.path())?;
    println!("{metadata:?} at {}", resolution.canonical_uri());
    Ok(())
}
```

应将 URI 与配置保留在 resolution 边界。下游代码使用 `resolution.file_system()` 和
`resolution.path()`，而不是再次解码 URI。

## 进阶用法

### Selection 优先级

| 入口 | selection 规则 |
| --- | --- |
| `resolve_config` | config 的 selection；没有时从 URI scheme 构造 named selection。 |
| `resolve_uri` | 从 URI scheme 构造 named selection。 |
| `resolve_selected_config` | 调用方提供的 selection；内嵌不同 selection 时出错。 |
| `resolve_default_config` | 当前 registry 默认 selection；内嵌不同 selection 时出错。 |

因此，`resolve_config` 不会回退到 registry 默认 selection。只有 selection 应由调用方而非 URI
配置决定时，才使用 explicit/default 入口。

### 凭据与异步 resolution

`CredentialRef` 只能引用 provider 可识别的来源：`DefaultChain`、profile、环境变量名称或外部
provider ID。不得将 secret material 放入其中。registry 也会在 provider 创建前拒绝相互冲突的
credential 配置。

对于异步 provider，使用 `AsyncFileSystemRegistry` 注册，并 await 其接收 owned config 的
`resolve_config`、`resolve_uri`、`resolve_selected_config` 或 `resolve_default_config` future。所得
`AsyncFileSystemResolution` 同样包含 filesystem/path/canonical-URI。

## 错误与诊断

registry 操作返回 `FileSystemRegistryResult`，并在 `FileSystemRegistryError` 中保留结构化的注册、
selection、resolution 和 provider 创建诊断。provider 被选中后创建仍可能失败；应检查 typed error，
而非将其替换为笼统消息。registry error 可转换为 `FsError`，同时保留 typed registry error 作为 source。

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

## 延伸阅读

- [README](../README.zh_CN.md)
- [English user guide](user_guide.md)
- [API 文档](https://docs.rs/qubit-fs-registry)
