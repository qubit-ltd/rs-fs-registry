# Qubit FS Registry 设计

> 状态：已批准并已实现，已按最终版 `qubit-fs` 的 URI、Path 与门面边界复核。
> 本文定义 `qubit-fs-registry` 在 filesystem 门面/SPI 重构后的长期边界。

## 1. 定位

`qubit-fs-registry` 负责运行时 provider 注册、selection、完整配置和 URI
resolution。它是应用组装层，不是 filesystem operation 实现层，也不执行自动
provider discovery 或 credential value resolution。

```text
FileSystemConfig / ConnectionUri
          │
          ▼
FileSystemRegistry
  ├─ provider selection/fallback
  ├─ credential source conflict validation
  ├─ provider-specific URI decode
  └─ configured filesystem creation
          │
          ▼
FileSystemResolution
  ├─ FileSystem
  ├─ Path
  └─ canonical Uri
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
- 把 credential value 存入 canonical `Uri`、`Path`、properties 或普通错误显示；
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
    path: Path,
    canonical_uri: Uri,
}

#[derive(Clone)]
pub struct AsyncFileSystemResolution {
    file_system: AsyncFileSystem,
    path: Path,
    canonical_uri: Uri,
}
```

公开 API：

```rust
impl FileSystemResolution {
    pub fn try_new(
        file_system: FileSystem,
        path: Path,
        canonical_uri: Uri,
    ) -> Result<Self, FsError>;

    pub fn file_system(&self) -> &FileSystem;
    pub fn path(&self) -> &Path;
    pub fn canonical_uri(&self) -> &Uri;
    pub fn into_parts(self) -> (FileSystem, Path, Uri);
}
```

异步 resolution 提供对称 getter 和 `into_parts`，但不构造
`AsyncFileResource`。

Public resolution 不使用泛型，以免再次允许 `Arc<dyn FileSystemSpi>` 或任意 provider
类型穿过 registry 边界。若实现需要共享代码，只能使用私有 generic core。

## 5. Resolution 不变量

Provider 构造 resolution 时必须同时满足：

1. `file_system.properties().info().provider_id()` 与选中的 provider 一致；
2. decoded `Path` 符合 filesystem 的 `PathSemantics`、limits 与
   `PathConstraints`；
3. canonical URI 的 scheme/authority 与 provider resolution 一致；
4. canonical URI 不含 credential；
5. URI path 与 decoded path 的 provider-specific 关系由 provider 明确建立；
6. configuration 中的显式 selection 不与外部 selection 冲突；
7. resolution 不保存 transient credential value。

同步和异步 resolution 都使用 `try_new`，不能以 infallible `new` 接受互相矛盾的三个
值。

`try_new` 可以通用复核 path semantics/limits/constraints 与 canonical URI 的
secret-free 结构，但不能仅凭三个值复核 provider identity 或重新推导所有
provider-specific URI ↔ Path 关系。provider identity 由 registry 的 adapter 在
construction boundary 复核；URI ↔ Path 关系由选中的 provider 建立。registry 还负责
保证 resolution 来自该 provider 且没有在之后替换其中任一部分。

Registry 不提供 `resource()` convenience。若调用者只需要 filesystem，可以显式使用
resolution 的 clone getter；需要 URI 定位结果时保留完整 resolution，避免再次引入
一套绑定路径 convenience API。

## 6. 同步 Registry API

`FileSystemRegistry` 继续组织所有同步 registry 能力：

```rust
impl FileSystemRegistry {
    pub fn register<P>(&self, provider: P) -> FileSystemRegistryResult<()>;

    pub fn resolve_config(
        &self,
        config: &FileSystemConfig,
    ) -> FileSystemRegistryResult<FileSystemResolution>;
}
```

URI、selected config 和 default config convenience 仍作为 registry 的 inherent
methods 组织，不增加 free function。

Registry 不提供会静默丢弃 resolution path/canonical URI 的 `file_system(config)`
捷径。调用者确实只需要 configured filesystem 时，显式
`registry.resolve_config(config)?.file_system().clone()`，使丢弃定位信息在调用点
可见。

## 7. 异步 Registry API

`AsyncFileSystemRegistry` 使用同名 inherent method，返回标准 `Future`：

```rust
pub fn resolve_config(
    &self,
    config: FileSystemConfig,
) -> impl Future<
    Output = FileSystemRegistryResult<AsyncFileSystemResolution>,
> + Send + 'static;

let resolution = registry.resolve_config(config).await?;
let file_system = resolution.file_system().clone();
```

这里不使用自定义 future 实现，也不为了语法统一强行写成借用 `&self` / `&config` 的
`async fn`。方法在返回前同步取得 provider/catalog snapshot 并把 owned config 移入
future，因此可以兑现以下契约：

- 拥有完成 resolution 所需的 config 和 provider snapshot；
- 可以安全地比 registry handle 和调用参数活得更久；
- 等待 provider 时不持有 catalog write lock；
- 不把同步 provider 作为隐式 blocking fallback；
- 保持 `Send` 约束与 crate 现有 runtime-neutral 目标一致。

类型名称已经表达异步，不给方法附加 `_async`。其他异步 resolution convenience 采用
相同 owned-input 模式；`ConnectionUri` 在入口被消费，避免 secret-bearing 输入借用
跨越不受控生命周期。

## 8. Selection 与 fallback

Selection precedence 保持确定：

1. 显式传入的 selection；
2. `FileSystemConfig` 内嵌 selection；
3. URI scheme 派生的 named selection；
4. 只有明确使用 default/auto API 时才考虑 registry default 或 provider priority。

冲突的显式 selection 必须在 provider creation 前失败。

从 URI 派生 selection 时只读取 `ConnectionUri` 已解析并校验的 scheme component，
不暴露或重新解析原始 URI 文本，也不把 authority、userinfo 或 query 放入 selection
diagnostics。

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

本轮重构不新增 configured filesystem cache；每次 resolution 是否复用 provider
内部资源仍由 provider 自己的既有契约决定。以下规则是未来引入 registry/provider
configured filesystem cache 的硬前置条件，而不是本轮顺带增加的新功能。

Cache key 必须包含会影响 configured filesystem identity、authority 或 capabilities
的非敏感配置维度。Credential value 不能出现在可显示的 cache key 中。

凭证隔离是 cache correctness，而不只是日志脱敏：

- authority userinfo、敏感 query 或其他 inline/embedded credential 存在时默认禁止
  configured filesystem cache；
- 使用 `CredentialRef` 时，只有 provider 能提供稳定、非敏感且足以区分 principal /
  scope / credential version 或 refresh epoch 的 cache identity，才允许共享；
- cache key 可以包含 credential reference 的安全 identity/version，不能包含 secret
  value，也不能用低熵 secret 的 hash 伪装成非敏感 key；
- provider 无法证明两个 resolution 可以安全复用同一认证上下文时必须返回
  uncacheable；
- credential rotation、revocation 或 scope change 必须使旧 cache entry 不再用于新
  resolution。

具体 cache 机制可以由 registry 或 provider 实现，但必须先得到 provider 明确的
cache eligibility/key；registry 不能只排除 credential value 后自行猜测其余维度。

## 10. Configuration 与 credential

`FileSystemConfig` 包含：

- `ConnectionUri`；
- 可选 `ProviderSelection`；
- 非敏感 provider options；
- 可选 `CredentialRef`；
- 必要的非敏感 metadata。

普通 provider options 不得承载 secret。需要 secret 的 provider 必须使用
`ConnectionUri` 的受控输入或 `CredentialRef`；不能把 token/password 塞进任意
字符串 map 后绕过 redaction 与 cache policy。

同一个 credential slot 同时由 embedded URI secret 与 `CredentialRef` 提供时必须在
provider creation 前报 configuration conflict，不能静默选择优先级。当前 registry
不提供 provider-specific 例外；如果未来需要组合不同 credential role，应先定义并
验证显式的 credential policy，再放宽这一不变量。

`CredentialRef` 只描述外部 secret 的引用，不承载 token、password、private key 等
secret value。`ConnectionUri` 可以在受控入口承载 userinfo password 或敏感 query，
但其普通 `Display`/`Debug` 必须委托 `qubit-redact`，且不能实现会静默暴露明文的
`Deref<str>`、`AsRef<str>` 或普通 raw getter。

Provider creation 可以通过名称醒目的受控 API 临时读取原始 URI/secret，并在返回
resolution 前产生 secret-free canonical `Uri`。在此过程中：

- secret 不进入 `FileSystemProperties`；
- secret 不进入 canonical URI；
- secret 不进入 registry error 的普通格式化；
- `FileSystemConfig` clone 保留相同的受保护值，`Display`/`Debug` 遵守 redaction
  规则；
- `FileSystemConfig` 不派生会输出明文 `ConnectionUri` 的普通 serialization；如需
  持久化 secret-bearing 配置，必须使用名称醒目的显式导出边界；
- registry error 的手写 `Display`/`Debug` 不展开 provider source；
- typed provider source 可以通过显式 `Error::source()` 链访问，但不能因 derive 或
  diagnostics 自动格式化而泄漏 secret。

`ConnectionUri` 不能直接作为 cache key；canonical `Uri` 只有在 provider 移除
credential-like query、password 及其他 secret 后才能进入 resolution 或非敏感 key。

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
2. 在受控边界读取 `ConnectionUri` 并拒绝 local provider 不支持的 credential；
3. 解码 provider URI path 为 logical `Path`；
4. 调用 `LocalFileSystems::host` 或 `rooted`；
5. 生成 secret-free canonical `Uri`；
6. 返回 `FileSystemResolution`。

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
- resolution 的 `Path` 满足 semantics、limits 与 constraints；
- public API 不再出现 `FileResource` / `AsyncFileResource`；
- `ConnectionUri` 的 `Display` / `Debug` 脱敏；
- `FileSystemConfig` 不通过普通 serialization 泄漏 connection secret；
- embedded secret 与同 slot `CredentialRef` 冲突在 provider creation 前失败；
- canonical `Uri` 无 password、credential-like query 或其他 secret；
- 本轮 registry 不产生 configured filesystem cache；
- 若未来启用 cache，inline credential 默认不可缓存；
- 若未来启用 cache，不同 credential identity/version 不复用 configured filesystem，
  rotation/revocation 使旧 entry 失效或不可选；
- 每种 selection precedence 与冲突；
- 每种 fallback failure class；
- ordered aggregated failures 与 decisive failure；
- registry clone 的共享 catalog 行为；
- provider snapshot 不受并发注册影响；
- async await 不持有 registry lock；
- async future 可以 outlive registry/config 参数；
- sync/async concrete resolution 对称；
- registry error 到 `FsError` 的分类、context 和 source 保留。
