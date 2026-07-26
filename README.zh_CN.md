# Qubit FS Registry

[![Rust CI](https://github.com/qubit-ltd/rs-fs-registry/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-fs-registry/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/qubit-fs-registry.svg?color=blue)](https://crates.io/crates/qubit-fs-registry)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Docs.rs](https://docs.rs/qubit-fs-registry/badge.svg)](https://docs.rs/qubit-fs-registry)
[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)

`qubit-fs-registry` 为 [`qubit-fs`](https://crates.io/crates/qubit-fs) 提供运行时
provider 发现、完整文件系统配置与 SPI 集成。只使用文件系统 trait 和值类型的程序
应仅依赖 `qubit-fs`。

## 安装

```bash
cargo add qubit-fs-registry
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
credential-like option key。所有 secret 只能通过 `CredentialRef` 保存。

同步和异步 registry 都公开 provider descriptor、catalog 大小、底层 selection 解析，
以及 URI/config 便捷方法。`resolve_selected` 与 `resolve` 返回某一时刻的 provider
snapshot；`resolve_config_async` 根据配置创建异步文件系统。

## 测试

```bash
cargo test --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
```

## 许可证

Copyright (c) 2026 Haixing Hu。本项目基于 Apache License 2.0 授权，完整文本见
[LICENSE](LICENSE)。

## 贡献

请保持公共 API 文档和外部测试同步，并在发起 Pull Request 前运行上述测试命令。

## 作者

Haixing Hu — Qubit Co. Ltd.
