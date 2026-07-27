// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::error::Error;
use std::future::Future;
use std::io::Result as IoResult;
use std::pin::Pin;
use std::sync::{
    Arc,
    Mutex,
    mpsc,
};
use std::task::{
    Context,
    Poll,
    Waker,
};

use qubit_fs::{
    AsyncFileReader,
    AsyncFileSystem,
    FileKind,
    FileLocation,
    FileMetadata,
    FileSystemCapabilities,
    FileSystemId,
    FileSystemInfo,
    FileSystemProperties,
    FsError,
    FsErrorKind,
    FsFuture,
    FsOperation,
    FsPath,
    FsUri,
    OpenedFileInfo,
    PathSemantics,
    ReadOptions,
    UserMetadata,
};
use qubit_fs_registry::{
    AsyncFileSystemProvider,
    AsyncFileSystemRegistry,
    CredentialRef,
    FileSystemConfig,
    FileSystemRegistryError,
    FileSystemResolution,
    FileSystemSpec,
};
use qubit_io::AsyncInput;
use qubit_spi::error::{
    ProviderFailure,
    ProviderResolutionError,
};
use qubit_spi::{
    AsyncServiceProvider,
    FallbackPolicy,
    ProviderDescriptor,
    ProviderFuture,
    ProviderId,
    ProviderMetadata,
    ProviderSelection,
    ProviderSelectionTargetRef,
};

#[derive(Debug)]
struct AsyncOnlyFs {
    info: FileSystemInfo,
}

impl AsyncOnlyFs {
    fn new(id: &str) -> Self {
        Self {
            info: FileSystemInfo::new(
                FileSystemId::new(id).expect("filesystem id should parse"),
                ProviderId::new("async-capture")
                    .expect("provider id should parse"),
                PathSemantics::Hierarchical,
            ),
        }
    }
}

impl FileSystemProperties for AsyncOnlyFs {
    fn info(&self) -> &FileSystemInfo {
        &self.info
    }

    fn capabilities(&self) -> FileSystemCapabilities {
        FileSystemCapabilities::default()
    }

    fn limits(&self) -> &qubit_fs::FileSystemLimits {
        static LIMITS: qubit_fs::FileSystemLimits =
            qubit_fs::FileSystemLimits::unknown();
        &LIMITS
    }
}

impl AsyncFileSystem for AsyncOnlyFs {
    fn stat_async<'a>(
        &'a self,
        _path: &'a FsPath,
    ) -> FsFuture<'a, FileMetadata> {
        Box::pin(async { Ok(FileMetadata::new(FileKind::File)) })
    }

    fn open_reader_async<'a>(
        &'a self,
        path: &'a FsPath,
        _options: ReadOptions,
    ) -> FsFuture<'a, AsyncFileReader> {
        let location = FileLocation::new(self.info.id().clone(), path.clone());
        Box::pin(async move {
            Ok(AsyncFileReader::new(
                EmptyAsyncInput,
                OpenedFileInfo::new(location),
            ))
        })
    }
}

struct EmptyAsyncInput;

impl AsyncInput for EmptyAsyncInput {
    type Item = u8;

    unsafe fn poll_read_unchecked(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _output: &mut [u8],
        _index: usize,
        _count: usize,
    ) -> Poll<IoResult<usize>> {
        Poll::Ready(Ok(0))
    }
}

struct CapturingAsyncProvider {
    descriptor: ProviderDescriptor,
    captured: Arc<Mutex<Option<FileSystemConfig>>>,
    path: &'static str,
}

impl ProviderMetadata for CapturingAsyncProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        self.descriptor.clone()
    }
}

impl AsyncServiceProvider<FileSystemSpec> for CapturingAsyncProvider {
    fn create_configured<'a>(
        &'a self,
        config: &'a FileSystemConfig,
    ) -> ProviderFuture<
        'a,
        Result<FileSystemResolution<dyn AsyncFileSystem>, ProviderFailure<FsError>>,
    > {
        *self.captured.lock().expect("lock should succeed") =
            Some(config.clone());
        let fs: Arc<dyn AsyncFileSystem> =
            Arc::new(AsyncOnlyFs::new("async-only"));
        let path = FsPath::parse_literal(self.path)
            .expect("provider path should parse");
        let uri = config.uri().clone();
        Box::pin(async move { Ok(FileSystemResolution::new(fs, path, uri)) })
    }
}

struct UnavailableAsyncProvider {
    descriptor: ProviderDescriptor,
}

struct ErrorAsyncProvider {
    descriptor: ProviderDescriptor,
    kind: FsErrorKind,
}

impl ProviderMetadata for ErrorAsyncProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        self.descriptor.clone()
    }
}

impl AsyncServiceProvider<FileSystemSpec> for ErrorAsyncProvider {
    fn create_configured<'a>(
        &'a self,
        _config: &'a FileSystemConfig,
    ) -> ProviderFuture<
        'a,
        Result<FileSystemResolution<dyn AsyncFileSystem>, ProviderFailure<FsError>>,
    > {
        let kind = self.kind;
        Box::pin(async move {
            Err(provider_failure(kind, "provider creation failed"))
        })
    }
}

impl ProviderMetadata for UnavailableAsyncProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        self.descriptor.clone()
    }
}

impl AsyncServiceProvider<FileSystemSpec> for UnavailableAsyncProvider {
    fn create_configured<'a>(
        &'a self,
        _config: &'a FileSystemConfig,
    ) -> ProviderFuture<
        'a,
        Result<FileSystemResolution<dyn AsyncFileSystem>, ProviderFailure<FsError>>,
    > {
        Box::pin(async {
            Err(provider_failure(
                FsErrorKind::ProviderUnavailable,
                "provider is unavailable",
            ))
        })
    }
}

/// Creates a typed provider failure that retains the original filesystem error.
fn provider_failure(
    kind: FsErrorKind,
    message: &'static str,
) -> ProviderFailure<FsError> {
    let error = FsError::new(kind, FsOperation::Provider, message);
    match kind {
        FsErrorKind::ProviderUnavailable => ProviderFailure::unavailable(error),
        FsErrorKind::UnsupportedOperation
        | FsErrorKind::UnsupportedCapability
        | FsErrorKind::RequirementNotMet => ProviderFailure::unsupported(error),
        FsErrorKind::InvalidUri | FsErrorKind::InvalidPath | FsErrorKind::InvalidOptions => {
            ProviderFailure::invalid_configuration(error)
        }
        _ => ProviderFailure::initialization_failed(error),
    }
}

fn ready<F>(future: F) -> F::Output
where
    F: Future,
{
    let mut context = Context::from_waker(Waker::noop());
    let mut future = std::pin::pin!(future);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("test future should be immediately ready"),
    }
}

/// Verifies configuration futures retain only the configuration borrow.
#[test]
fn test_config_resolution_futures_outlive_registry() {
    let config = FileSystemConfig::new(
        FsUri::parse("missing:///resource").expect("URI should parse"),
    );
    let selection =
        ProviderSelection::named("missing").expect("selection should parse");
    let (configured, selected, default) = {
        let registry = AsyncFileSystemRegistry::default();
        (
            registry.resolve_config_async(&config),
            registry.resolve_selected_config_async(&selection, &config),
            registry.resolve_default_config_async(&config),
        )
    };

    for error in [
        ready(configured).expect_err("configured resolution should fail"),
        ready(selected).expect_err("selected resolution should fail"),
        ready(default).expect_err("default resolution should fail"),
    ] {
        assert!(matches!(error, FileSystemRegistryError::Resolution(_)));
    }
}

/// Verifies URI convenience futures own the URI and provider snapshot.
#[test]
fn test_uri_resolution_futures_outlive_registry_and_uri() {
    let (filesystem, resource) = {
        let registry = AsyncFileSystemRegistry::default();
        registry
            .register(CapturingAsyncProvider {
                descriptor: descriptor("async-capture"),
                captured: Arc::new(Mutex::new(None)),
                path: "resolved-after-drop",
            })
            .expect("provider should register");
        let uri = FsUri::parse("async-capture:///resource")
            .expect("URI should parse");

        (
            registry.file_system_uri_async(&uri),
            registry.resource_uri_async(&uri),
        )
    };

    assert_eq!(
        "async-only",
        ready(filesystem)
            .expect("filesystem future should retain provider snapshot")
            .info()
            .id()
            .as_str(),
    );
    assert_eq!(
        "resolved-after-drop",
        ready(resource)
            .expect("resource future should retain provider snapshot")
            .path()
            .as_str(),
    );
}

/// Verifies an empty async registry exposes consistent catalog state and
/// low-level resolution errors.
#[test]
fn test_async_registry_exposes_empty_catalog_and_resolution_errors() {
    let registry = AsyncFileSystemRegistry::default();
    let selection =
        ProviderSelection::named("missing").expect("selection should parse");

    assert!(registry.is_empty());
    assert_eq!(0, registry.len());
    assert!(registry.descriptors().is_empty());
    assert!(registry.resolve_selected(&selection).is_err());
    assert!(registry.resolve().is_err());
}

#[test]
fn test_async_registry_accepts_async_only_provider_and_passes_complete_config()
{
    let captured = Arc::new(Mutex::new(None));
    let registry = AsyncFileSystemRegistry::default();
    registry
        .register(CapturingAsyncProvider {
            descriptor: descriptor("async-capture"),
            captured: captured.clone(),
            path: "provider-decoded/%252F",
        })
        .expect("async provider should register");
    let config = FileSystemConfig::new(
        FsUri::parse("unrelated:///raw%2Fkey").expect("URI should parse"),
    )
    .with_selection(
        ProviderSelection::named("async-capture")
            .expect("selection should parse"),
    )
    .with_options(
        UserMetadata::new()
            .with("region", "test-1")
            .expect("options should be valid"),
    )
    .with_credentials(CredentialRef::Profile {
        name: "integration".to_owned(),
    });

    let resource = ready(registry.resource_async(&config))
        .expect("complete config should resolve asynchronously");

    assert_eq!("provider-decoded/%252F", resource.path().as_str());
    assert_eq!(
        Some(config),
        captured.lock().expect("lock should succeed").clone()
    );
    let reader = ready(resource.open_reader_async(ReadOptions::default()))
        .expect("reader should open");
    assert_eq!(
        resource.location().uri(),
        reader.info().location().uri(),
        "registry canonical identity should reach asynchronous handles",
    );
}

#[test]
fn test_async_registry_applies_absence_fallback_after_awaiting_creation() {
    let registry = AsyncFileSystemRegistry::default();
    registry
        .register(UnavailableAsyncProvider {
            descriptor: descriptor("offline"),
        })
        .expect("unavailable provider should register");
    registry
        .register(CapturingAsyncProvider {
            descriptor: descriptor("async-capture"),
            captured: Arc::new(Mutex::new(None)),
            path: "fallback-result",
        })
        .expect("fallback provider should register");
    let selection = ProviderSelection::chain(["offline", "async-capture"])
        .expect("selection should parse")
        .with_fallback_policy(FallbackPolicy::OnAbsence);
    let config = FileSystemConfig::new(
        FsUri::parse("async-capture:///resource").expect("URI should parse"),
    )
    .with_selection(selection);

    let resolution = ready(registry.resolve_config_async(&config))
        .expect("absence fallback should reach the second provider");

    assert_eq!("fallback-result", resolution.path().as_str());
}

#[test]
fn test_async_registry_rejects_partially_unknown_strict_chain_before_await() {
    let registry = AsyncFileSystemRegistry::default();
    registry
        .register(CapturingAsyncProvider {
            descriptor: descriptor("known"),
            captured: Arc::new(Mutex::new(None)),
            path: "known",
        })
        .expect("known provider should register");
    let selection = ProviderSelection::chain(["missing", "known"])
        .expect("strict chain should parse");
    let config = FileSystemConfig::new(
        FsUri::parse("known:///resource").expect("URI should parse"),
    );

    let error =
        ready(registry.resolve_selected_config_async(&selection, &config))
            .expect_err("strict chain should fail before provider creation");

    assert!(
        error
            .source()
            .and_then(|source| source.downcast_ref::<ProviderResolutionError>())
            .is_some(),
    );
}

#[test]
fn test_async_registry_allows_explicit_missing_chain_entries() {
    let registry = AsyncFileSystemRegistry::default();
    registry
        .register(CapturingAsyncProvider {
            descriptor: descriptor("known"),
            captured: Arc::new(Mutex::new(None)),
            path: "known",
        })
        .expect("known provider should register");
    let selection =
        ProviderSelection::chain_allowing_missing(["missing", "known"])
            .expect("lenient chain should parse");
    let config = FileSystemConfig::new(
        FsUri::parse("known:///resource").expect("URI should parse"),
    );

    assert_eq!(
        "known",
        ready(registry.resolve_selected_config_async(&selection, &config))
            .expect("known provider should create")
            .path()
            .as_str(),
    );
}

#[test]
fn test_async_registry_rejects_conflicting_provider_selectors() {
    let registry = AsyncFileSystemRegistry::default();
    registry
        .register(CapturingAsyncProvider {
            descriptor: descriptor("async-capture"),
            captured: Arc::new(Mutex::new(None)),
            path: "first",
        })
        .expect("first provider should register");

    let error = registry
        .register(CapturingAsyncProvider {
            descriptor: descriptor("async-capture"),
            captured: Arc::new(Mutex::new(None)),
            path: "second",
        })
        .expect_err("duplicate selector should fail atomically");

    assert!(matches!(error, FileSystemRegistryError::Registration(_)));
    assert_eq!(
        vec!["async-capture"],
        registry
            .provider_ids()
            .iter()
            .map(ProviderId::as_str)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_async_registry_exposes_default_and_uri_convenience_paths() {
    let registry = AsyncFileSystemRegistry::default();
    assert!(matches!(
        registry.default_selection().target(),
        ProviderSelectionTargetRef::Auto,
    ));
    let provider: Arc<AsyncFileSystemProvider> =
        Arc::new(CapturingAsyncProvider {
            descriptor: descriptor("async-capture"),
            captured: Arc::new(Mutex::new(None)),
            path: "resolved",
        });
    registry
        .register_shared(provider)
        .expect("shared provider should register");
    let selection = ProviderSelection::named("async-capture")
        .expect("selection should parse");
    registry.set_default_selection(selection.clone());
    assert_eq!(selection.target(), registry.default_selection().target());

    let uri =
        FsUri::parse("async-capture:///resource").expect("URI should parse");
    let config =
        FileSystemConfig::new(uri.clone()).with_selection(selection.clone());
    assert_eq!(
        "resolved",
        ready(registry.resolve_default_config_async(&config))
            .expect("default configuration should resolve")
            .path()
            .as_str(),
    );
    assert_eq!(
        "resolved",
        ready(registry.resolve_selected_config_async(&selection, &config))
            .expect("selected configuration should resolve")
            .path()
            .as_str(),
    );
    assert_eq!(
        "async-only",
        ready(registry.file_system_async(&config))
            .expect("filesystem configuration should resolve")
            .info()
            .id()
            .as_str(),
    );
    assert_eq!(
        "async-only",
        ready(registry.file_system_uri_async(&uri))
            .expect("URI filesystem configuration should resolve")
            .info()
            .id()
            .as_str(),
    );
    assert_eq!(
        "resolved",
        ready(registry.resource_uri_async(&uri))
            .expect("URI resource configuration should resolve")
            .path()
            .as_str(),
    );
}

/// Verifies explicit selection rejects a conflicting configuration selection.
#[test]
fn test_async_registry_selected_configuration_rejects_conflicting_embedded_selection()
 {
    let registry = AsyncFileSystemRegistry::default();
    let selection =
        ProviderSelection::named("requested").expect("selection should parse");
    let config = FileSystemConfig::new(
        FsUri::parse("unrelated:///resource").expect("URI should parse"),
    )
    .with_selection(
        ProviderSelection::named("embedded").expect("selection should parse"),
    );

    let error =
        ready(registry.resolve_selected_config_async(&selection, &config))
            .expect_err("conflicting provider selections should be rejected");

    assert!(matches!(error, FileSystemRegistryError::SelectionConflict { .. }));
}

/// Verifies default selection rejects a conflicting configuration selection.
#[test]
fn test_async_registry_default_configuration_rejects_conflicting_embedded_selection()
 {
    let registry = AsyncFileSystemRegistry::default();
    registry.set_default_selection(
        ProviderSelection::named("default").expect("selection should parse"),
    );
    let config = FileSystemConfig::new(
        FsUri::parse("unrelated:///resource").expect("URI should parse"),
    )
    .with_selection(
        ProviderSelection::named("embedded").expect("selection should parse"),
    );

    let error = ready(registry.resolve_default_config_async(&config))
        .expect_err("conflicting provider selections should be rejected");

    assert!(matches!(error, FileSystemRegistryError::SelectionConflict { .. }));
}

#[test]
fn test_empty_async_registry_reports_provider_unavailable_from_every_entry_point()
 {
    let registry = AsyncFileSystemRegistry::default();
    let uri = FsUri::parse("missing:///resource").expect("URI should parse");
    let config = FileSystemConfig::new(uri.clone());
    let selection =
        ProviderSelection::named("missing").expect("selection should parse");

    let errors = [
        ready(registry.resolve_default_config_async(&config))
            .expect_err("default resolution should fail"),
        ready(registry.resolve_config_async(&config))
            .expect_err("configuration resolution should fail"),
        ready(registry.resolve_selected_config_async(&selection, &config))
            .expect_err("selected resolution should fail"),
        ready(registry.file_system_async(&config))
            .err()
            .expect("filesystem creation should fail"),
        ready(registry.resource_async(&config))
            .expect_err("resource resolution should fail"),
        ready(registry.file_system_uri_async(&uri))
            .err()
            .expect("URI filesystem creation should fail"),
        ready(registry.resource_uri_async(&uri))
            .expect_err("URI resource resolution should fail"),
    ];
    for error in errors {
        assert!(matches!(error, FileSystemRegistryError::Resolution(_)));
    }

    let invalid_selector_config = FileSystemConfig::new(
        FsUri::parse("missing-:///resource").expect("URI scheme should parse"),
    );
    let error = ready(registry.resolve_config_async(&invalid_selector_config))
        .expect_err("the URI scheme should not form a provider selector");
    assert!(matches!(error, FileSystemRegistryError::Selection(_)));
}

#[test]
fn test_async_registry_applies_each_creation_fallback_policy() {
    let registry = AsyncFileSystemRegistry::default();
    registry
        .register(ErrorAsyncProvider {
            descriptor: descriptor("broken"),
            kind: FsErrorKind::Other,
        })
        .unwrap();
    registry
        .register(ErrorAsyncProvider {
            descriptor: descriptor("unsupported"),
            kind: FsErrorKind::UnsupportedCapability,
        })
        .unwrap();
    registry
        .register(CapturingAsyncProvider {
            descriptor: descriptor("async-capture"),
            captured: Arc::new(Mutex::new(None)),
            path: "fallback",
        })
        .unwrap();
    let config = FileSystemConfig::new(
        FsUri::parse("async-capture:///resource").expect("URI should parse"),
    );

    let never = ProviderSelection::chain(["broken", "async-capture"])
        .unwrap()
        .with_fallback_policy(FallbackPolicy::Never);
    let error = ready(registry.resolve_selected_config_async(&never, &config))
        .expect_err("never policy should stop at the first error");
    let FileSystemRegistryError::Creation(creation) = error else {
        panic!("creation should preserve its typed aggregate");
    };
    assert_eq!("broken", creation.decisive_attempt().provider_id().as_str());

    let absence = ProviderSelection::chain(["broken", "async-capture"])
        .unwrap()
        .with_fallback_policy(FallbackPolicy::OnAbsence);
    assert!(matches!(
        ready(registry.resolve_selected_config_async(&absence, &config)).unwrap_err(),
        FileSystemRegistryError::Creation(_)
    ));

    let unsupported =
        ProviderSelection::chain(["unsupported", "async-capture"])
            .unwrap()
            .with_fallback_policy(FallbackPolicy::OnAbsence);
    assert_eq!(
        "fallback",
        ready(registry.resolve_selected_config_async(&unsupported, &config))
            .unwrap()
            .path()
            .as_str(),
    );

    let any = ProviderSelection::chain(["broken", "async-capture"])
        .unwrap()
        .with_fallback_policy(FallbackPolicy::OnAnyError);
    assert_eq!(
        "fallback",
        ready(registry.resolve_selected_config_async(&any, &config))
            .unwrap()
            .path()
            .as_str(),
    );
}

#[test]
fn test_async_registry_retains_ordered_failures_when_fallback_is_exhausted() {
    let registry = AsyncFileSystemRegistry::default();
    registry
        .register(UnavailableAsyncProvider {
            descriptor: descriptor("first-offline"),
        })
        .unwrap();
    registry
        .register(ErrorAsyncProvider {
            descriptor: descriptor("second-unsupported"),
            kind: FsErrorKind::UnsupportedOperation,
        })
        .unwrap();
    let selection =
        ProviderSelection::chain(["first-offline", "second-unsupported"])
            .unwrap()
            .with_fallback_policy(FallbackPolicy::OnAbsence);
    let config = FileSystemConfig::new(
        FsUri::parse("unrelated:///resource").expect("URI should parse"),
    );

    let error =
        ready(registry.resolve_selected_config_async(&selection, &config))
            .expect_err("every admitted provider should fail");

    let FileSystemRegistryError::Creation(creation) = error else {
        panic!("creation should preserve its typed aggregate");
    };
    assert_eq!(
        ["first-offline", "second-unsupported"],
        creation
            .attempts()
            .iter()
            .map(|attempt| attempt.provider_id().as_str())
            .collect::<Vec<_>>()
            .as_slice(),
    );
}

#[test]
fn test_async_registry_aggregates_failures_before_policy_stops_fallback() {
    let registry = AsyncFileSystemRegistry::default();
    registry
        .register(UnavailableAsyncProvider {
            descriptor: descriptor("first-offline"),
        })
        .unwrap();
    registry
        .register(ErrorAsyncProvider {
            descriptor: descriptor("second-broken"),
            kind: FsErrorKind::Other,
        })
        .unwrap();
    registry
        .register(CapturingAsyncProvider {
            descriptor: descriptor("unreached"),
            captured: Arc::new(Mutex::new(None)),
            path: "unreached",
        })
        .unwrap();
    let selection = ProviderSelection::chain([
        "first-offline",
        "second-broken",
        "unreached",
    ])
    .unwrap()
    .with_fallback_policy(FallbackPolicy::OnAbsence);
    let config = FileSystemConfig::new(
        FsUri::parse("unrelated:///resource").expect("URI should parse"),
    );

    let error =
        ready(registry.resolve_selected_config_async(&selection, &config))
            .expect_err("non-absence failure should stop fallback");

    let FileSystemRegistryError::Creation(creation) = error else {
        panic!("creation should preserve its typed aggregate");
    };
    let diagnostics = creation.to_string();
    assert!(diagnostics.contains("stopped by fallback policy"));
    assert!(diagnostics.contains("first-offline"));
    assert!(diagnostics.contains("second-broken"));
    assert!(!diagnostics.contains("unreached"));
}

#[test]
fn test_async_registry_automatic_priority_aliases_and_deduplication_are_stable()
{
    let registry = AsyncFileSystemRegistry::default();
    registry
        .register(CapturingAsyncProvider {
            descriptor: descriptor("low").with_priority(1),
            captured: Arc::new(Mutex::new(None)),
            path: "low",
        })
        .unwrap();
    let high_descriptor = descriptor("high")
        .with_aliases(["fast"])
        .expect("alias should parse")
        .with_priority(10);
    registry
        .register(CapturingAsyncProvider {
            descriptor: high_descriptor,
            captured: Arc::new(Mutex::new(None)),
            path: "high",
        })
        .unwrap();
    let config = FileSystemConfig::new(
        FsUri::parse("unrelated:///resource").expect("URI should parse"),
    );

    assert_eq!(
        "high",
        ready(registry.resolve_default_config_async(&config))
            .unwrap()
            .path()
            .as_str(),
    );
    let deduplicated = ProviderSelection::chain(["fast", "high"]).unwrap();
    assert_eq!(
        "high",
        ready(registry.resolve_selected_config_async(&deduplicated, &config))
            .unwrap()
            .path()
            .as_str(),
    );

    let conflicting = descriptor("other")
        .with_aliases(["fast"])
        .expect("alias should parse");
    assert!(matches!(
        registry
            .register(CapturingAsyncProvider {
                descriptor: conflicting,
                captured: Arc::new(Mutex::new(None)),
                path: "other",
            })
            .unwrap_err(),
        FileSystemRegistryError::Registration(_)
    ));
    assert_eq!(
        vec!["low", "high"],
        registry
            .provider_ids()
            .iter()
            .map(ProviderId::as_str)
            .collect::<Vec<_>>()
    );
}

struct PendingAsyncProvider {
    descriptor: ProviderDescriptor,
    entered: mpsc::Sender<()>,
    release: Mutex<Option<mpsc::Receiver<()>>>,
}

impl ProviderMetadata for PendingAsyncProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        self.descriptor.clone()
    }
}

impl AsyncServiceProvider<FileSystemSpec> for PendingAsyncProvider {
    fn create_configured<'a>(
        &'a self,
        config: &'a FileSystemConfig,
    ) -> ProviderFuture<
        'a,
        Result<FileSystemResolution<dyn AsyncFileSystem>, ProviderFailure<FsError>>,
    > {
        let entered = self.entered.clone();
        let release = self
            .release
            .lock()
            .expect("release lock should succeed")
            .take()
            .expect("pending provider should be called once");
        let uri = config.uri().clone();
        Box::pin(async move {
            entered
                .send(())
                .expect("entry receiver should remain alive");
            release.recv().expect("release sender should remain alive");
            let fs: Arc<dyn AsyncFileSystem> =
                Arc::new(AsyncOnlyFs::new("pending"));
            Ok(FileSystemResolution::new(
                fs,
                FsPath::parse_literal("released")
                    .expect("provider path should parse"),
                uri,
            ))
        })
    }
}

#[test]
fn test_async_provider_pending_does_not_hold_registry_lock() {
    let registry = AsyncFileSystemRegistry::default();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    registry
        .register(PendingAsyncProvider {
            descriptor: descriptor("pending"),
            entered: entered_tx,
            release: Mutex::new(Some(release_rx)),
        })
        .expect("pending provider should register");
    let creation_registry = registry.clone();
    let creation = std::thread::spawn(move || {
        let config = FileSystemConfig::new(
            FsUri::parse("pending:///resource").expect("URI should parse"),
        );
        ready(creation_registry.resolve_default_config_async(&config))
    });

    entered_rx.recv().expect("provider should announce entry");
    registry
        .register(CapturingAsyncProvider {
            descriptor: descriptor("later"),
            captured: Arc::new(Mutex::new(None)),
            path: "later",
        })
        .expect("registration should proceed while creation is pending");
    assert_eq!(
        vec!["pending", "later"],
        registry
            .provider_ids()
            .iter()
            .map(ProviderId::as_str)
            .collect::<Vec<_>>()
    );
    release_tx
        .send(())
        .expect("pending provider should remain alive");

    assert_eq!(
        "released",
        creation
            .join()
            .expect("creation thread should not panic")
            .expect("pending provider should succeed")
            .path()
            .as_str(),
    );
}

fn descriptor(id: &str) -> ProviderDescriptor {
    ProviderDescriptor::new(
        ProviderId::new(id).expect("provider id should parse"),
    )
}
