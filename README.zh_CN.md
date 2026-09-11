# qubit-fs-registry

[![Rust CI](https://github.com/qubit-ltd/rs-fs-registry/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-fs-registry/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-fs-registry/coverage-badge.json)](https://qubit-ltd.github.io/rs-fs-registry/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-fs-registry.svg?color=blue)](https://crates.io/crates/qubit-fs-registry)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)

`qubit-fs-registry` 将应用配置交给运行时注册的文件系统提供者。在启动阶段注册工厂后，
应用即可将连接配置解析为文件系统门面、已解码路径和不含凭据的规范 URI，
业务代码直接使用解析结果，无需了解提供者的创建过程。

## 安装

```bash
cargo add qubit-fs@0.7 qubit-fs-registry@0.6
cargo add qubit-fs-local@0.8 --features registry
```

默认仅启用同步接口。接入异步提供者时，需要开启 `qubit-fs-registry/async`。
使用 SPI 选择类型时，直接添加 `qubit-spi@0.12`；本 crate 不重新导出这些类型。

## 快速开始

在一个新建的空工作目录中运行下面的完整程序。程序写入 `report.csv`，注册以该目录为根的
提供者，解析文件 URI，再查询并核对报表大小。预期输出为 `file:///report.csv: 22 bytes`。

```rust
use std::time::Duration;

use qubit_fs::metadata::FileSystemId;
use qubit_fs::path::ConnectionUri;
use qubit_fs_local::LocalCopyResourceLimits;
use qubit_fs_local::LocalDeleteResourceLimits;
use qubit_fs_local::LocalFileSystemProvider;
use qubit_fs_local::LocalListResourceLimits;
use qubit_fs_local::LocalResourcePolicy;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemRegistry;

fn bounded_policy() -> Result<LocalResourcePolicy, Box<dyn std::error::Error>> {
    Ok(LocalResourcePolicy::bounded(
        LocalListResourceLimits::new(16, 10_000, 8_388_608, 32, Duration::from_secs(30))?,
        LocalCopyResourceLimits::new(
            16,
            10_000,
            1_073_741_824,
            32,
            Duration::from_secs(30),
        )?,
        LocalDeleteResourceLimits::new(16, 10_000, 8_388_608, Duration::from_secs(30)),
    ))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::current_dir()?;
    std::fs::write(root.join("report.csv"), b"name,total\nexample,42\n")?;
    let registry = FileSystemRegistry::default();
    registry.register(LocalFileSystemProvider::rooted(
        FileSystemId::new("reports")?, &root, bounded_policy()?,
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

## 为什么需要这个项目

应用通常用连接 URI 配置文件访问，并在不同部署环境中选用不同后端。若业务代码直接调用
某个具体 provider 的工厂，就会与后端的注册方式和 URI 规则紧耦合。`qubit-fs-registry`
承担装配边界：启动阶段注册 provider，之后传入 `FileSystemConfig` 或 `ConnectionUri`，
即可得到经过校验的文件系统、已解码路径和不含凭据的规范 URI。选择与回退复用
`qubit-spi`，不必重复实现服务目录。

本 crate 不实现存储操作、不解析凭据秘密，也不自动发现 provider。

## 提供的能力

- 同步与异步注册表；克隆共享目录，每次解析持有自己的候选快照。
- 完整配置：连接 URI、选择规则、非敏感选项与元数据，以及外部凭据引用。
- 经过校验的文件系统、路径和规范 URI，以及可按类型处理的选择与创建错误。

`resolve_config` 使用配置中的选择规则，缺省时使用 URI 协议；只有 `resolve_default_config`
使用注册表默认值。选择规则冲突，或内嵌秘密与外部凭据引用并存时，会在创建提供者之前报错。
`CredentialRef` 仅标识凭据来源，不能存放秘密值。

解析只校验声明的属性，不证明文件已存在。URI 解码和存储操作由提供者负责。
规范 URI 的含义取决于返回的文件系统，不能单独作为跨提供者身份或缓存键。

错误的 `Display`/`Debug` 使用不可变的 `Redactor::standard()` 策略，不递归格式化提供者错误源。
程序应使用 `reason_code()` 和类型化错误进行判断；替换应用默认脱敏器不会改变这里的策略。

## 延伸阅读

- [中文用户手册](doc/user_guide.zh_CN.md) · [English user guide](doc/user_guide.md)
- [中文设计](doc/file_system_registry_design.zh_CN.md) · [Design](doc/file_system_registry_design.md)
- [中文迁移说明](doc/registry_contract_migration.zh_CN.md) · [Migration](doc/registry_contract_migration.md)
- [API 文档](https://docs.rs/qubit-fs-registry) · [English README](README.md)

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
