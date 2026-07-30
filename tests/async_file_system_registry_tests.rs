// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use super::common;
use qubit_fs::ConnectionUri;
use qubit_fs::{
    FsError,
    FsErrorKind,
    FsOperation,
};
use qubit_fs_registry::{
    AsyncFileSystemRegistry,
    AsyncFileSystemResolution,
    CredentialRef,
    FileSystemConfig,
    FileSystemRegistryError,
    FileSystemSpec,
};
use qubit_spi::error::ProviderFailure;
use qubit_spi::{
    AsyncServiceProvider,
    ProviderDescriptor,
    ProviderFuture,
    ProviderId,
    ProviderMetadata,
    ProviderSelection,
};

/// Cloned asynchronous registries share providers and default selection state.
#[test]
fn test_async_registry_clone_shares_catalog_and_default_selection() {
    let registry = AsyncFileSystemRegistry::default();
    let clone = registry.clone();
    registry
        .register(AsyncFailingProvider)
        .expect("register shared provider");
    assert_eq!(clone.len(), 1);

    let selection =
        ProviderSelection::named("async-failing").expect("valid selection");
    clone.set_default_selection(selection.clone());
    assert_eq!(registry.default_selection(), selection);
}

/// Async resolution rejects conflicting embedded and referenced credentials
/// before provider invocation.
#[test]
fn test_async_registry_rejects_embedded_and_referenced_credentials() {
    let config = FileSystemConfig::new(
        ConnectionUri::parse("s3://user:password@bucket/key")
            .expect("URI should parse"),
    )
    .with_credential(CredentialRef::DefaultChain);

    let error = common::block_on(
        AsyncFileSystemRegistry::default().resolve_config(config),
    )
    .expect_err("credential sources conflict");
    assert!(matches!(
        error,
        FileSystemRegistryError::InvalidConfiguration { .. }
    ));
}
/// Resolution futures own their configuration rather than borrowing it.
#[test]
fn test_async_registry_accepts_owned_config_without_borrowing_the_registry() {
    let future = {
        let registry = AsyncFileSystemRegistry::default();
        registry.resolve_config(FileSystemConfig::new(
            ConnectionUri::parse("missing:///resource")
                .expect("URI should parse"),
        ))
    };
    drop(future);
}

/// Resolution futures remain usable after the originating registry is dropped.
#[test]
fn test_async_registry_future_is_static_and_polls_after_registry_is_dropped() {
    let future = {
        let registry = AsyncFileSystemRegistry::default();
        registry
            .register(AsyncFailingProvider)
            .expect("register provider");
        registry.resolve_config(FileSystemConfig::new(
            ConnectionUri::parse("async-failing:///resource")
                .expect("URI should parse"),
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
    registry
        .register(AsyncFailingProvider)
        .expect("register provider");
    assert!(!registry.is_empty());
    assert_eq!(registry.len(), 1);
    assert_eq!(registry.descriptors()[0].id().as_str(), "async-failing");

    let uri = ConnectionUri::parse("async-failing:///resource")
        .expect("URI should parse");
    let selection = ProviderSelection::named("async-failing")
        .expect("selection should parse");
    for result in [
        common::block_on(registry.resolve_uri(uri.clone())),
        common::block_on(registry.resolve_selected_config(
            selection.clone(),
            FileSystemConfig::new(uri.clone()),
        )),
    ] {
        assert!(matches!(result, Err(FileSystemRegistryError::Creation(_))));
    }
    registry.set_default_selection(selection);
    assert!(matches!(
        common::block_on(
            registry.resolve_default_config(FileSystemConfig::new(uri))
        ),
        Err(FileSystemRegistryError::Creation(_))
    ));
}

/// Async explicit selections conflict when configuration embeds another one.
#[test]
fn test_async_registry_selected_config_rejects_conflicting_selection() {
    let registry = AsyncFileSystemRegistry::default();
    let config = FileSystemConfig::new(
        ConnectionUri::parse("configured:///resource")
            .expect("URI should parse"),
    )
    .with_selection(
        ProviderSelection::named("configured").expect("selection should parse"),
    );
    let requested =
        ProviderSelection::named("requested").expect("selection should parse");
    assert!(matches!(
        common::block_on(registry.resolve_selected_config(requested, config)),
        Err(FileSystemRegistryError::SelectionConflict { .. })
    ));
}

/// An explicit configuration selection takes precedence over the URI scheme.
#[test]
fn test_resolve_config_prefers_explicit_selection_over_uri_scheme() {
    let registry = AsyncFileSystemRegistry::default();
    registry
        .register(AsyncFailingProvider)
        .expect("register provider");
    let config = FileSystemConfig::new(
        ConnectionUri::parse("unregistered-scheme:///resource")
            .expect("URI should parse"),
    )
    .with_selection(
        ProviderSelection::named("async-failing")
            .expect("selection should parse"),
    );

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
        ConnectionUri::parse("async-failing:///resource")
            .expect("URI should parse"),
    ));
    registry
        .register(AsyncFailingProvider)
        .expect("register provider after creating future");

    assert!(matches!(
        common::block_on(future),
        Err(FileSystemRegistryError::Resolution(_))
    ));
}

struct AsyncFailingProvider;
impl ProviderMetadata for AsyncFailingProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(
            ProviderId::new("async-failing").expect("provider id"),
        )
    }
}
impl AsyncServiceProvider<FileSystemSpec> for AsyncFailingProvider {
    fn create_configured<'a>(
        &'a self,
        _: &'a FileSystemConfig,
    ) -> ProviderFuture<
        'a,
        Result<AsyncFileSystemResolution, ProviderFailure<FsError>>,
    > {
        Box::pin(async {
            Err(ProviderFailure::unavailable(FsError::new(
                FsErrorKind::ProviderUnavailable,
                FsOperation::Provider,
                "unavailable",
            )))
        })
    }
}
