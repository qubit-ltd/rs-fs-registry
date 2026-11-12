# qubit-fs-registry User Guide

[中文](user_guide.zh_CN.md) · [README](../README.md) · [API documentation](https://docs.rs/qubit-fs-registry)

## Purpose and Audience

This guide is for application and provider authors who need to bind
`qubit-fs` to runtime-registered filesystem providers. It covers the current
`qubit-fs-registry` 0.8 API, including synchronous and asynchronous resolution.

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
Success means the boundary yields a filesystem and logical path that have passed
the facade's `PathSemantics`, path-form constraints, and limits checks, while
the canonical URI is available for safe identification. The path and scheme
checks performed by `FileSystemResolution::try_new` and
`AsyncFileSystemResolution::try_new` are local and property-based: these checks
do not call `stat`, access the backend, or prove that the target exists. Provider
factories invoked by registry resolution may perform IO to create their
configured filesystem facades. Call `stat` or another operation when the
application needs resource state.

## Installation and Minimal Configuration

```bash
cargo add qubit-fs@0.3 qubit-fs-registry@0.8
cargo add qubit-fs-local@0.11 --features registry
```

A synchronous example without a local provider or linked discovery uses only the default feature set:

```rust
use qubit_fs::path::ConnectionUri;
use qubit_fs_registry::FileSystemRegistry;
use qubit_fs_registry::FileSystemRegistryError;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = FileSystemRegistry::default();
    let error = registry.resolve_uri(&ConnectionUri::parse("missing:///report.csv")?)
        .expect_err("empty registry cannot resolve a provider");
    assert!(matches!(error, FileSystemRegistryError::Resolution(_)));
    Ok(())
}
```
Provider crates that create explicit SPI selections or use low-level provider
catalog types must add `qubit-spi` directly with `cargo add qubit-spi@0.14`;
those SPI-owned types are not re-exported by this crate.

### Optional linked provider discovery

Enable `inventory` to make `FileSystemRegistry::from_inventory()` available.
Provider crates submit factories to `sync_file_system_providers::Entry` with
`FileSystemSpec` through `qubit_spi::submit_sync_provider!`. Async discovery
requires both `async` and `inventory`, and uses
`async_file_system_providers::Entry` and
`AsyncFileSystemRegistry::from_inventory()`. The application must still link the
submitting provider crate. Discovered providers receive the same identity
validation as explicitly registered ones. Duplicate selectors return a typed
inventory build error with the submission source.

With the local provider used in this guide, the `registry` feature on
`qubit-fs-local` enables inventory support. The linked provider crate submits a
factory for a host-wide provider using `LocalResourcePolicy::standard()`; it
does not retain the rooted directory or custom policy from the explicit
registration example. A `file:` URI therefore has a different authority
context. Keep explicit registration on a fresh `FileSystemRegistry::default()`
when you need a rooted provider or custom policy.

This example discovers the linked provider and opens the current directory as a
rooted authority. It does not create or read `report.csv`, or perform file
operations through the discovered host provider:

<!-- registry-example: local -->
<!-- registry-example: inventory -->
```rust
use qubit_fs::metadata::FileSystemId;
use qubit_fs::path::ConnectionUri;
use qubit_fs_local::LocalFileSystemProvider;
use qubit_fs_local::LocalResourcePolicy;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemRegistry;
use qubit_spi::ProviderDescriptor;
use qubit_spi::ProviderId;
use qubit_spi::ProviderSelection;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = FileSystemRegistry::from_inventory()?;
    assert!(registry.provider_ids().iter().any(|id| id.as_str() == "local-file"));
    let root = std::env::current_dir()?;
    registry.register(LocalFileSystemProvider::rooted_with_descriptor(
        ProviderDescriptor::new(ProviderId::new("rooted-reports")?),
        FileSystemId::new("reports")?,
        &root,
        LocalResourcePolicy::standard(),
    )?)?;
    let config = FileSystemConfig::new(ConnectionUri::parse("file:///report.csv")?)
        .with_selection(ProviderSelection::named("rooted-reports")?);
    let resolution = registry.resolve_config(&config)?;
    assert_eq!(resolution.file_system().properties().info().provider_id(), "rooted-reports");
    assert_eq!(resolution.canonical_uri().as_str(), "file:///report.csv");
    registry.seal();
    assert!(registry.is_sealed());
    Ok(())
}
```

Provider IDs must be unique, and every alias must avoid all registered IDs and
aliases. Changing only the filesystem ID does not avoid a registration
conflict. Explicit selection chooses among providers after successful
registration; it does not bypass registration conflicts.
See [Separate roots with the same URI](#separate-roots-with-the-same-uri) for
how identical URIs can resolve within different roots.

## Core Workflow

Run this program in an empty working directory. It creates `report.csv` and verifies its size.

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

Keep the URI and configuration at the resolution boundary. Downstream code
uses `resolution.file_system()` and `resolution.path()` rather than decoding
the URI again.

Both synchronous and asynchronous resolutions apply the same path validation
rules. Provider-specific URI-to-path decoding remains the provider's job; the
registry validates the resulting path against the configured facade before it
returns the resolution.

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

When a URI scheme is used as a named selection, it is passed through
`ProviderSelection::named`. The selector must be a nonempty ASCII token with an
alphanumeric first and last character. Its body may contain only lowercase
ASCII letters, digits, `-`, `_`, `.`, and `+`. Selector parsing trims
surrounding whitespace and lowercases ASCII letters; `/`, `:`, whitespace,
non-ASCII characters, and other punctuation are rejected. Only the parsed
scheme is used for selection. Authority, userinfo, query, and the raw URI text
are not re-parsed as selector input.

The registry default is resolved as an atomic catalog snapshot: the default
selection and its candidate provider handles are read together. A concurrent
registration or default replacement affects a later snapshot and cannot mix
two catalog versions into one default resolution.
At the SPI layer, this operation is exposed as
`ProviderRegistry::resolve_default_snapshot()` and is performed once for each
default resolution.

### Credentials and async resolution

Use `CredentialRef` only to reference a provider-recognized source:
`DefaultChain`, a profile, environment variable names, or an external provider
ID. Do not place secret material in it. The registry also rejects configuration
credential conflicts before provider creation. This returns
`FileSystemRegistryError::CredentialSourceConflict`; its stable
`reason_code()` is `credential_source_conflict` when an embedded URI
secret and an external `CredentialRef` occupy the same slot. The reason code is
safe structured data and contains no URI, reference, or secret payload.

Register providers and choose the default during startup, then call `seal()`
when configuration is complete. Clones share the sealed state. After sealing,
registration and default-selection updates return
`FileSystemRegistryError::RegistryMutation`; existing resolution remains
available. Handle this error by its type and reason code, rather than by
matching display text.

For async providers, register with `AsyncFileSystemRegistry` and await its
owned-config `resolve_config`, `resolve_uri`, `resolve_selected_config`, or
`resolve_default_config` future. The resulting `AsyncFileSystemResolution`
has the same filesystem/path/canonical-URI shape.

### Provider integration tutorials

Run filesystem examples from an empty working directory; they create their own
report files and subdirectories. Add SPI types with `cargo add qubit-spi@0.14`.

### Separate roots with the same URI

A descriptor ID names a registered factory; its aliases route selections. The
returned facade must report that exact provider ID. Its filesystem ID identifies
the configured filesystem. URI schemes describe supported locations and need
not equal the descriptor ID. Two roots can therefore have the same canonical URI
while containing different data. Preserve the returned facade with the path:

<!-- registry-example: local -->
```rust
use std::time::Duration;

use qubit_fs::metadata::FileSystemId;
use qubit_fs::path::ConnectionUri;
use qubit_fs::read::ReadOptions;
use qubit_fs_local::LocalCopyResourceLimits;
use qubit_fs_local::LocalDeleteResourceLimits;
use qubit_fs_local::LocalFileSystemProvider;
use qubit_fs_local::LocalListResourceLimits;
use qubit_fs_local::LocalResourcePolicy;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemRegistry;
use qubit_spi::ProviderDescriptor;
use qubit_spi::ProviderId;
use qubit_spi::ProviderSelection;

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
    let registry = FileSystemRegistry::default();
    for (id, contents) in [("first", "first report"), ("second", "second report")] {
        let directory = root.join(id);
        std::fs::create_dir_all(&directory)?;
        std::fs::write(directory.join("report.csv"), contents)?;
        registry.register(LocalFileSystemProvider::rooted_with_descriptor(
            ProviderDescriptor::new(ProviderId::new(id)?),
            FileSystemId::new(id)?, &directory, bounded_policy()?,
        )?)?;
    }
    let uri = ConnectionUri::parse("file:///report.csv")?;
    let first = registry.resolve_config(&FileSystemConfig::new(uri.clone())
        .with_selection(ProviderSelection::named("first")?))?;
    let second = registry.resolve_config(&FileSystemConfig::new(uri)
        .with_selection(ProviderSelection::named("second")?))?;
    assert_eq!(first.canonical_uri(), second.canonical_uri());
    assert_ne!(first.file_system().properties().info().id(), second.file_system().properties().info().id());
    assert_eq!(first.file_system().read_all(first.path(), ReadOptions::default(), 64)?, b"first report");
    assert_eq!(second.file_system().read_all(second.path(), ReadOptions::default(), 64)?, b"second report");
    Ok(())
}
```

### Implementing a provider

Implement `ProviderMetadata` and `ServiceProvider<FileSystemSpec>`. This example
composes the established local provider so URI decoding, credentials, and path
rules have a single owner. A new backend instead constructs its facade and calls
`FileSystemResolution::try_new(facade, decoded_path, canonical_uri)` itself.
That constructor validates path semantics/limits and requires an advertised
canonical scheme; an empty scheme list fails. Registry identity checks follow.

<!-- registry-example: local -->
```rust
use std::time::Duration;

use qubit_fs::FsError;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::path::ConnectionUri;
use qubit_fs_local::LocalCopyResourceLimits;
use qubit_fs_local::LocalDeleteResourceLimits;
use qubit_fs_local::LocalFileSystemProvider;
use qubit_fs_local::LocalListResourceLimits;
use qubit_fs_local::LocalResourcePolicy;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemRegistry;
use qubit_fs_registry::FileSystemResolution;
use qubit_fs_registry::FileSystemSpec;
use qubit_spi::ProviderDescriptor;
use qubit_spi::ProviderId;
use qubit_spi::ProviderMetadata;
use qubit_spi::ServiceProvider;
use qubit_spi::error::ProviderFailure;

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

struct ReportProvider { local: LocalFileSystemProvider }
impl ProviderMetadata for ReportProvider {
    fn descriptor(&self) -> ProviderDescriptor { self.local.descriptor() }
}
impl ServiceProvider<FileSystemSpec> for ReportProvider {
    fn create_configured(&self, config: &FileSystemConfig)
        -> Result<FileSystemResolution, ProviderFailure<FsError>>
    {
        // Reuse the local adapter's credential checks and percent-decoding rules.
        self.local.create_configured(config)
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = ReportProvider { local: LocalFileSystemProvider::rooted_with_descriptor(
        ProviderDescriptor::new(ProviderId::new("report-storage")?).with_aliases(["file"])?,
        FileSystemId::new("report-root")?, &std::env::current_dir()?, bounded_policy()?,
    )? };
    let registry = FileSystemRegistry::default();
    registry.register(provider)?;
    let resolution = registry.resolve_uri(&ConnectionUri::parse("file:///not-created.csv")?)?;
    assert_eq!(resolution.file_system().properties().info().provider_id(), "report-storage");
    // Resolution validates properties; it does not require the file to exist.
    assert_eq!(resolution.path().as_str(), "/not-created.csv");
    Ok(())
}
```

### Inspecting fallback decisions

These two intentionally failing providers make the policy difference visible.
`OnAbsence` stops on invalid configuration; `OnAnyError` attempts the second
provider. Read attempts in call order and inspect the decisive failure rather
than matching display text. A successful later provider ends the chain and
returns its resolution instead of an error aggregate.

```rust
use qubit_fs::FsError;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::path::ConnectionUri;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemRegistry;
use qubit_fs_registry::FileSystemRegistryError;
use qubit_fs_registry::FileSystemResolution;
use qubit_fs_registry::FileSystemSpec;
use qubit_spi::FallbackPolicy;
use qubit_spi::ProviderDescriptor;
use qubit_spi::ProviderId;
use qubit_spi::ProviderMetadata;
use qubit_spi::ProviderSelection;
use qubit_spi::ServiceProvider;
use qubit_spi::error::ProviderFailure;
use qubit_spi::error::ProviderFailureKind;

struct Rejected { id: &'static str, invalid: bool }
impl ProviderMetadata for Rejected {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(ProviderId::new(self.id).expect("static ID"))
    }
}
impl ServiceProvider<FileSystemSpec> for Rejected {
    fn create_configured(&self, _: &FileSystemConfig)
        -> Result<FileSystemResolution, ProviderFailure<FsError>>
    {
        if self.invalid {
            Err(ProviderFailure::invalid_configuration(FsError::new(
                FsErrorKind::InvalidOptions, FsOperation::Provider, "invalid provider options",
            )))
        } else {
            Err(ProviderFailure::unavailable(FsError::new(
                FsErrorKind::ProviderUnavailable, FsOperation::Provider, "provider unavailable",
            )))
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = FileSystemRegistry::default();
    registry.register(Rejected { id: "first", invalid: true })?;
    registry.register(Rejected { id: "second", invalid: false })?;
    let config = FileSystemConfig::new(ConnectionUri::parse("file:///report.csv")?);
    for (policy, count, decisive) in [
        (FallbackPolicy::OnAbsence, 1, "first"),
        (FallbackPolicy::OnAnyError, 2, "second"),
    ] {
        let selection = ProviderSelection::chain(["first", "second"])?.with_fallback_policy(policy);
        let error = registry.resolve_selected_config(&selection, &config).expect_err("fixture providers fail");
        let FileSystemRegistryError::Creation(creation) = error else { panic!("creation aggregate expected") };
        assert_eq!(creation.attempts().len(), count);
        assert_eq!(creation.attempts()[0].failure().kind(), ProviderFailureKind::InvalidConfiguration);
        assert_eq!(creation.decisive_attempt().provider_id().as_str(), decisive);
    }
    Ok(())
}
```

### Referencing credentials

A username alone is not an embedded secret. Passwords and sensitive query
values conflict with an external reference. This example shows the exact error
boundary without connecting to any storage service:

```rust
use qubit_fs::path::ConnectionUri;
use qubit_fs_registry::CredentialRef;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemRegistry;
use qubit_fs_registry::FileSystemRegistryError;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = FileSystemRegistry::default();
    let config = FileSystemConfig::new(ConnectionUri::parse("s3://user:example-secret@bucket/key")?)
        .with_credential(CredentialRef::DefaultChain);
    let error = registry.resolve_config(&config).expect_err("conflicting sources");
    assert!(matches!(error, FileSystemRegistryError::CredentialSourceConflict));
    assert_eq!(error.reason_code(), "credential_source_conflict");
    assert!(!format!("{error:?}").contains("example-secret"));
    let username = FileSystemConfig::new(ConnectionUri::parse("s3://user@bucket/key")?)
        .with_credential(CredentialRef::DefaultChain);
    // Credentials are valid; an empty registry now fails in the resolution stage.
    assert!(matches!(registry.resolve_config(&username), Err(FileSystemRegistryError::Resolution(_))));
    Ok(())
}
```

### Owning an asynchronous resolution

```bash
cargo add qubit-fs-registry@0.8 --features async
cargo add futures@0.3
```

This helper function demonstrates ownership of an asynchronous resolution future using an already registered provider. See the linked [complete provider example](async_resolution_example.md) for registration and execution. The registry has no executor runtime dependency.

Pass a registry that your application has already populated. `resolve_uri` creates an owned future; after creating it, this function drops its registry handle and awaits the result. Other registry clones may still exist: the example shows that this future does not borrow the handle passed here. See the [complete runnable provider example](async_resolution_example.md) for registration and setup.

<!-- registry-example: async -->
```rust,no_run
use qubit_fs::path::ConnectionUri;
use qubit_fs_registry::AsyncFileSystemRegistry;
use qubit_fs_registry::AsyncFileSystemResolution;
use qubit_fs_registry::FileSystemRegistryResult;

async fn resolve_after_registration(
    registry: AsyncFileSystemRegistry,
) -> Result<AsyncFileSystemResolution, Box<dyn std::error::Error>> {
    let future = registry.resolve_uri(ConnectionUri::parse("memory:///report.csv")?);
    drop(registry);
    let resolution: FileSystemRegistryResult<AsyncFileSystemResolution> = future.await;
    Ok(resolution?)
}
fn main() {}
```

The initial default selection is `auto()` with `OnAbsence`. This does not change
URI/config routing. Registration and default replacement are synchronous;
provider work runs outside catalog locks. The registry adds neither automatic
blocking adapters nor a configured-filesystem cache. Credential references alone
are not sufficient cache identities: principal, scope and credential rotation
must be accounted for by a provider before sharing authenticated resources.

## Errors and Diagnostics

Registry operations return `FileSystemRegistryResult` and preserve structured
registration, selection, resolution, and provider-creation diagnostics in
`FileSystemRegistryError`. Provider creation may fail after a provider has
been selected; inspect the typed error rather than replacing it with a generic
message. A registry error can convert to `FsError` while retaining the typed
registry error as its source.
Formatted registry errors include only applicable safe selector and provider context. Their
fields are rendered with the immutable built-in standard policy from
`qubit_redact::Redactor::standard()`; registry `Display` and `Debug` do not read
or follow later replacements of the process-wide application-default redactor.
They do not recursively expand a provider `source()` or emit an internal
message as unredacted text. Use the typed `Error::source()` chain explicitly
when structured error handling needs it.

Provider creation failures retain their SPI classification:
`Unsupported`, `Unavailable`, `InvalidConfiguration`, or
`InitializationFailed`. The default `FallbackPolicy::OnAbsence` continues only
after `Unsupported` and `Unavailable`; `Never` always stops, and `OnAnyError`
continues after every leaf failure. Named selections never fall back. Resolution
errors raised before a provider is called do not create provider attempts.

The canonical URI is the selected provider's credential-free location for this
resolution. It is not a universal URI normalization or a replacement for the
connection URI. Its scheme must be advertised by the returned filesystem
facade; provider-specific authority, path normalization, and URI-to-path
semantics remain the provider's responsibility. Do not treat it as a
cross-provider global identity.

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

## Published Documentation Check

The CI feature matrix checks the default, `async`-only, `inventory`-only, and
combined `async` plus `inventory` configurations.

Before publishing, run `python3 scripts/check_published_docs.py .` with Python
3.12+ and the Rust toolchain from `rust-toolchain.toml`. The checker snapshots
the registry and the `qubit-fs`, `qubit-fs-local`, and `qubit-local-files`
sibling inputs before packaging and compiling examples. It records versions,
source paths, Git HEAD and dirty state, plus a SHA-256 digest over sorted input
paths, lengths, contents, and executable bits. A change during copying fails
the check and preserves diagnostics; later source changes do not alter the
prepared inputs. This validates the package archive and examples against those
fixed sibling source snapshots, not the contents of their crates.io releases.
Rerun the check after changing source, the manifest, or examples.

Before publishing the registry, publish the required sibling releases first,
then run strict registry consumption:

```bash
python3 scripts/check_published_docs.py . --dependency-source registry
```

Strict mode resolves dependencies from crates.io and checks the packaged
archive, default and all-feature doctests, and all shipped Markdown examples.
It uses no sibling path overrides or provider shim and fails without fallback
if a required release is unavailable or a dependency resolves outside crates.io.
Run the manual **Published Registry Consumption** workflow on the exact release
ref, and publish only after success. The result records resolved versions and
the source digest; failed runs preserve individual Cargo logs.

## Further Reading

- [README](../README.md)
- [中文用户手册](user_guide.zh_CN.md)
- [API documentation](https://docs.rs/qubit-fs-registry)

- [Design / 设计](file_system_registry_design.md) · [中文设计](file_system_registry_design.zh_CN.md)
- [Migration / 迁移](registry_contract_migration.md) · [中文迁移](registry_contract_migration.zh_CN.md)
