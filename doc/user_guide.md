# qubit-fs-registry User Guide

[中文](user_guide.zh_CN.md) · [README](../README.md) · [API documentation](https://docs.rs/qubit-fs-registry)

## Purpose and Audience

This guide is for application and provider authors who need to bind
`qubit-fs` to runtime-registered filesystem providers. It covers the current
`qubit-fs-registry` 0.2 API, including synchronous and asynchronous resolution.

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
the canonical URI is available for safe identification. Resolution validation
is local and metadata-based: creating a resolution does not call `stat`, touch
the backend, or prove that the target exists. Call `stat` or another operation
when the application needs resource state.

## Installation and Minimal Configuration

```bash
cargo add qubit-fs@0.3 qubit-fs-registry@0.2
cargo add qubit-fs-local@0.2 --features registry
```

Provider crates that create explicit SPI selections or use low-level provider
catalog types must add `qubit-spi` directly with `cargo add qubit-spi@0.11`;
those SPI-owned types are not re-exported by this crate.

## Core Workflow

Run this program in an empty working directory. It creates `report.csv` and verifies its size.

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

## Provider Integration Tutorials

Run filesystem examples from an empty working directory; they create their own
report files and subdirectories. Add SPI types with `cargo add qubit-spi@0.11`.

### Separate roots with the same URI

A descriptor ID names a registered factory; its aliases route selections. The
returned facade must report that exact provider ID. Its filesystem ID identifies
the configured filesystem. URI schemes describe supported locations and need
not equal the descriptor ID. Two roots can therefore have the same canonical URI
while containing different data. Preserve the returned facade with the path:

```rust
use qubit_fs::metadata::FileSystemId;
use qubit_fs::path::ConnectionUri;
use qubit_fs::read::ReadOptions;
use qubit_fs_local::LocalFileSystemProvider;
use qubit_fs_local::LocalResourcePolicy;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemRegistry;
use qubit_spi::ProviderDescriptor;
use qubit_spi::ProviderId;
use qubit_spi::ProviderSelection;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::current_dir()?;
    let registry = FileSystemRegistry::default();
    for (id, contents) in [("first", "first report"), ("second", "second report")] {
        let directory = root.join(id);
        std::fs::create_dir_all(&directory)?;
        std::fs::write(directory.join("report.csv"), contents)?;
        registry.register(LocalFileSystemProvider::rooted_with_descriptor(
            ProviderDescriptor::new(ProviderId::new(id)?),
            FileSystemId::new(id)?, &directory, LocalResourcePolicy::unbounded(),
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

```rust
use qubit_fs::FsError;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::path::ConnectionUri;
use qubit_fs_local::LocalFileSystemProvider;
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
        FileSystemId::new("report-root")?, &std::env::current_dir()?, LocalResourcePolicy::unbounded(),
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
cargo add qubit-fs-registry@0.2 --features async
cargo add futures@0.3
```

`futures` is an executor for this example, not a registry runtime dependency.
The local provider above is synchronous. The following self-contained,
properties-only in-memory provider demonstrates the async creation contract;
its SPI operations deliberately return unsupported errors because no file IO is
needed for resolution. The future captures the catalog before polling, invokes
the provider only when polled, and remains usable after the registry is dropped.
Clones share registrations and defaults; later changes affect later snapshots.

<!-- registry-example: async -->
```rust
use qubit_fs::AsyncFileSystem;
use qubit_fs::FileSystem;
use qubit_fs::FsError;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::directory::CreateDirectoryOutcome;
use qubit_fs::directory::DeleteOutcome;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;
use qubit_fs::path::Uri;
use qubit_fs::rename::RenameFailureState;
use qubit_fs::rename::RenameOutcome;
use qubit_fs::spi::AsyncFileSystemSpi;
use qubit_fs::spi::CreateDirectoryRequest;
use qubit_fs::spi::CreateTempDirectoryRequest;
use qubit_fs::spi::CreateTempFileRequest;
use qubit_fs::spi::DeleteDirectoryRequest;
use qubit_fs::spi::DeleteFileRequest;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::ListRequest;
use qubit_fs::spi::OpenReaderRequest;
use qubit_fs::spi::OpenWriterRequest;
use qubit_fs::spi::OpenedAsyncDirectoryStream;
use qubit_fs::spi::OpenedAsyncReader;
use qubit_fs::spi::OpenedAsyncTempDirectory;
use qubit_fs::spi::OpenedAsyncTempFile;
use qubit_fs::spi::OpenedAsyncWriter;
use qubit_fs::spi::OpenedDirectoryStream;
use qubit_fs::spi::OpenedReader;
use qubit_fs::spi::OpenedTempDirectory;
use qubit_fs::spi::OpenedTempFile;
use qubit_fs::spi::OpenedWriter;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::RenameRequest;
use qubit_fs::spi::SpiFuture;
use qubit_fs::spi::SpiRenameFailure;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;
fn properties(
    provider_id: &'static str,
    scheme: Option<&str>,
    limits: FileSystemLimits,
    path_constraints: PathConstraints,
    path_semantics: PathSemantics,
) -> ProviderProperties {
    let mut info = FileSystemInfo::new(
        FileSystemId::new("registry-test-fs").expect("valid filesystem ID"),
        provider_id,
        path_semantics,
    );
    if let Some(scheme) = scheme {
        info = info.with_scheme(scheme).expect("valid test scheme");
    }
    ProviderProperties::new(
        info,
        ProviderOperations::new(),
        FileSystemCapabilities::new(),
        limits,
        path_constraints,
        SymlinkPolicy::Reject,
    )
    .expect("valid test properties")
}
fn unused() -> FsError {
    FsError::new(
        FsErrorKind::UnsupportedOperation,
        FsOperation::Other,
        "unused test operation",
    )
}
struct AsyncPropertiesOnlySpi {
    provider_id: &'static str,
    scheme: Option<&'static str>,
    limits: FileSystemLimits,
    path_constraints: PathConstraints,
    path_semantics: PathSemantics,
}
impl AsyncFileSystemSpi for AsyncPropertiesOnlySpi {
    fn properties(&self) -> ProviderProperties {
        properties(
            self.provider_id,
            self.scheme,
            self.limits,
            self.path_constraints.clone(),
            self.path_semantics,
        )
    }

    fn stat<'a>(&'a self, _: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
        Box::pin(async { Err(unused()) })
    }

    fn list<'a>(&'a self, _: ListRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncDirectoryStream>> {
        Box::pin(async { Err(unused()) })
    }

    fn open_reader<'a>(&'a self, _: OpenReaderRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncReader>> {
        Box::pin(async { Err(unused()) })
    }

    fn open_writer<'a>(&'a self, _: OpenWriterRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncWriter>> {
        Box::pin(async { Err(unused()) })
    }

    fn create_directory<'a>(
        &'a self,
        _: CreateDirectoryRequest<'a>,
    ) -> SpiFuture<'a, FsResult<CreateDirectoryOutcome>> {
        Box::pin(async { Err(unused()) })
    }

    fn delete_file<'a>(&'a self, _: DeleteFileRequest<'a>) -> SpiFuture<'a, FsResult<DeleteOutcome>> {
        Box::pin(async { Err(unused()) })
    }

    fn delete_directory<'a>(&'a self, _: DeleteDirectoryRequest<'a>) -> SpiFuture<'a, FsResult<DeleteOutcome>> {
        Box::pin(async { Err(unused()) })
    }

    fn rename<'a>(&'a self, _: RenameRequest<'a>) -> SpiFuture<'a, Result<RenameOutcome, SpiRenameFailure>> {
        Box::pin(async { Err(SpiRenameFailure::new(unused(), RenameFailureState::Unchanged)) })
    }

    fn create_temp_file<'a>(&'a self, _: CreateTempFileRequest) -> SpiFuture<'a, FsResult<OpenedAsyncTempFile>> {
        Box::pin(async { Err(unused()) })
    }

    fn create_temp_directory<'a>(
        &'a self,
        _: CreateTempDirectoryRequest,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncTempDirectory>> {
        Box::pin(async { Err(unused()) })
    }
}
use qubit_fs_registry::AsyncFileSystemRegistry;
use qubit_fs_registry::AsyncFileSystemResolution;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemSpec;
use qubit_fs::path::ConnectionUri;
use qubit_spi::AsyncServiceProvider;
use qubit_spi::ProviderDescriptor;
use qubit_spi::ProviderId;
use qubit_spi::ProviderMetadata;
use qubit_spi::ProviderFuture;
use qubit_spi::error::ProviderFailure;

// A properties-only in-memory example: it demonstrates resolution, not file IO.
struct MemoryProvider { filesystem: AsyncFileSystem }
impl ProviderMetadata for MemoryProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(ProviderId::new("example").expect("static ID"))
            .with_aliases(["memory"]).expect("static alias")
    }
}
impl AsyncServiceProvider<FileSystemSpec> for MemoryProvider {
    fn create_configured<'a>(&'a self, config: &'a FileSystemConfig)
        -> ProviderFuture<'a, Result<AsyncFileSystemResolution, ProviderFailure<FsError>>>
    {
        Box::pin(async move {
            let uri = config.uri().try_to_uri().map_err(ProviderFailure::invalid_configuration)?;
            if uri.as_str() != "memory:///report.csv" || !config.options().is_empty()
                || !config.metadata().is_empty() || config.credential().is_some() {
                return Err(ProviderFailure::invalid_configuration(FsError::new(
                    FsErrorKind::InvalidOptions, FsOperation::Provider, "unsupported example configuration",
                )));
            }
            AsyncFileSystemResolution::try_new(self.filesystem.clone(),
                Path::parse("/report.csv").expect("static path"), uri)
                .map_err(ProviderFailure::initialization_failed)
        })
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let filesystem = AsyncFileSystem::from_spi(AsyncPropertiesOnlySpi {
        provider_id: "example", scheme: Some("memory"), limits: FileSystemLimits::unknown(),
        path_constraints: PathConstraints::absolute(), path_semantics: PathSemantics::Hierarchical,
    })?;
    let registry = AsyncFileSystemRegistry::default();
    registry.register(MemoryProvider { filesystem })?;
    let future = registry.resolve_uri(ConnectionUri::parse("memory:///report.csv")?);
    drop(registry);
    let resolution = futures::executor::block_on(future)?;
    assert_eq!(resolution.path().as_str(), "/report.csv");
    assert_eq!(resolution.canonical_uri().as_str(), "memory:///report.csv");
    Ok(())
}
```

The initial default selection is `auto()` with `OnAbsence`. This does not change
URI/config routing. Registration and default replacement are synchronous;
provider work runs outside catalog locks. The registry adds neither automatic
blocking adapters nor a configured-filesystem cache. Credential references alone
are not sufficient cache identities: principal, scope and credential rotation
must be accounted for by a provider before sharing authenticated resources.

## Further Reading

- [README](../README.md)
- [中文用户手册](user_guide.zh_CN.md)
- [API documentation](https://docs.rs/qubit-fs-registry)

- [Design / 设计](file_system_registry_design.md) · [中文设计](file_system_registry_design.zh_CN.md)
- [Migration / 迁移](registry_contract_migration.md) · [中文迁移](registry_contract_migration.zh_CN.md)
