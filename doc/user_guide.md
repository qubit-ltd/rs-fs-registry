# qubit-fs-registry User Guide

[中文](user_guide.zh_CN.md) · [README](../README.md) · [API documentation](https://docs.rs/qubit-fs-registry)

## Purpose and Audience

This guide is for application and provider authors who need to bind
`qubit-fs` to runtime-registered filesystem providers. It covers the current
`qubit-fs-registry` 0.1 API, including synchronous and asynchronous resolution.

## Conceptual Model

```text
FileSystemConfig
  ├─ ConnectionUri
  ├─ optional ProviderSelection
  ├─ non-sensitive options and metadata
  └─ optional CredentialRef
          │
          ▼
registered provider
          │
          ▼
resolution = filesystem + decoded path + canonical URI
```

`FileSystemRegistry` creates synchronous resolutions;
`AsyncFileSystemRegistry` creates asynchronous resolutions. Both expose
registration, descriptors, catalog size, URI convenience methods, and the same
selection rules. The asynchronous configuration methods take ownership of their
configuration and return futures for resolution.

## Scenario

An application selects a local filesystem provider at startup, then opens a
report URI without coupling its report-handling code to a provider factory.
Success means the boundary yields a filesystem and logical path that can be
used for `stat`, while the canonical URI is available for safe identification.

## Installation and Minimal Configuration

```bash
cargo add qubit-fs qubit-fs-registry
cargo add qubit-fs-local --features registry
```

Provider crates that create explicit SPI selections or use low-level provider
catalog types must depend directly on `qubit-spi`; those SPI-owned types are
not re-exported by this crate.

## Core Workflow

```rust
use qubit_fs::error::FsResult;
use qubit_fs::path::ConnectionUri;
use qubit_fs_local::{LocalFileSystemProvider, LocalResourcePolicy};
use qubit_fs_registry::{FileSystemConfig, FileSystemRegistry};

fn inspect_report() -> FsResult<()> {
    let registry = FileSystemRegistry::default();
    registry.register(LocalFileSystemProvider::host(LocalResourcePolicy::unbounded()))?;

    let config = FileSystemConfig::new(ConnectionUri::parse("file:///tmp/report.csv")?);
    let resolution = registry.resolve_config(&config)?;
    let metadata = resolution.file_system().stat(resolution.path())?;
    println!("{metadata:?} at {}", resolution.canonical_uri());
    Ok(())
}
```

Keep the URI and configuration at the resolution boundary. Downstream code
uses `resolution.file_system()` and `resolution.path()` rather than decoding
the URI again.

## Advanced Usage

### Selection precedence

| Entry point | Selection rule |
| --- | --- |
| `resolve_config` | Configuration selection, otherwise a named selection from the URI scheme. |
| `resolve_uri` | A named selection from the URI scheme. |
| `resolve_selected_config` | The supplied selection; a different embedded selection is an error. |
| `resolve_default_config` | The current registry default; a different embedded selection is an error. |

Therefore `resolve_config` does not fall back to the registry default. Use the
explicit/default entry points only when the caller, rather than the URI
configuration, owns selection.

### Credentials and async resolution

Use `CredentialRef` only to reference a provider-recognized source:
`DefaultChain`, a profile, environment variable names, or an external provider
ID. Do not place secret material in it. The registry also rejects configuration
credential conflicts before provider creation.

For async providers, register with `AsyncFileSystemRegistry` and await its
owned-config `resolve_config`, `resolve_uri`, `resolve_selected_config`, or
`resolve_default_config` future. The resulting `AsyncFileSystemResolution`
has the same filesystem/path/canonical-URI shape.

## Errors and Diagnostics

Registry operations return `FileSystemRegistryResult` and preserve structured
registration, selection, resolution, and provider-creation diagnostics in
`FileSystemRegistryError`. Provider creation may fail after a provider has
been selected; inspect the typed error rather than replacing it with a generic
message. A registry error can convert to `FsError` while retaining the typed
registry error as its source.
Formatted registry errors include safe selector and provider context. Their
fields are passed through the process-wide `qubit_redact::RedactionPolicy`, so
applications can raise `provider_id` or `selection` to a sensitive level before
formatting diagnostics.

## Troubleshooting

| Symptom | Check |
| --- | --- |
| No provider resolves a URI | Register the provider and use a URI scheme compatible with its selection. |
| `resolve_config` ignores the default | This is expected; provide a configuration selection or use `resolve_default_config`. |
| Selection conflict | Remove the different embedded selection or call the configuration-owned `resolve_config` path. |
| Credential configuration is rejected | Use a `CredentialRef` reference only; remove embedded/query credential material and secret-like options. |
| Cannot name a selection type | Add a direct `qubit-spi` dependency. |

## Limitations and Best Practices

- The registry does not implement a storage backend; a registered provider
  creates the filesystem facade.
- Provider-specific URI decoding, path rules, capabilities, and secret source
  interpretation remain provider responsibilities.
- Keep configuration non-sensitive. `CredentialRef` is a reference boundary,
  not secret storage.

## Further Reading

- [README](../README.md)
- [中文用户手册](user_guide.zh_CN.md)
- [API documentation](https://docs.rs/qubit-fs-registry)
