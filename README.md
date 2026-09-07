# qubit-fs-registry

[![Rust CI](https://github.com/qubit-ltd/rs-fs-registry/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-fs-registry/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-fs-registry/coverage-badge.json)](https://qubit-ltd.github.io/rs-fs-registry/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-fs-registry.svg?color=blue)](https://crates.io/crates/qubit-fs-registry)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

`qubit-fs-registry` binds application configuration to runtime-registered filesystem
providers. Register factories during startup, then resolve each connection into
a filesystem facade, its decoded path, and a credential-free canonical URI.
Business code can operate on that result without knowing the provider factory.

## Installation

```bash
cargo add qubit-fs@0.4 qubit-fs-registry@0.3
cargo add qubit-fs-local@0.3 --features registry
```

The default feature set is synchronous. For asynchronous providers, enable
`qubit-fs-registry/async`. SPI selection types require a direct `qubit-spi@0.11`
dependency; this crate does not re-export them.

## Quick Start

Run this complete program in a new, empty working directory. It writes
`report.csv`, registers a provider rooted in that directory, resolves the URI,
and verifies the report's size. The output is `file:///report.csv: 22 bytes`.

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

## What It Provides

- Synchronous and asynchronous registries with shared catalogs and owned resolution snapshots.
- Configuration carrying a connection URI, selection, non-sensitive options/metadata, and credential references.
- Validated filesystem/path/canonical-URI results, plus typed selection and creation errors.

`resolve_config` uses the configured selection or the URI scheme. Only
`resolve_default_config` uses the registry default. Conflicting selections and
embedded-secret/external-reference combinations fail before provider creation.
`CredentialRef` identifies a source; never store secret values in it.

Resolution validates declared properties; it does not prove a file exists.
Providers own URI decoding and storage operations. A canonical URI is scoped to
the returned filesystem and is not a cross-provider identity or a sufficient cache key.

Error `Display`/`Debug` use the immutable `Redactor::standard()` policy and do
not recursively format provider sources. Use `reason_code()` and typed errors
for programmatic handling. Application-default redactor changes do not alter this policy.

## Learn More

- [English user guide](doc/user_guide.md) · [中文用户手册](doc/user_guide.zh_CN.md)
- [Design](doc/file_system_registry_design.md) · [中文设计](doc/file_system_registry_design.zh_CN.md)
- [Migration](doc/registry_contract_migration.md) · [中文迁移说明](doc/registry_contract_migration.zh_CN.md)
- [API documentation](https://docs.rs/qubit-fs-registry) · [中文 README](README.zh_CN.md)

## Testing

```bash
# Run tests with the default feature set
cargo test

# Run tests with all declared features
cargo test --all-features

# Project CI checks
./ci-check.sh

# Check code coverage
./coverage.sh
```

## License

Copyright (c) 2025 - 2026. Haixing Hu. All rights reserved.

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for the
full license text.

## Contributing

Contributions are welcome. Please follow the Rust API guidelines, keep public
API documentation and tests current, and run `./align-ci.sh` to format code and
`./ci-check.sh` to satisfy CI requirements before submitting a pull request.

## Author

**Haixing Hu** - *Qubit Co. Ltd.*

Repository: [https://github.com/qubit-ltd/rs-fs-registry](https://github.com/qubit-ltd/rs-fs-registry)
