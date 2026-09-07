// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;

use futures::channel::oneshot;
use futures::task::noop_waker;
use qubit_fs::FsError;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::path::ConnectionUri;
use qubit_fs_registry::AsyncFileSystemRegistry;
use qubit_fs_registry::AsyncFileSystemResolution;
use qubit_fs_registry::CredentialRef;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemRegistryError;
use qubit_fs_registry::FileSystemSpec;
use qubit_spi::AsyncServiceProvider;
use qubit_spi::ProviderDescriptor;
use qubit_spi::ProviderFuture;
use qubit_spi::ProviderId;
use qubit_spi::ProviderMetadata;
use qubit_spi::ProviderSelection;
use qubit_spi::error::ProviderFailure;

use super::common;
use crate::support::provider_fixtures::ObservedProvider;

/// Cloned asynchronous registries share providers and default selection state.
#[test]
fn test_async_registry_clone_shares_catalog_and_default_selection() {
    let registry = AsyncFileSystemRegistry::default();
    let clone = registry.clone();
    registry
        .register(AsyncFailingProvider)
        .expect("register shared provider");
    assert_eq!(clone.len(), 1);

    let selection = ProviderSelection::named("async-failing").expect("valid selection");
    clone.set_default_selection(selection.clone());
    assert_eq!(registry.default_selection(), selection);
}

/// Async resolution rejects conflicting embedded and referenced credentials
/// before provider invocation.
#[test]
fn test_async_registry_rejects_embedded_and_referenced_credentials() {
    let config =
        FileSystemConfig::new(ConnectionUri::parse("s3://user:password@bucket/key").expect("URI should parse"))
            .with_credential(CredentialRef::DefaultChain);

    let error = common::block_on(AsyncFileSystemRegistry::default().resolve_config(config))
        .expect_err("credential sources conflict");
    assert!(matches!(error, FileSystemRegistryError::CredentialSourceConflict));
}

/// An asynchronous default selection conflict takes precedence over resolving
/// an unknown default provider.
#[test]
fn test_async_registry_default_selection_conflict_precedes_default_resolution() {
    let registry = AsyncFileSystemRegistry::default();
    registry.set_default_selection(ProviderSelection::named("missing-default").expect("selection should parse"));
    let config = FileSystemConfig::new(ConnectionUri::parse("configured:///resource").expect("URI should parse"))
        .with_selection(ProviderSelection::named("configured").expect("selection should parse"));

    let error = common::block_on(registry.resolve_default_config(config))
        .expect_err("the configured selection should conflict before resolution");
    assert!(matches!(error, FileSystemRegistryError::SelectionConflict { .. }));
}

/// Async credential validation takes precedence over resolving an unknown
/// default.
#[test]
fn test_async_registry_default_config_validates_credentials_before_resolution() {
    let registry = AsyncFileSystemRegistry::default();
    registry.set_default_selection(ProviderSelection::named("missing-default").expect("selection should parse"));
    let config = FileSystemConfig::new(
        ConnectionUri::parse("configured://user:password@bucket/resource").expect("URI should parse"),
    )
    .with_credential(CredentialRef::DefaultChain);

    let error = common::block_on(registry.resolve_default_config(config))
        .expect_err("credential conflict should precede default resolution");
    assert!(matches!(error, FileSystemRegistryError::CredentialSourceConflict));
}

/// Credential validation prevents asynchronous provider creation from observing
/// conflicting embedded and referenced credentials.
#[test]
fn test_async_registry_rejects_credential_conflict_before_provider_creation() {
    let create_calls = Arc::new(AtomicUsize::new(0));
    let registry = AsyncFileSystemRegistry::default();
    registry
        .register(CountingAsyncProvider::new(
            "async-credential-counter",
            Arc::clone(&create_calls),
        ))
        .expect("register provider");
    let config = FileSystemConfig::new(
        ConnectionUri::parse("async-credential-counter://user:password@bucket/resource").expect("URI should parse"),
    )
    .with_credential(CredentialRef::DefaultChain);

    let error = common::block_on(registry.resolve_config(config))
        .expect_err("credential conflict should fail before provider creation");
    assert!(matches!(error, FileSystemRegistryError::CredentialSourceConflict));
    assert_eq!(create_calls.load(Ordering::SeqCst), 0);
}
/// Resolution futures own their configuration rather than borrowing it.
#[test]
fn test_async_registry_accepts_owned_config_without_borrowing_the_registry() {
    let future = {
        let registry = AsyncFileSystemRegistry::default();
        registry.resolve_config(FileSystemConfig::new(
            ConnectionUri::parse("missing:///resource").expect("URI should parse"),
        ))
    };
    drop(future);
}

/// Resolution futures remain usable after the originating registry is dropped.
#[test]
fn test_async_registry_future_is_static_and_polls_after_registry_is_dropped() {
    let future = {
        let registry = AsyncFileSystemRegistry::default();
        registry.register(AsyncFailingProvider).expect("register provider");
        registry.resolve_config(FileSystemConfig::new(
            ConnectionUri::parse("async-failing:///resource").expect("URI should parse"),
        ))
    };
    let error = common::block_on(future).expect_err("provider should fail");
    assert!(matches!(error, FileSystemRegistryError::Creation(_)));
}

/// Async registry inspection and every owned resolution entry point preserve
/// the provider snapshot until the returned future completes.
#[test]
fn test_async_registry_inspection_and_resolution_entry_points() {
    let registry = AsyncFileSystemRegistry::default();
    assert!(registry.is_empty());
    registry.register(AsyncFailingProvider).expect("register provider");
    assert!(!registry.is_empty());
    assert_eq!(registry.len(), 1);
    assert_eq!(registry.descriptors()[0].id().as_str(), "async-failing");
    assert_eq!(registry.provider_ids()[0].as_str(), "async-failing");

    let uri = ConnectionUri::parse("async-failing:///resource").expect("URI should parse");
    let selection = ProviderSelection::named("async-failing").expect("selection should parse");
    for result in [
        common::block_on(registry.resolve_uri(uri.clone())),
        common::block_on(registry.resolve_selected_config(selection.clone(), FileSystemConfig::new(uri.clone()))),
    ] {
        assert!(matches!(result, Err(FileSystemRegistryError::Creation(_))));
    }
    registry.set_default_selection(selection);
    assert!(matches!(
        common::block_on(registry.resolve_default_config(FileSystemConfig::new(uri))),
        Err(FileSystemRegistryError::Creation(_))
    ));
}

/// Async explicit selections conflict when configuration embeds another one.
#[test]
fn test_async_registry_selected_config_rejects_conflicting_selection() {
    let registry = AsyncFileSystemRegistry::default();
    let config = FileSystemConfig::new(ConnectionUri::parse("configured:///resource").expect("URI should parse"))
        .with_selection(ProviderSelection::named("configured").expect("selection should parse"));
    let requested = ProviderSelection::named("requested").expect("selection should parse");
    assert!(matches!(
        common::block_on(registry.resolve_selected_config(requested, config)),
        Err(FileSystemRegistryError::SelectionConflict { .. })
    ));
}

/// An explicit configuration selection takes precedence over the URI scheme.
#[test]
fn test_resolve_config_prefers_explicit_selection_over_uri_scheme() {
    let registry = AsyncFileSystemRegistry::default();
    registry.register(AsyncFailingProvider).expect("register provider");
    let config =
        FileSystemConfig::new(ConnectionUri::parse("unregistered-scheme:///resource").expect("URI should parse"))
            .with_selection(ProviderSelection::named("async-failing").expect("selection should parse"));

    assert!(matches!(
        common::block_on(registry.resolve_config(config)),
        Err(FileSystemRegistryError::Creation(_))
    ));
}

/// Async resolution snapshots a missing provider before later registrations.
#[test]
fn test_resolve_config_snapshots_missing_provider_before_registration() {
    let registry = AsyncFileSystemRegistry::default();
    let future = registry.resolve_config(FileSystemConfig::new(
        ConnectionUri::parse("async-failing:///resource").expect("URI should parse"),
    ));
    registry
        .register(AsyncFailingProvider)
        .expect("register provider after creating future");

    assert!(matches!(
        common::block_on(future),
        Err(FileSystemRegistryError::Resolution(_))
    ));
}

/// Async default resolution snapshots a missing provider before later
/// registration.
#[test]
fn test_resolve_default_config_snapshots_missing_provider_before_registration() {
    let registry = AsyncFileSystemRegistry::default();
    registry.set_default_selection(ProviderSelection::named("async-late").expect("selection should parse"));
    let future = registry.resolve_default_config(FileSystemConfig::new(
        ConnectionUri::parse("async-late:///resource").expect("URI should parse"),
    ));
    registry
        .register(NamedAsyncFailingProvider::new("async-late"))
        .expect("register provider after creating future");

    assert!(matches!(
        common::block_on(future),
        Err(FileSystemRegistryError::Resolution(_))
    ));
}

/// An asynchronous default future retains its provider snapshot after the
/// registry's default selection changes.
#[test]
fn test_resolve_default_config_snapshots_provider_before_default_changes() {
    let registry = AsyncFileSystemRegistry::default();
    registry.register(AsyncFailingProvider).expect("register provider");
    registry.set_default_selection(ProviderSelection::named("async-failing").expect("selection should parse"));
    let future = registry.resolve_default_config(FileSystemConfig::new(
        ConnectionUri::parse("async-failing:///resource").expect("URI should parse"),
    ));
    registry.set_default_selection(ProviderSelection::named("missing-default").expect("selection should parse"));

    assert!(matches!(
        common::block_on(future),
        Err(FileSystemRegistryError::Creation(_))
    ));
}

struct AsyncFailingProvider;

struct CountingAsyncProvider {
    id: &'static str,
    create_calls: Arc<AtomicUsize>,
}

impl CountingAsyncProvider {
    fn new(id: &'static str, create_calls: Arc<AtomicUsize>) -> Self {
        Self { id, create_calls }
    }
}

impl ProviderMetadata for CountingAsyncProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(ProviderId::new(self.id).expect("provider id"))
    }
}

impl AsyncServiceProvider<FileSystemSpec> for CountingAsyncProvider {
    fn create_configured<'a>(
        &'a self,
        _: &'a FileSystemConfig,
    ) -> ProviderFuture<'a, Result<AsyncFileSystemResolution, ProviderFailure<FsError>>> {
        self.create_calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {
            Err(ProviderFailure::unavailable(FsError::new(
                FsErrorKind::ProviderUnavailable,
                FsOperation::Provider,
                "unavailable",
            )))
        })
    }
}

struct NamedAsyncFailingProvider {
    id: &'static str,
}

impl NamedAsyncFailingProvider {
    fn new(id: &'static str) -> Self {
        Self { id }
    }
}

impl ProviderMetadata for NamedAsyncFailingProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(ProviderId::new(self.id).expect("provider id"))
    }
}

impl AsyncServiceProvider<FileSystemSpec> for NamedAsyncFailingProvider {
    fn create_configured<'a>(
        &'a self,
        _: &'a FileSystemConfig,
    ) -> ProviderFuture<'a, Result<AsyncFileSystemResolution, ProviderFailure<FsError>>> {
        Box::pin(async {
            Err(ProviderFailure::unavailable(FsError::new(
                FsErrorKind::ProviderUnavailable,
                FsOperation::Provider,
                "unavailable",
            )))
        })
    }
}

impl ProviderMetadata for AsyncFailingProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(ProviderId::new("async-failing").expect("provider id"))
    }
}
impl AsyncServiceProvider<FileSystemSpec> for AsyncFailingProvider {
    fn create_configured<'a>(
        &'a self,
        _: &'a FileSystemConfig,
    ) -> ProviderFuture<'a, Result<AsyncFileSystemResolution, ProviderFailure<FsError>>> {
        Box::pin(async {
            Err(ProviderFailure::unavailable(FsError::new(
                FsErrorKind::ProviderUnavailable,
                FsOperation::Provider,
                "unavailable",
            )))
        })
    }
}

/// Pending creation keeps owned inputs and the original snapshot without
/// blocking catalog writes.
#[test]
fn test_pending_creation_outlives_registry_and_catalog_changes() {
    fn require_send_static<F: Future + Send + 'static>(future: F) -> F {
        future
    }
    let registry = AsyncFileSystemRegistry::default();
    let provider = ObservedProvider::new("original");
    let calls = Arc::clone(&provider.calls);
    let (release_tx, release_rx) = oneshot::channel();
    *provider.release.lock().expect("release control") = Some(release_rx);
    registry.register(provider).expect("register");
    registry.set_default_selection(ProviderSelection::named("original").expect("selection"));
    let config = FileSystemConfig::new(ConnectionUri::parse("file:///resource").expect("URI"));
    let mut future = Box::pin(require_send_static(registry.resolve_default_config(config.clone())));
    assert!(calls.lock().expect("calls").is_empty());
    let waker = noop_waker();
    assert!(matches!(
        future.as_mut().poll(&mut Context::from_waker(&waker)),
        Poll::Pending
    ));
    assert_eq!(*calls.lock().expect("calls"), vec![config.clone()]);
    registry
        .register(ObservedProvider::new("new"))
        .expect("register while pending");
    registry.set_default_selection(ProviderSelection::named("new").expect("new selection"));
    let new = common::block_on(registry.resolve_default_config(config)).expect("new default");
    assert_eq!(new.file_system().properties().info().provider_id(), "new");
    drop(registry);
    release_tx.send(()).expect("release");
    let old = common::block_on(future).expect("original snapshot completes");
    assert_eq!(old.file_system().properties().info().provider_id(), "original");
}

/// Dropping any unpolled owned-config future never creates a provider.
#[test]
fn test_unpolled_entry_points_do_not_create_provider() {
    let registry = AsyncFileSystemRegistry::default();
    let provider = ObservedProvider::new("lazy");
    let calls = Arc::clone(&provider.calls);
    registry.register(provider).expect("register");
    let selection = ProviderSelection::named("lazy").expect("selection");
    registry.set_default_selection(selection.clone());
    let uri = ConnectionUri::parse("lazy:///resource").expect("URI");
    drop(registry.resolve_config(FileSystemConfig::new(uri.clone())));
    drop(registry.resolve_selected_config(selection, FileSystemConfig::new(uri.clone())));
    drop(registry.resolve_default_config(FileSystemConfig::new(uri.clone())));
    drop(registry.resolve_uri(uri));
    assert!(calls.lock().expect("calls").is_empty());
}

/// All async config entry points enforce the same pre-creation credential
/// boundary.
#[test]
fn test_all_async_entry_points_validate_credentials_before_creation() {
    let provider = ObservedProvider::new("s3");
    let calls = Arc::clone(&provider.calls);
    let registry = AsyncFileSystemRegistry::default();
    registry.register(provider).expect("provider");
    let selection = ProviderSelection::named("s3").expect("selection");
    registry.set_default_selection(selection.clone());
    for uri in ["s3://user:password@bucket/key", "s3://bucket/key?token=secret"] {
        let config =
            FileSystemConfig::new(ConnectionUri::parse(uri).expect("URI")).with_credential(CredentialRef::DefaultChain);
        for result in [
            common::block_on(registry.resolve_config(config.clone())),
            common::block_on(registry.resolve_selected_config(selection.clone(), config.clone())),
            common::block_on(registry.resolve_default_config(config)),
        ] {
            let error = result.expect_err("conflict");
            assert!(matches!(error, FileSystemRegistryError::CredentialSourceConflict));
            assert_eq!(error.reason_code(), "credential_source_conflict");
        }
    }
    assert!(calls.lock().expect("calls").is_empty());
    let username = FileSystemConfig::new(ConnectionUri::parse("s3://user@bucket/key").expect("URI"))
        .with_credential(CredentialRef::DefaultChain);
    let resolution = common::block_on(registry.resolve_config(username.clone())).expect("username is allowed");
    assert_eq!(resolution.file_system().properties().info().provider_id(), "s3");
    assert_eq!(*calls.lock().expect("calls"), vec![username]);
}

/// An asynchronous provider cannot change the identity captured at
/// registration.
#[test]
fn test_async_registration_binds_original_descriptor() {
    let provider = ObservedProvider::new("original");
    let descriptor = Arc::clone(&provider.descriptor);
    let registry = AsyncFileSystemRegistry::default();
    registry.register(provider).expect("register");
    *descriptor.lock().expect("descriptor") = ProviderDescriptor::new(ProviderId::new("changed").expect("ID"));
    let resolution = common::block_on(registry.resolve_uri(ConnectionUri::parse("original:///resource").expect("URI")))
        .expect("original identity");
    assert_eq!(resolution.file_system().properties().info().provider_id(), "original");
    assert_eq!(registry.provider_ids()[0].as_str(), "original");
}
