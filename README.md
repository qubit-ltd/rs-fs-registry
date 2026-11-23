# qubit-fs-registry

[![Rust CI](https://github.com/qubit-ltd/rs-fs-registry/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-fs-registry/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-fs-registry/coverage-badge.json)](https://qubit-ltd.github.io/rs-fs-registry/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-fs-registry.svg?color=blue)](https://crates.io/crates/qubit-fs-registry)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

`qubit-fs-registry` binds application configuration to runtime-registered filesystem
providers. Register providers during startup, then resolve each connection into
a filesystem facade, its decoded path, and a credential-free canonical URI.
Business code can operate on that result without knowing the provider factory.

## Installation

```bash
cargo add qubit-fs@0.4 qubit-fs-registry@0.8
cargo add qubit-fs-local@0.12 --features registry
```

These commands describe the current 0.8.3 source candidate. Since `qubit-fs ^0.4`
is not yet published, they cannot currently resolve from crates.io; the local
provider examples in this checkout require the sibling source workspace.
The currently published compatible line is registry 0.8.2 with `qubit-fs` 0.3.3
and `qubit-fs-local` 0.11.2. Install that older line with:

```bash
cargo add qubit-fs@0.3 qubit-fs-registry@0.8
cargo add qubit-fs-local@0.11 --features registry
```

The candidate source uses sibling versions `qubit-fs` 0.4.0,
`qubit-fs-local` 0.11.2, `qubit-local-files` 0.6.0, and `qubit-operation`
0.3.1. Verify local-provider compatibility against that graph when the missing
`qubit-fs ^0.4` release becomes available; the older published combination does
not establish candidate compatibility.

The default feature set is synchronous. For asynchronous providers, enable
`qubit-fs-registry/async`. SPI selection types require a direct `qubit-spi@0.14`
dependency; this crate does not re-export them.

## Quick Start

Run this complete program in a new, empty working directory. It writes
`report.csv`, registers a provider rooted in that directory, resolves the URI,
and verifies the report's size. The output is `file:///report.csv: 22 bytes`.

<!-- registry-example: local -->
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

## Why This Project Exists

Applications configure filesystem access through connection URIs and often need
different backends per deployment. Calling a concrete provider factory directly
couples business code to that backend's registration and URI rules.
`qubit-fs-registry` is the assembly boundary: register providers during startup,
then pass `FileSystemConfig` or a `ConnectionUri` and receive a validated
filesystem, decoded path, and credential-free canonical URI. It composes with
`qubit-spi` for selection and fallback instead of reimplementing a service
catalog.

The crate does not implement storage operations or resolve credential secrets.
Optional `inventory` discovers linked provider factories at startup; the default
feature set continues to use explicit registration.

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

CI checks four feature combinations: the default set, `async` only,
`inventory` only, and `async` plus `inventory`.

```bash
# Run tests with the default feature set
cargo test

# Run tests with all declared features
cargo test --all-features

# Project CI checks
./.infra/bin/ci-check.sh

# Check code coverage
./.infra/bin/coverage.sh

# Verify documentation against published dependencies before publishing
python3 -m pip install -r requirements.txt
python3 scripts/check_published_docs.py .

# Check the latest published registry archive and its shipped Markdown instead
python3 scripts/check_published_docs.py . --mode published
```

The checker requires Python 3.12+, the packages in `requirements.txt`, and the
Rust toolchain channel configured in `rust-toolchain.toml`. The default
`candidate` mode packages this checkout, then checks its default and all-feature
Rustdoc tests and shipped Markdown examples, including both async-resolution
appendices. Run it on the exact source state intended for release. The
`published` mode downloads the latest registry archive from crates.io and checks
that archive's own Rustdoc and Markdown examples; it does not validate this
checkout's candidate Markdown. Both modes resolve dependencies from crates.io
and fail without local-source fallback.

The published `0.8.2` line has a working documented combination with
`qubit-fs` 0.3.3 and `qubit-fs-local` 0.11.2. The current 0.8.3 source candidate
instead requires `qubit-fs ^0.4`, for which no matching crates.io release is
available, so candidate mode currently fails during dependency resolution.
`qubit-fs-local` 0.11.2 also needs to be checked against the candidate dependency
graph when compatible `qubit-fs` 0.4 and registry candidate releases are
available. Do not infer that compatibility from the older published
combination. Keep the candidate gate in place and resolve the missing release
prerequisite before publishing.
See the [Published Documentation Check](doc/user_guide.md#published-documentation-check)
section of the English user guide for the full release prerequisites.

After the missing compatible `qubit-fs ^0.4` release and candidate provider
compatibility are verified, run candidate mode on the release source. To inspect
the latest published registry consumer, use:

```bash
python3 scripts/check_published_docs.py . --mode published
```

That mode resolves the declared dependencies from crates.io and checks the
published archive, default and all-feature doctests, and that archive's shipped
Markdown examples. It uses no sibling overrides and fails without fallback when
a required release is unavailable or a dependency resolves outside crates.io.
Run candidate mode on the exact release ref before publishing. The result records
the source digest and resolved package versions. Failed runs preserve separate
Cargo logs for diagnosis.

## License

Copyright (c) 2025 - 2026. Haixing Hu. All rights reserved.

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for the
full license text.

## Contributing

Contributions are welcome. Please follow the Rust API guidelines, keep public
API documentation and tests current, and run `./.infra/bin/align-ci.sh` to format code and
`./.infra/bin/ci-check.sh` to satisfy CI requirements before submitting a pull request.

## Author

**Haixing Hu** - *Qubit Co. Ltd.*

Repository: [https://github.com/qubit-ltd/rs-fs-registry](https://github.com/qubit-ltd/rs-fs-registry)
