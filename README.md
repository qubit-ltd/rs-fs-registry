# Qubit FS Registry

[![Rust CI](https://github.com/qubit-ltd/rs-fs-registry/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-fs-registry/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-fs-registry/coverage-badge.json)](https://qubit-ltd.github.io/rs-fs-registry/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-fs-registry.svg?color=blue)](https://crates.io/crates/qubit-fs-registry)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

`qubit-fs-registry` provides runtime provider discovery, complete filesystem
configuration, and SPI integration for [`qubit-fs`](https://crates.io/crates/qubit-fs).
Applications that only need filesystem traits and value types should depend on
`qubit-fs` alone.

## Installation

```bash
cargo add qubit-fs qubit-fs-registry
cargo add qubit-fs-local --features registry
```

## Usage

Register backend crates during application assembly. A provider is selected by
the URI scheme unless the configuration supplies an explicit selection.

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

`FileSystemConfig` contains a URI, optional `ProviderSelection`, validated
`UserMetadata`, and an optional `CredentialRef`. Build `UserMetadata` before
passing it to `with_options`; construction rejects credential-like option keys.
Store secrets only through `CredentialRef`.

Both synchronous and asynchronous registries expose provider descriptors,
catalog size, low-level selection resolution, and URI/config convenience
methods. `resolve_selected` and `resolve` return a point-in-time provider
snapshot. `resolve_selected_config` and `resolve_default_config` create a
filesystem through explicit or default selection; asynchronous counterparts
use the `_async` suffix. Provider catalog IDs retain the `ProviderId` type.

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
