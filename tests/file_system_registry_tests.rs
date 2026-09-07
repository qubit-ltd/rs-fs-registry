// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use qubit_fs::FsError;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::path::ConnectionUri;
use qubit_fs_registry::CredentialRef;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemProvider;
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

use crate::support::provider_fixtures::ObservedProvider;

/// Shared synchronous providers can be registered through their public
/// trait-object contract.
#[test]
fn test_registry_register_shared_accepts_arc_trait_object() {
    let provider = ObservedProvider::new("shared-trait-object");
    let calls = Arc::clone(&provider.calls);
    let provider: Arc<FileSystemProvider> = Arc::new(provider);
    let registry = FileSystemRegistry::default();
    registry
        .register_shared(provider)
        .expect("register shared trait object");

    let config = FileSystemConfig::new(
        ConnectionUri::parse("shared-trait-object:///resource").expect("URI should parse"),
    );
    let resolution = registry
        .resolve_config(&config)
        .expect("resolve shared provider");

    assert_eq!(
        resolution.file_system().properties().info().provider_id(),
        "shared-trait-object"
    );
    assert_eq!(*calls.lock().expect("calls"), vec![config]);
}

/// Cloned synchronous registries share providers and default selection state.
#[test]
fn test_registry_clone_shares_catalog_and_default_selection() {
    let registry = FileSystemRegistry::default();
    let clone = registry.clone();
    registry
        .register(FailingProvider::new("shared"))
        .expect("register shared provider");
    assert_eq!(clone.len(), 1);

    let selection = ProviderSelection::named("shared").expect("valid selection");
    clone.set_default_selection(selection.clone());
    assert_eq!(registry.default_selection(), selection);
}

/// Embedded URI secrets conflict with an external credential reference.
#[test]
fn test_registry_rejects_embedded_and_referenced_credentials_before_resolution() {
    let config = FileSystemConfig::new(
        ConnectionUri::parse("s3://user:password@bucket/key").expect("URI should parse"),
    )
    .with_credential(CredentialRef::Profile {
        name: "integration".to_owned(),
    });
    let error = FileSystemRegistry::default()
        .resolve_config(&config)
        .expect_err("credential sources conflict");
    assert!(matches!(
        error,
        FileSystemRegistryError::CredentialSourceConflict
    ));
}

/// A username without secret material may coexist with a credential reference.
#[test]
fn test_registry_allows_username_only_connection_uri_with_credential_reference() {
    let config = FileSystemConfig::new(
        ConnectionUri::parse("s3://user@bucket/key").expect("URI should parse"),
    )
    .with_credential(CredentialRef::DefaultChain);
    let error = FileSystemRegistry::default()
        .resolve_config(&config)
        .expect_err("empty registry should fail after credential validation");
    assert!(matches!(error, FileSystemRegistryError::Resolution(_)));
}

/// Provider creation failures preserve provider registration order.
#[test]
fn test_registry_aggregates_provider_failures_in_registration_order() {
    let registry = FileSystemRegistry::default();
    registry
        .register(FailingProvider::new("first"))
        .expect("register first");
    registry
        .register(FailingProvider::new("second"))
        .expect("register second");
    let config =
        FileSystemConfig::new(ConnectionUri::parse("first:///resource").expect("URI should parse"))
            .with_selection(
                ProviderSelection::chain(["first", "second"])
                    .expect("selection should parse")
                    .with_fallback_policy(FallbackPolicy::OnAnyError),
            );
    let error = registry
        .resolve_config(&config)
        .expect_err("providers fail");
    let FileSystemRegistryError::Creation(creation) = error else {
        panic!("expected aggregate creation error")
    };
    let ids: Vec<_> = creation
        .attempts()
        .iter()
        .map(|attempt| attempt.provider_id().as_str())
        .collect();
    assert_eq!(ids, ["first", "second"]);
}

/// Registry inspection and each public resolution entry point preserve the
/// registered provider snapshot and structured errors.
#[test]
fn test_registry_inspection_and_resolution_entry_points() {
    let registry = FileSystemRegistry::default();
    assert!(registry.is_empty());
    registry
        .register(FailingProvider::new("entry-points"))
        .expect("register provider");
    assert!(!registry.is_empty());
    assert_eq!(registry.len(), 1);
    assert_eq!(registry.descriptors()[0].id().as_str(), "entry-points");
    assert_eq!(registry.provider_ids()[0].as_str(), "entry-points");

    let uri = ConnectionUri::parse("entry-points:///resource").expect("URI should parse");
    let selection = ProviderSelection::named("entry-points").expect("selection should parse");
    for result in [
        registry.resolve_uri(&uri),
        registry.resolve_selected_config(&selection, &FileSystemConfig::new(uri.clone())),
    ] {
        assert!(matches!(result, Err(FileSystemRegistryError::Creation(_))));
    }
    registry.set_default_selection(selection);
    assert!(matches!(
        registry.resolve_default_config(&FileSystemConfig::new(uri)),
        Err(FileSystemRegistryError::Creation(_))
    ));
}

/// Explicit selections conflict when configuration embeds a different one.
#[test]
fn test_registry_selected_config_rejects_conflicting_selection() {
    let registry = FileSystemRegistry::default();
    let config = FileSystemConfig::new(
        ConnectionUri::parse("configured:///resource").expect("URI should parse"),
    )
    .with_selection(ProviderSelection::named("configured").expect("selection should parse"));
    let requested = ProviderSelection::named("requested").expect("selection should parse");
    assert!(matches!(
        registry.resolve_selected_config(&requested, &config),
        Err(FileSystemRegistryError::SelectionConflict { .. })
    ));
}

/// Matching embedded and requested selections remain valid, while secret-like
/// query credentials conflict with an external credential reference.
#[test]
fn test_registry_validates_matching_selection_and_query_credentials() {
    let registry = FileSystemRegistry::default();
    registry
        .register(FailingProvider::new("matching"))
        .expect("register provider");
    let selection = ProviderSelection::named("matching").expect("selection should parse");
    let matching = FileSystemConfig::new(
        ConnectionUri::parse("matching:///resource").expect("URI should parse"),
    )
    .with_selection(selection.clone());
    assert!(matches!(
        registry.resolve_selected_config(&selection, &matching),
        Err(FileSystemRegistryError::Creation(_))
    ));

    let query_credential = FileSystemConfig::new(
        ConnectionUri::parse("s3://bucket/key?token=secret").expect("URI should parse"),
    )
    .with_credential(CredentialRef::DefaultChain);
    assert!(matches!(
        FileSystemRegistry::default().resolve_config(&query_credential),
        Err(FileSystemRegistryError::CredentialSourceConflict)
    ));
}

/// A default selection conflict takes precedence over resolving an unknown
/// default provider.
#[test]
fn test_registry_default_selection_conflict_precedes_default_resolution() {
    let registry = FileSystemRegistry::default();
    registry.set_default_selection(
        ProviderSelection::named("missing-default").expect("selection should parse"),
    );
    let config = FileSystemConfig::new(
        ConnectionUri::parse("configured:///resource").expect("URI should parse"),
    )
    .with_selection(ProviderSelection::named("configured").expect("selection should parse"));

    let error = registry
        .resolve_default_config(&config)
        .expect_err("the configured selection should conflict before resolution");
    assert!(matches!(
        error,
        FileSystemRegistryError::SelectionConflict { .. }
    ));
}

/// Credential validation takes precedence over resolving an unknown default.
#[test]
fn test_registry_default_config_validates_credentials_before_resolution() {
    let registry = FileSystemRegistry::default();
    registry.set_default_selection(
        ProviderSelection::named("missing-default").expect("selection should parse"),
    );
    let config = FileSystemConfig::new(
        ConnectionUri::parse("configured://user:password@bucket/resource")
            .expect("URI should parse"),
    )
    .with_credential(CredentialRef::DefaultChain);

    let error = registry
        .resolve_default_config(&config)
        .expect_err("credential conflict should precede default resolution");
    assert!(matches!(
        error,
        FileSystemRegistryError::CredentialSourceConflict
    ));
}

/// Credential validation prevents provider creation from observing conflicting
/// embedded and referenced credentials.
#[test]
fn test_registry_rejects_credential_conflict_before_provider_creation() {
    let create_calls = Arc::new(AtomicUsize::new(0));
    let registry = FileSystemRegistry::default();
    registry
        .register(CountingProvider::new(
            "credential-counter",
            Arc::clone(&create_calls),
        ))
        .expect("register provider");
    let config = FileSystemConfig::new(
        ConnectionUri::parse("credential-counter://user:password@bucket/resource")
            .expect("URI should parse"),
    )
    .with_credential(CredentialRef::DefaultChain);

    let error = registry
        .resolve_config(&config)
        .expect_err("credential conflict should fail before provider creation");
    assert!(matches!(
        error,
        FileSystemRegistryError::CredentialSourceConflict
    ));
    assert_eq!(create_calls.load(Ordering::SeqCst), 0);
}

/// An explicit configuration selection takes precedence over the URI scheme.
#[test]
fn test_resolve_config_prefers_explicit_selection_over_uri_scheme() {
    let registry = FileSystemRegistry::default();
    registry
        .register(FailingProvider::new("selected-provider"))
        .expect("register provider");
    let config = FileSystemConfig::new(
        ConnectionUri::parse("unregistered-scheme:///resource").expect("URI should parse"),
    )
    .with_selection(ProviderSelection::named("selected-provider").expect("selection should parse"));

    assert!(matches!(
        registry.resolve_config(&config),
        Err(FileSystemRegistryError::Creation(_))
    ));
}

pub(crate) struct FailingProvider {
    id: &'static str,
}

struct CountingProvider {
    id: &'static str,
    create_calls: Arc<AtomicUsize>,
}

impl CountingProvider {
    fn new(id: &'static str, create_calls: Arc<AtomicUsize>) -> Self {
        Self { id, create_calls }
    }
}

impl ProviderMetadata for CountingProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(ProviderId::new(self.id).expect("provider id"))
    }
}

impl ServiceProvider<FileSystemSpec> for CountingProvider {
    fn create_configured(
        &self,
        _: &FileSystemConfig,
    ) -> Result<FileSystemResolution, ProviderFailure<FsError>> {
        self.create_calls.fetch_add(1, Ordering::SeqCst);
        Err(ProviderFailure::unavailable(FsError::new(
            FsErrorKind::ProviderUnavailable,
            FsOperation::Provider,
            "unavailable",
        )))
    }
}

impl FailingProvider {
    /// Creates a provider fixture with the requested descriptor identity.
    ///
    /// # Parameters
    ///
    /// - `id`: Valid provider identifier used by the descriptor.
    ///
    /// # Returns
    ///
    /// A provider fixture that always reports an unavailable error.
    pub(crate) fn new(id: &'static str) -> Self {
        Self { id }
    }
}
impl ProviderMetadata for FailingProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(ProviderId::new(self.id).expect("provider id"))
    }
}
impl ServiceProvider<FileSystemSpec> for FailingProvider {
    fn create_configured(
        &self,
        _: &FileSystemConfig,
    ) -> Result<FileSystemResolution, ProviderFailure<FsError>> {
        Err(ProviderFailure::unavailable(FsError::new(
            FsErrorKind::ProviderUnavailable,
            FsOperation::Provider,
            "unavailable",
        )))
    }
}

/// A resolution already inside its first provider keeps its chain after
/// concurrent catalog changes.
#[test]
fn test_default_snapshot_survives_changes_during_creation() {
    let registry = FileSystemRegistry::default();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release_rx = Arc::new(std::sync::Mutex::new(release_rx));
    let mut first = ObservedProvider::new("first");
    first.fail = true;
    first.on_create = Some(Arc::new(move || {
        entered_tx.send(()).expect("entry signal");
        release_rx
            .lock()
            .expect("release receiver")
            .recv_timeout(Duration::from_secs(10))
            .expect("release signal");
    }));
    registry.register(first).expect("first");
    registry
        .register(ObservedProvider::new("old"))
        .expect("old");
    registry.set_default_selection(ProviderSelection::chain(["first", "old"]).expect("chain"));
    let worker_registry = registry.clone();
    let config = FileSystemConfig::new(ConnectionUri::parse("file:///resource").expect("URI"));
    let worker_config = config.clone();
    let worker = thread::spawn(move || worker_registry.resolve_default_config(&worker_config));
    entered_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("provider entered");
    registry
        .register(ObservedProvider::new("new"))
        .expect("register during creation");
    registry.set_default_selection(ProviderSelection::named("new").expect("new default"));
    release_tx.send(()).expect("release");
    let old = worker.join().expect("worker").expect("old resolution");
    assert_eq!(old.file_system().properties().info().provider_id(), "old");
    let new = registry
        .resolve_default_config(&config)
        .expect("new resolution");
    assert_eq!(new.file_system().properties().info().provider_id(), "new");
}

/// Username-only URIs actually reach a provider with the complete referenced
/// configuration.
#[test]
fn test_username_and_reference_reach_provider_unchanged() {
    let provider = ObservedProvider::new("s3");
    let calls = Arc::clone(&provider.calls);
    let registry = FileSystemRegistry::default();
    registry.register(provider).expect("provider");
    let config = FileSystemConfig::new(ConnectionUri::parse("s3://user@bucket/key").expect("URI"))
        .with_credential(CredentialRef::DefaultChain);
    let resolution = registry
        .resolve_config(&config)
        .expect("username does not conflict");
    assert_eq!(
        resolution.file_system().properties().info().provider_id(),
        "s3"
    );
    assert_eq!(*calls.lock().expect("calls"), vec![config]);
}

/// Each entry point rejects credential conflicts without invoking a provider.
#[test]
fn test_all_entry_points_validate_credentials_before_creation() {
    let provider = ObservedProvider::new("s3");
    let calls = Arc::clone(&provider.calls);
    let registry = FileSystemRegistry::default();
    registry.register(provider).expect("provider");
    let selection = ProviderSelection::named("s3").expect("selection");
    registry.set_default_selection(selection.clone());
    for uri in [
        "s3://user:password@bucket/key",
        "s3://bucket/key?token=secret",
    ] {
        let config = FileSystemConfig::new(ConnectionUri::parse(uri).expect("URI"))
            .with_credential(CredentialRef::DefaultChain);
        for result in [
            registry.resolve_config(&config),
            registry.resolve_selected_config(&selection, &config),
            registry.resolve_default_config(&config),
        ] {
            let error = result.expect_err("conflict");
            assert!(matches!(
                error,
                FileSystemRegistryError::CredentialSourceConflict
            ));
            assert_eq!(error.reason_code(), "credential_source_conflict");
        }
    }
    assert!(calls.lock().expect("calls").is_empty());
}

/// Registration snapshots metadata even if the provider later reports another
/// descriptor.
#[test]
fn test_registration_binds_original_descriptor() {
    let provider = ObservedProvider::new("original");
    let descriptor = Arc::clone(&provider.descriptor);
    let registry = FileSystemRegistry::default();
    registry.register(provider).expect("register");
    *descriptor.lock().expect("descriptor") =
        ProviderDescriptor::new(ProviderId::new("changed").expect("ID"));
    let resolution = registry
        .resolve_uri(&ConnectionUri::parse("original:///resource").expect("URI"))
        .expect("original identity");
    assert_eq!(
        resolution.file_system().properties().info().provider_id(),
        "original"
    );
    assert_eq!(registry.provider_ids()[0].as_str(), "original");
}
