# qubit-fs-registry

[![Rust CI](https://github.com/qubit-ltd/rs-fs-registry/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-fs-registry/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-fs-registry/coverage-badge.json)](https://qubit-ltd.github.io/rs-fs-registry/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-fs-registry.svg?color=blue)](https://crates.io/crates/qubit-fs-registry)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

`qubit-fs-registry` is the runtime boundary between a `qubit-fs` application
and provider crates. Register synchronous or asynchronous providers during
application assembly, resolve a complete filesystem configuration, and receive
the filesystem together with its decoded path and canonical URI.

## Installation

```bash
cargo add qubit-fs qubit-fs-registry
```

The registry is synchronous by default. Add the async feature to
qubit-fs-registry when registering asynchronous providers.

A local provider is supplied by its own crate:

```bash
cargo add qubit-fs-local --features registry
```

## Quick Start

An application opening a local report can register its provider once and
resolve a `file:` configuration at the boundary:

```rust
use qubit_fs::error::FsResult;
use qubit_fs::path::ConnectionUri;
use qubit_fs_local::LocalFileSystemProvider;
use qubit_fs_registry::{FileSystemConfig, FileSystemRegistry};

fn open_local_report() -> FsResult<()> {
    let registry = FileSystemRegistry::default();
    registry.register(LocalFileSystemProvider::new())?;

    let config = FileSystemConfig::new(ConnectionUri::parse("file:///tmp/report.csv")?);
    let resolution = registry.resolve_config(&config)?;
    let _metadata = resolution.file_system().stat(resolution.path())?;
    println!("{}", resolution.canonical_uri());
    Ok(())
}
```

## What It Provides

- `FileSystemRegistry` and `AsyncFileSystemRegistry` register providers and
  resolve synchronous or asynchronous configurations.
- `FileSystemConfig` carries a URI, optional selection, non-sensitive options
  and metadata, and an optional `CredentialRef`.
- Each resolution pairs a filesystem with its provider-decoded path and a
  secret-free canonical URI.

Formatted registry errors include safe selector and provider context. These
fields are classified through the process-wide `qubit_redact::RedactionPolicy`;
applications can raise `provider_id` or `selection` to a sensitive level before
formatting diagnostics.

Selection is configuration-first: `resolve_config` uses an explicit selection,
then the URI scheme; it does not fall back to the registry default.
`resolve_selected_config` and `resolve_default_config` reject a conflicting
selection embedded in the configuration.

`CredentialRef` identifies a credential source such as a profile, environment
variable names, or external provider ID; it is not a place to store a token,
password, private key, or other secret. `ProviderSelection`, `ProviderId`, and
`ProviderDescriptor` are owned by `qubit-spi` and are intentionally not
re-exported. Add `qubit-spi` directly when using those types.

## Learn More

- [English user guide](doc/user_guide.md)
- [中文用户手册](doc/user_guide.zh_CN.md)
- [API documentation](https://docs.rs/qubit-fs-registry)
- [中文 README](README.zh_CN.md)

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
