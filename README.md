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
cargo add qubit-fs@0.3 qubit-fs-registry@0.8
cargo add qubit-fs-local@0.11 --features registry
```

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
python3 scripts/check_published_docs.py .
```

The published dependency check requires Python 3.12+ and the Rust toolchain
channel configured in `rust-toolchain.toml`.
It runs default and all-feature Rustdoc tests on the extracted package archive,
then checks the Markdown examples, including both complete async-resolution
appendices. It must succeed for the same source state you intend to publish.
The check snapshots the registry and its three sibling inputs (`qubit-fs`,
`qubit-fs-local`, and `qubit-local-files`) before packaging or compiling examples.
Each snapshot includes Git-tracked and non-ignored untracked regular files,
including `Cargo.lock`, while excluding `.git`, `target`, and `.cargo`. The
`input-manifest.json` records each package version, source and snapshot paths,
Git HEAD, dirty state, and a SHA-256 digest over sorted relative paths, contents,
lengths, and executable bits. The checker compares source inputs before and after
copying and verifies the snapshot digest; a copy-time change fails and retains
diagnostics. Changes after copying do not change the inputs used by that run.
The exact sibling versions still come from
`[package.metadata.documentation.published-check.sibling-versions]` in
`Cargo.toml`: `qubit-fs` 0.3.0, `qubit-fs-local` 0.11.0, and
`qubit-local-files` 0.5.1. The provider shim references the tested archive. This
checks the archive and examples against fixed sibling source snapshots; it does
not verify the contents of corresponding crates.io releases. The graph check
allows only these named sibling paths, the archive, and the generated
Markdown-example crate root; every other dependency must come from a registry.
Rerun it after changing source files, the manifest, or examples. CI runs it for
pull requests, pushes to `main`, and manual workflow runs; failures must not fall
back to other local overrides.

Before publishing, verify that the required registry versions are already
published and satisfy the normal dependency requirements: `qubit-fs` 0.3.0,
`qubit-fs-local` 0.11, and `qubit-local-files` 0.5.1. Optional dependencies
remain in published crate metadata even when their features are disabled. In
particular, the first release of registry 0.8 has a dependency cycle:
`qubit-fs-local` 0.11 requires registry 0.8, while the strict registry 0.8
check requires `qubit-fs-local` 0.11. If these prerequisites are not already
published, stop and record the missing versions and dependency chain; resolve
the release policy or dependency design through separate review. Snapshot-check
success does not permit bypassing this gate.
See the [Published Documentation Check](doc/user_guide.md#published-documentation-check)
section of the English user guide for the full release prerequisites.

Once a compatible registry prerequisite is published, publish any missing
`qubit-fs`, `qubit-fs-local`, and `qubit-local-files` releases, then run the
strict registry consumer check:

```bash
python3 scripts/check_published_docs.py . --dependency-source registry
```

This mode resolves the declared dependencies from crates.io and checks the
packaged archive, default and all-feature doctests, and all shipped Markdown
examples. It uses no sibling overrides or provider shim, and fails without
falling back when a required release is unavailable or a dependency resolves
outside crates.io. Run the manual **Published Registry Consumption** workflow
on the exact release ref; publish only after it succeeds. The result records
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
