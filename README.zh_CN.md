# Qubit FS Registry

[![Rust CI](https://github.com/qubit-ltd/rs-fs-registry/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-fs-registry/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-fs-registry/coverage-badge.json)](https://qubit-ltd.github.io/rs-fs-registry/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-fs-registry.svg?color=blue)](https://crates.io/crates/qubit-fs-registry)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)

`qubit-fs-registry` 为 [`qubit-fs`](https://crates.io/crates/qubit-fs) 提供运行时
provider 发现、完整文件系统配置与 SPI 集成。只使用文件系统 trait 和值类型的程序
应仅依赖 `qubit-fs`。

## 安装

```bash
cargo add qubit-fs qubit-fs-registry
cargo add qubit-fs-local --features registry
```

## 使用方法

在应用组装阶段注册后端 crate。没有显式 selection 时，provider 按 URI scheme 选择。

```rust
use qubit_fs::{FsResult, FsUri};
use qubit_fs_local::LocalFileSystemProvider;
use qubit_fs_registry::{FileSystemConfig, FileSystemRegistry};

fn open_local_file() -> FsResult<()> {
    let registry = FileSystemRegistry::default();
    registry.register(LocalFileSystemProvider)?;

    let config = FileSystemConfig::new(FsUri::parse("file:///tmp/example.txt")?);
    let resource = registry.resource(&config)?;
    println!("{}", resource.path());
    Ok(())
}
```

`FileSystemConfig` 包含 URI、可选 `ProviderSelection`、已校验的 `UserMetadata` 和可选
`CredentialRef`。先构造 `UserMetadata`，再传给 `with_options`；构造时会拒绝
credential-like option key。`CredentialRef` 的值必须只包含 provider 能识别的引用，例如
profile 名称、环境变量名称或外部凭据 provider ID；不得包含 credential、token、password、
private key 或其他 secret 材料。

自动选择会从 URI scheme 派生 provider selector。请使用 selector 兼容的 scheme（例如
`file` 或 `s3`）；无法派生时，请提供显式 `ProviderSelection`。

同步和异步 registry 都公开 provider descriptor、catalog 大小、底层 selection 解析，
以及 URI/config 便捷方法。`resolve_selected` 与 `resolve` 返回某一时刻的 provider
snapshot。`resolve_selected_config` 与 `resolve_default_config` 分别通过显式或默认
selection 创建文件系统；异步版本使用 `_async` 后缀。Catalog ID 保留
`ProviderId` 强类型。

### 异步使用

在应用程序组装阶段注册异步 provider 后，可以等待同样的 URI 便捷流程。URI 便捷方法返回的
future 自行持有 URI 配置和 provider snapshot，因此可以在传入的 registry handle 与 URI 离开
作用域后继续使用。

```rust,no_run
use qubit_fs::{AsyncFileResource, FsResult, FsUri};
use qubit_fs_registry::AsyncFileSystemRegistry;

async fn open_async(
    registry: &AsyncFileSystemRegistry,
) -> FsResult<AsyncFileResource> {
    let uri = FsUri::parse("memory:///example.txt")?;
    registry.resource_uri_async(&uri).await
}
```

## 编写 provider

Provider crate 需要直接依赖 SPI：

```bash
cargo add qubit-spi
```

实现 `ProviderMetadata` 和 `ServiceProvider<FileSystemSpec>`，并从 provider 的配置化
创建路径返回 `FileSystemResolution`。应用程序通过 `qubit-fs-registry` 使用 provider；
provider 则使用 SPI 契约公开 metadata、selection identity 与 resolution。
异步 provider 需要实现 `ProviderMetadata` 和 `AsyncServiceProvider<FileSystemSpec>`；registry
通过 `AsyncFileSystemProvider` 约束接收它们。

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
