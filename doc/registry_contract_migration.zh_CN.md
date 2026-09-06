# Registry 合约迁移说明

本文面向从旧版 `qubit-fs-registry` 迁移的应用和 provider 作者，记录 filesystem
facade/SPI 重构后的可观察合约。它补充[中文用户手册](user_guide.zh_CN.md)，重点
说明错误、快照、fallback、canonical URI 与 scheme selection 的兼容边界。

## 1. 凭据来源冲突

同一个 credential slot 不能同时由 URI 中的 embedded credential 和
`CredentialRef` 提供。registry 在调用 provider 之前检查这一不变量，并返回：

```text
FileSystemRegistryError::CredentialSourceConflict
```

`reason_code()` 固定返回 `credential_source_conflict`。它只表达冲突类别，不包含 URI、credential
reference 或 secret；此变体的 `Error::source()` 为 `None`。迁移时应按变体或
reason code 处理，而不要依赖旧版 `InvalidConfiguration.message` 的文字内容。

`CredentialRef` 仍然只是 provider 可识别的外部来源引用，不能保存 token、password、
private key 或其他 secret。

## 2. 安全诊断与错误链

`FileSystemRegistryError` 的 `Display` 和 `Debug` 只生成安全诊断：

- 不递归展开 provider 的 typed `source()`；
- 不把内部 `message` 当作未经处理的普通文本输出，message 按敏感字段交给 redactor；
- 保留有限的 selector、provider identity 和稳定 reason code 上下文；
- 需要程序化处理因果链时，调用方显式使用 `Error::source()`。

registry 格式化每次使用 `qubit_redact::Redactor::standard()` 的不可变内置策略快照。
它不读取，也不跟随之后替换的进程级 application-default redactor。应用不能通过替换
全局默认 redactor 改变 registry error 的 `Display`/`Debug` 行为；需要更严格的展示时，
调用方应在自己的输出边界使用合适的 redactor。

## 3. default resolution 的原子快照

registry clone 共享 provider catalog 和默认 selection。default resolution 会在同一个
catalog read snapshot 中读取 default selection 并复制对应的候选 provider handle，随后
在释放锁后创建 provider。一次 resolution 不会把旧 selection 与新注册表中的候选集合
拼在一起。

因此，注册 provider 或调用 `set_default_selection` 只影响之后取得的快照；已经取得的
同步或异步 snapshot 保留自己的候选集合，即使 registry 被 clone、修改或 drop 也可以
继续完成。若需要观察新注册或新默认值，必须重新调用 resolution 入口。

## 4. provider failure 与 fallback

provider 在创建阶段返回的叶失败按 `ProviderFailureKind` 分类：

| 分类 | 含义 | 默认 `OnAbsence` 是否继续 |
| --- | --- | --- |
| `Unsupported` | provider 不支持当前请求 | 是 |
| `Unavailable` | provider 或运行环境不可用 | 是 |
| `InvalidConfiguration` | provider-specific 配置无效 | 否 |
| `InitializationFailed` | 接受请求后初始化失败 | 否 |

`FallbackPolicy::Never` 在第一个叶失败后停止，`OnAbsence` 只在前两类 absence
失败后继续，`OnAnyError` 在所有叶失败后继续。named selection 只有一个候选，永远
不会 fallback。fallback 终止时，`ProviderCreationError` 按实际调用顺序保留 attempts，
最后一次实际调用是 decisive attempt。

provider 尚未被调用前的 `UnknownProviders`、`NoCandidates` 或 `EmptyRegistry` 属于
selection/resolution error，不属于 provider failure，也不会生成虚假的 provider
attempt。迁移时应根据分类和 selection policy 决定是否重试，不能仅按错误字符串判断。

## 5. canonical URI 的作用域

`FileSystemResolution` 中的 canonical URI 是选中 provider 为本次 resolution 生成的
无凭据安全定位结果。它与作为连接输入的 `ConnectionUri` 具有不同生命周期和用途：

- provider 负责建立 URI 与 decoded `Path` 的 provider-specific 关系；
- canonical URI 不含 password、userinfo、credential-like query 或其他 secret；
- `FileSystemResolution::try_new` 只验证 path 约束、limits，以及 canonical URI scheme
  是否属于返回 filesystem facade 声明的 schemes；
- authority、path 的规范化以及 capability 语义仍由 provider 定义；
- canonical URI 不是所有 provider 通用的 URI 规范化，也不是跨 provider 的全局 identity。

它可以用于当前 resolution 的安全标识和日志关联，但不能反向恢复原始 connection
文本，也不能单独作为带凭据配置或未来 configured-filesystem cache 的充分 key。

## 6. scheme-derived selection 的语法

`resolve_config` 没有显式 selection 时，读取已经解析的 URI scheme，并将它作为
`ProviderSelection::named` 的输入。selector parser 会先去除首尾空白并将 ASCII
字母转为小写，然后要求：

- 非空；
- 只含 ASCII 字符；
- 首尾是 ASCII 字母或数字；
- 中间字符只允许 ASCII 小写字母、数字、`-`、`_`、`.`、`+`。

因此 `/`、`:`、空白、非 ASCII 字符和其他标点不能作为 selector 语法的一部分。只有
scheme component 参与 selection；authority、userinfo、query 和原始 URI 文本不会被
拼接或重新解析为 selector。输入不符合语法时，错误发生在 provider 创建前。

## 7. 迁移检查表

1. 将 embedded credential 与 `CredentialRef` 冲突改为检查
   `CredentialSourceConflict` 和 `reason_code()`。
2. 将日志和用户界面依赖从错误完整文本迁移到 typed error、稳定 reason code 与
   provider failure 分类。
3. 不要假设替换全局 application-default redactor 会改变 registry error 的
   `Display`/`Debug`；在应用自己的输出边界执行额外 redaction。
4. 若修改默认 selection 或 provider 注册，重新取得 resolution 以获得新的原子快照。
5. 将 scheme-derived selection 限制在上述 selector grammar 内，并避免把 authority、
   userinfo 或 query 当成 selection。
6. 只把 credential-free canonical URI 用作本次 resolution 的安全定位结果，并由
   provider 负责其 URI/path 语义。
