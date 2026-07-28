# Qubit FS Registry

[![Rust CI](https://github.com/qubit-ltd/rs-fs-registry/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-fs-registry/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-fs-registry/coverage-badge.json)](https://qubit-ltd.github.io/rs-fs-registry/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-fs-registry.svg?color=blue)](https://crates.io/crates/qubit-fs-registry)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

`qubit-fs-registry` provides runtime provider registration and resolution,
complete filesystem configuration, and SPI integration for [`qubit-fs`](https://crates.io/crates/qubit-fs).
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
use qubit_fs::{ConnectionUri, FsResult};
use qubit_fs_local::LocalFileSystemProvider;
use qubit_fs_registry::{FileSystemConfig, FileSystemRegistry};

fn open_local_file() -> FsResult<()> {
    let registry = FileSystemRegistry::default();
    registry.register(LocalFileSystemProvider)?;

    let config = FileSystemConfig::new(ConnectionUri::parse("file:///tmp/example.txt")?);
    let resolution = registry.resolve_config(&config)?;
    println!("{}", resolution.path());
    Ok(())
}
```

`FileSystemConfig` contains a URI, optional `ProviderSelection`, validated
`NonSensitiveMetadata`, and an optional `CredentialRef`. Build `UserMetadata` before
passing it to `with_options`; construction rejects credential-like option keys.
`CredentialRef` values must contain only provider-recognized references, such
as profile names, environment-variable names, or external credential-provider
IDs. They must not contain credentials, tokens, passwords, private keys, or
other secret material.

### Selection precedence

The following rules apply equally to synchronous and asynchronous registries;
asynchronous config methods consume their configuration.

| Method family | Selection used, in precedence order |
| --- | --- |
| `resolve_config` | The configuration's explicit `ProviderSelection`; otherwise `ProviderSelection::named` from its URI scheme. |
| `resolve_uri` | `ProviderSelection::named` from the URI scheme. |
| `resolve_selected_config` | The supplied selection; an embedded, different configuration selection is rejected. |
| `resolve_default_config` | The registry's current default selection; an embedded, different configuration selection is rejected. |

Use a selector-compatible URI scheme (for example, `file` or `s3`), or supply
an explicit `ProviderSelection` when the scheme cannot form one. In
particular, `resolve_config` does not fall back to the registry default.

`ProviderSelection`, `ProviderId`, and `ProviderDescriptor` are SPI-owned types
and are intentionally not re-exported by this crate. Applications that
construct explicit selections or use the low-level provider catalog APIs must
also add a direct `qubit-spi` dependency.

Both synchronous and asynchronous registries expose provider descriptors,
catalog size, and URI/config convenience methods. `resolve_selected_config`
and `resolve_default_config` create a
concrete resolution through explicit or default selection. They reject a configuration whose embedded selection
conflicts with the explicit or current default selection; use `resolve_config`
when the configuration itself owns selection. Provider catalog IDs retain the
`ProviderId` type.

### Asynchronous usage

Register asynchronous providers during application assembly, then await the
same URI convenience flow. URI convenience futures own their URI configuration
and provider snapshot, so they can outlive the registry handle and URI passed
to the method.

```rust,no_run
use qubit_fs::{ConnectionUri, FsResult};
use qubit_fs_registry::{AsyncFileSystemRegistry, FileSystemConfig};

async fn open_async(
    registry: &AsyncFileSystemRegistry,
) -> FsResult<()> {
    let config = FileSystemConfig::new(ConnectionUri::parse("memory:///example.txt")?);
    let resolution = registry.resolve_config(config).await.map_err(Into::into)?;
    let _file_system = resolution.file_system().clone();
    Ok(())
}
```

Registry methods return `FileSystemRegistryResult`, whose
`FileSystemRegistryError` preserves registration, selection, resolution, and
provider-creation diagnostics. `FileSystemRegistryError` converts into
`FsError` for applications that use `FsResult`; the conversion retains the
typed registry error as its source.

## Writing a provider

Provider crates require the SPI directly:

```bash
cargo add qubit-spi
```

Implement `ProviderMetadata` and `ServiceProvider<FileSystemSpec>`, then
return `FileSystemResolution` from the provider's configured creation path.
Applications consume the provider through `qubit-fs-registry`; providers use
the SPI contract to expose their metadata, selection identity, and resolution.
Asynchronous providers implement `ProviderMetadata` and
`AsyncServiceProvider<FileSystemSpec>`. `AsyncFileSystemProvider` is the
corresponding trait object alias for shared registrations, such as
`Arc<AsyncFileSystemProvider>`.

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
