# Qubit FS Registry 设计

> 状态：已批准的目标设计。本文定义 `qubit-fs-registry` 在 filesystem 门面/SPI
> 重构后的长期边界；当前实现迁移前可能与本文不同。

## 1. 定位

`qubit-fs-registry` 负责运行时 provider discovery、selection、完整配置和 URI
resolution。它是应用组装层，不是 filesystem operation 实现层。

```text
FileSystemConfig / FsUri
          │
          ▼
FileSystemRegistry
  ├─ provider selection/fallback
  ├─ credential reference resolution
  ├─ provider-specific URI decode
  └─ configured filesystem creation
          │
          ▼
FileSystemResolution
  ├─ FileSystem
  ├─ FsPath
  └─ canonical FsUri
```

Registry 只能返回 `FileSystem` / `AsyncFileSystem` 门面，不能把 operation SPI 暴露给
应用。

## 2. 目标与非目标

目标：

1. 支持同步与异步 provider 的注册、选择和创建；
2. 保留 URI 与完整 config 的 provider-specific 解释权；
3. 返回具体门面，确保所有操作经过 `qubit-fs` 公共契约；
4. resolution 同时保留 decoded path 和 credential-free canonical URI；
5. selection、fallback、错误聚合和并发行为确定且可测试；
6. registry clone 共享 catalog，但 provider resolution 使用一致的快照；
7. 与 `qubit-spi` 组合，不重新实现通用 service registry。

非目标：

- 定义 filesystem operation；
- 持有或公开 `FileSystemSpi`；
- 重新校验每个文件操作的 options；
- 在 registry 中实现 provider-native path 算法；
- 把 credential value 存入 `FsUri`、`FileLocation` 或普通错误显示；
- 为同步 provider 自动构造异步 wrapper。

## 3. Provider 与 SPI 命名

两个扩展点具有不同职责：

| 类型 | 所属 crate | 职责 |
| --- | --- | --- |
| `FileSystemSpi` | `qubit-fs` | configured filesystem 的操作原语 |
| `FileSystemProvider` | `qubit-fs-registry` | 配置、URI 解码和 filesystem 创建 |

`FileSystemProvider` 名称继续保留给 registry 工厂，进一步说明 operation trait 使用
`Spi` 而不是 `Provider` 的必要性。

Provider 通过 `qubit-spi` 的 `ServiceProvider<FileSystemSpec>` 或异步对应接口接入。
Provider creation 的最终产物必须包含由 `FileSystem::from_spi` 构造的门面。

## 4. Concrete resolution

删除公开泛型 `FileSystemResolution<F: ?Sized>`，改成两个具体类型：

```rust
#[derive(Clone)]
pub struct FileSystemResolution {
    file_system: FileSystem,
    path: FsPath,
    canonical_uri: FsUri,
}

#[derive(Clone)]
pub struct AsyncFileSystemResolution {
    file_system: AsyncFileSystem,
    path: FsPath,
    canonical_uri: FsUri,
}
```

公开 API：

```rust
impl FileSystemResolution {
    pub fn try_new(
        file_system: FileSystem,
        path: FsPath,
        canonical_uri: FsUri,
    ) -> FsResult<Self>;

    pub fn file_system(&self) -> &FileSystem;
    pub fn path(&self) -> &FsPath;
    pub fn canonical_uri(&self) -> &FsUri;
    pub fn resource(&self) -> FsResult<FileResource>;
    pub fn into_parts(self) -> (FileSystem, FsPath, FsUri);
}
```

异步 resolution 对称返回 `AsyncFileResource`。

Public resolution 不使用泛型，以免再次允许 `Arc<dyn FileSystemSpi>` 或任意 provider
类型穿过 registry 边界。若实现需要共享代码，只能使用私有 generic core。

## 5. Resolution 不变量

Provider 构造 resolution 时必须同时满足：

1. `file_system.properties().info().provider_id()` 与选中的 provider 一致；
2. decoded `FsPath` 符合 filesystem 的 `PathSemantics` 与 limits；
3. canonical URI 的 scheme/authority 与 provider resolution 一致；
4. canonical URI 不含 credential；
5. URI path 与 decoded path 的关系由 provider 明确建立；
6. configuration 中的显式 selection 不与外部 selection 冲突；
7. resolution 不保存 transient credential value。

同步和异步 resolution 都使用 `try_new`，不能以 infallible `new` 接受互相矛盾的三个
值。

`resource()` 通过 `FileSystem::resource_at` 创建 resource。它不直接调用
`FileResource` 内部构造器，也不替换 resource 所持有的 filesystem。Registry/provider
负责证明 canonical URI 与 decoded path 的 provider-specific 对应关系；门面复核
filesystem identity、path semantics、limits 和 URI 的非敏感结构。

## 6. 同步 Registry API

`FileSystemRegistry` 继续组织所有同步 registry 能力：

```rust
impl FileSystemRegistry {
    pub fn register<P>(&self, provider: P) -> FileSystemRegistryResult<()>;

    pub fn resolve_config(
        &self,
        config: &FileSystemConfig,
    ) -> FileSystemRegistryResult<FileSystemResolution>;

    pub fn file_system(
        &self,
        config: &FileSystemConfig,
    ) -> FileSystemRegistryResult<FileSystem>;

    pub fn resource(
        &self,
        config: &FileSystemConfig,
    ) -> FileSystemRegistryResult<FileResource>;
}
```

URI、selected config 和 default config convenience 仍作为 registry 的 inherent
methods 组织，不增加 free function。

`file_system` 从 resolution 取出并返回可克隆门面；不再返回
`Arc<dyn FileSystem>`。

## 7. 异步 Registry API

`AsyncFileSystemRegistry` 使用同名 inherent `async fn`：

```rust
let resolution = registry.resolve_config(&config).await?;
let resource = registry.resource(&config).await?;
```

类型名称已经表达异步，不再给方法附加 `_async`。Future 必须：

- 拥有完成 resolution 所需的 config 和 provider snapshot；
- 可以安全地比 registry handle 和调用参数活得更久；
- 等待 provider 时不持有 catalog write lock；
- 不把同步 provider 作为隐式 blocking fallback；
- 保持 `Send` 约束与 crate 现有 runtime-neutral 目标一致。

## 8. Selection 与 fallback

Selection precedence 保持确定：

1. 显式传入的 selection；
2. `FileSystemConfig` 内嵌 selection；
3. URI scheme 派生的 named selection；
4. 只有明确使用 default/auto API 时才考虑 registry default 或 provider priority。

冲突的显式 selection 必须在 provider creation 前失败。

Fallback policy 必须区分：

- provider 未注册；
- provider 明确不适用于 config；
- provider configuration invalid；
- credential resolution failure；
- provider unavailable；
- provider construction failure。

只有 policy 明确允许的 failure class 才进入下一个 provider。错误聚合保留尝试顺序、
provider identity 和决定停止 fallback 的 decisive failure。

本次重构不改变已有 selection/fallback 业务规则，只改变成功结果中的 filesystem
类型。

## 9. Catalog、缓存与所有权

Registry clone 共享 provider catalog 和 default selection。注册变更对共享 clone
可见。

Provider resolution 使用 point-in-time provider snapshot：

- 解析期间注册表变化不改变当前尝试链；
- async await 期间不持有 registry lock；
- provider 实例的生命周期由 snapshot 保证；
- provider 创建出的 `FileSystem` 可以独立于 registry 存活。

若 registry 或 provider 实现 configured filesystem cache，缓存值必须是
`FileSystem` / `AsyncFileSystem` 门面，而不是 operation SPI。克隆门面保留同一 SPI
和 properties snapshot。

Cache key 必须包含会影响 configured filesystem identity、authority 或 capabilities
的非敏感配置维度。Credential value 不能出现在可显示的 cache key 中。

## 10. Configuration 与 credential

`FileSystemConfig` 包含：

- `FsUri`；
- 可选 `ProviderSelection`；
- provider options；
- 可选 `CredentialRef`；
- 必要的非敏感 metadata。

`CredentialRef` 只描述外部 secret 的引用，不承载 token、password、private key 等
secret value。Provider creation 可以临时解析 secret，但：

- secret 不进入 `FileSystemProperties`；
- secret 不进入 canonical URI；
- secret 不进入 registry error 的普通格式化；
- config clone/debug 遵守现有 redaction 规则；
- provider source error 只通过显式 `Error::source()` 链访问。

## 11. Error 模型

`FileSystemRegistryError` 保留以下类别：

- registration conflict；
- invalid selection；
- provider unavailable；
- invalid configuration；
- credential resolution；
- resolution failure；
- provider creation；
- exhausted fallback。

错误必须携带：

- selection/config context 的非敏感部分；
- provider id；
- 有序 provider failures；
- decisive failure；
- typed source。

转换为 `FsError` 时：

- 使用合适的 `FsOperation`；
- 保留 typed registry error 为 source；
- 不把所有错误压成 `Other`；
- 不展开可能含敏感内容的 provider source；
- resolution 已有 path/URI 时只附加安全表示。

Operation SPI 的 `ProviderContractViolation` 不由 registry 改写为 registry failure。

## 12. 与 `qubit-spi` 的边界

`qubit-spi` 继续负责：

- provider metadata；
- provider id、aliases、priority；
- registration；
- selection；
- sync/async service provider 生命周期；
- fallback policy 支撑。

`FileSystemSpec` 的 service output 改为 concrete resolution：

```rust
impl SyncServiceSpec for FileSystemSpec {
    type Output = FileSystemResolution;
}

impl AsyncServiceSpec for FileSystemSpec {
    type Output = AsyncFileSystemResolution;
}
```

本次设计不要求重构 `qubit-spi` 的通用 catalog 或其他 service 类型。

## 13. Local provider 集成

`qubit-fs-local` 的 `LocalFileSystemProvider`：

1. 校验 `file:` config；
2. 解码 provider URI path；
3. 调用 `LocalFileSystems::host` 或 `rooted`；
4. 生成 secret-free canonical URI；
5. 返回 `FileSystemResolution`。

Registry 不直接构造 `LocalFileSystemSpi`，也不调用 `qubit-local-files`。

## 14. 模块组织

```text
src/
├── file_system_config.rs
├── file_system_resolution.rs
├── async_file_system_resolution.rs
├── file_system_registry.rs
├── async_file_system_registry.rs
├── file_system_provider.rs
├── async_file_system_provider.rs
├── file_system_spec.rs
├── file_system_registry_error.rs
└── internal/
    └── registry_support.rs
```

同步与异步公开类型分开；共享 selection、validation 和 error aggregation 可以放入私有
helper 类型。Helper 由类型方法组织，不暴露 public free function。

## 15. 验证策略

测试至少覆盖：

- resolution 不接受 filesystem/provider/path/URI 不一致；
- public API 不再出现 `Arc<dyn FileSystem>` 或 operation SPI；
- `file_system` clone 保留同一 properties snapshot；
- `resource` 绑定 resolution 中的同一门面；
- canonical URI 去敏；
- 每种 selection precedence 与冲突；
- 每种 fallback failure class；
- ordered aggregated failures 与 decisive failure；
- registry clone 的共享 catalog 行为；
- provider snapshot 不受并发注册影响；
- async await 不持有 registry lock；
- async future 可以 outlive registry/config 参数；
- sync/async concrete resolution 对称；
- registry error 到 `FsError` 的分类、context 和 source 保留。
