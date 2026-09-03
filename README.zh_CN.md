# qubit-fs-registry

[![Rust CI](https://github.com/qubit-ltd/rs-fs-registry/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-fs-registry/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-fs-registry/coverage-badge.json)](https://qubit-ltd.github.io/rs-fs-registry/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-fs-registry.svg?color=blue)](https://crates.io/crates/qubit-fs-registry)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)

`qubit-fs-registry` 是 `qubit-fs` 应用与 provider crate 之间的运行时边界。在应用组装阶段
注册同步或异步 provider，解析完整的文件系统配置，并取得文件系统、已解码路径和 canonical URI。

## 安装

```bash
cargo add qubit-fs qubit-fs-registry
```

本地 provider 由独立 crate 提供：

```bash
cargo add qubit-fs-local --features registry
```

## 快速开始

打开本地报表的应用可注册一次 provider，并在边界处解析 `file:` 配置：

```rust
use qubit_fs::error::FsResult;
use qubit_fs::path::ConnectionUri;
use qubit_fs_local::{LocalFileSystemProvider, LocalResourcePolicy};
use qubit_fs_registry::{FileSystemConfig, FileSystemRegistry};

fn open_local_report() -> FsResult<()> {
    let registry = FileSystemRegistry::default();
    registry.register(LocalFileSystemProvider::host(LocalResourcePolicy::unbounded()))?;

    let config = FileSystemConfig::new(ConnectionUri::parse("file:///tmp/report.csv")?);
    let resolution = registry.resolve_config(&config)?;
    let _metadata = resolution.file_system().stat(resolution.path())?;
    println!("{}", resolution.canonical_uri());
    Ok(())
}
```

## 提供的能力

- `FileSystemRegistry` 与 `AsyncFileSystemRegistry` 注册 provider，并解析同步或异步配置。
- `FileSystemConfig` 包含 URI、可选 selection、非敏感 options 与 metadata，以及可选的
  `CredentialRef`。
- 每个 resolution 将文件系统与 provider 解码路径、无 secret 的 canonical URI 配对。

格式化 registry error 会包含安全的 selector 和 provider 上下文；这些字段会经过进程级
`qubit_redact::RedactionPolicy`。如果 provider identity 或 selection 也应视为敏感字段，可在格式化前
将 `provider_id` 或 `selection` 提升为敏感级别。

selection 以配置为先：`resolve_config` 先使用显式 selection，再使用 URI scheme；它不会回退到
registry 默认 selection。`resolve_selected_config` 和 `resolve_default_config` 会拒绝配置中与其
冲突的内嵌 selection。

`CredentialRef` 标识凭据来源，例如 profile、环境变量名称或外部 provider ID；它不用于存储
token、password、private key 或其他 secret。`ProviderSelection`、`ProviderId` 与
`ProviderDescriptor` 由 `qubit-spi` 所有，本 crate 有意不重新导出它们。使用这些类型时需直接
添加 `qubit-spi` 依赖。

## 延伸阅读

- [English user guide](doc/user_guide.md)
- [中文用户手册](doc/user_guide.zh_CN.md)
- [API 文档](https://docs.rs/qubit-fs-registry)
- [English README](README.md)

## 测试

```bash
# 使用默认 feature 集运行测试
cargo test

# 使用项目声明的全部 feature 运行测试
cargo test --all-features

# 运行项目 CI 检查
./ci-check.sh

# 检查代码覆盖率
./coverage.sh
```

## 许可证

Copyright (c) 2025 - 2026. Haixing Hu. All rights reserved.

本项目基于 Apache License 2.0 授权。完整许可证文本请参阅
[LICENSE](LICENSE)。

## 贡献

欢迎贡献。请遵循 Rust API 指南，及时更新公共 API 文档与测试，并在提交
Pull Request 前运行 `./align-ci.sh`格式化代码，运行`./ci-check.sh`对齐CI要求。

## 作者

**Haixing Hu** - *Qubit Co. Ltd.*

仓库地址：[https://github.com/qubit-ltd/rs-fs-registry](https://github.com/qubit-ltd/rs-fs-registry)
