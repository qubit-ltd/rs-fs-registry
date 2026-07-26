# Qubit FS Registry

[![Rust CI](https://github.com/qubit-ltd/rs-fs-registry/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-fs-registry/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/qubit-fs-registry.svg?color=blue)](https://crates.io/crates/qubit-fs-registry)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Docs.rs](https://docs.rs/qubit-fs-registry/badge.svg)](https://docs.rs/qubit-fs-registry)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

`qubit-fs-registry` provides runtime provider discovery, complete filesystem
configuration, and SPI integration for [`qubit-fs`](https://crates.io/crates/qubit-fs).
Applications that only need filesystem traits and value types should depend on
`qubit-fs` alone.

## Installation

```bash
cargo add qubit-fs-registry
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
passing it to `with_options`; it rejects credential-like option keys. Store
secrets only through `CredentialRef`.

Both synchronous and asynchronous registries expose provider descriptors,
catalog size, low-level selection resolution, and URI/config convenience
methods. `resolve_selected` and `resolve` return a point-in-time provider
snapshot; `resolve_config_async` creates an asynchronous filesystem from its
configuration.

## Testing

```bash
cargo test --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
```

## License

Copyright (c) 2026 Haixing Hu. Licensed under Apache License 2.0; see
[LICENSE](LICENSE).

## Contributing

Keep public API documentation and external tests current, and run the testing
commands above before opening a pull request.

## Author

Haixing Hu — Qubit Co. Ltd.
