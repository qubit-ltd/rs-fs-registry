// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::ConnectionUri;
use qubit_fs::{
    FsError,
    FsErrorKind,
    FsOperation,
};
use qubit_fs_registry::{
    CredentialRef,
    FileSystemConfig,
    FileSystemRegistry,
    FileSystemRegistryError,
    FileSystemResolution,
    FileSystemSpec,
};
use qubit_spi::error::ProviderFailure;
use qubit_spi::{
    FallbackPolicy,
    ProviderDescriptor,
    ProviderId,
    ProviderMetadata,
    ProviderSelection,
    ServiceProvider,
};

use super::common;

#[test]
fn test_registry_clone_shares_catalog_and_default_selection() {
    let registry = FileSystemRegistry::default();
    let clone = registry.clone();
    registry
        .register(FailingProvider::new("shared"))
        .expect("register shared provider");
    assert_eq!(clone.len(), 1);

    let selection =
        ProviderSelection::named("shared").expect("valid selection");
    clone.set_default_selection(selection.clone());
    assert_eq!(registry.default_selection(), selection);
}

#[test]
fn test_registry_rejects_resolution_with_mismatched_provider_identity() {
    let registry = FileSystemRegistry::default();
    registry
        .register(MismatchedProvider)
        .expect("register mismatched provider");
    let config = FileSystemConfig::new(
        ConnectionUri::parse("registered-sync:///resource").expect("valid URI"),
    );

    let error = registry
        .resolve_config(&config)
        .expect_err("mismatched provider identity must fail");
    let FileSystemRegistryError::Creation(creation) = error else {
        panic!("expected provider creation error")
    };
    assert_eq!(
        creation.decisive_attempt().failure().error().kind(),
        FsErrorKind::ProviderContractViolation
    );
}
#[test]
fn test_registry_rejects_embedded_and_referenced_credentials_before_resolution()
{
    let config = FileSystemConfig::new(
        ConnectionUri::parse("s3://user:password@bucket/key")
            .expect("URI should parse"),
    )
    .with_credential(CredentialRef::Profile {
        name: "integration".to_owned(),
    });
    let error = FileSystemRegistry::default()
        .resolve_config(&config)
        .expect_err("credential sources conflict");
    assert!(matches!(
        error,
        FileSystemRegistryError::InvalidConfiguration { .. }
    ));
}

#[test]
fn test_registry_allows_username_only_connection_uri_with_credential_reference()
{
    let config = FileSystemConfig::new(
        ConnectionUri::parse("s3://user@bucket/key").expect("URI should parse"),
    )
    .with_credential(CredentialRef::DefaultChain);
    let error = FileSystemRegistry::default()
        .resolve_config(&config)
        .expect_err("empty registry should fail after credential validation");
    assert!(!matches!(
        error,
        FileSystemRegistryError::InvalidConfiguration { .. }
    ));
}

#[test]
fn test_registry_aggregates_provider_failures_in_registration_order() {
    let registry = FileSystemRegistry::default();
    registry
        .register(FailingProvider::new("first"))
        .expect("register first");
    registry
        .register(FailingProvider::new("second"))
        .expect("register second");
    let config = FileSystemConfig::new(
        ConnectionUri::parse("first:///resource").expect("URI should parse"),
    )
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

    let uri = ConnectionUri::parse("entry-points:///resource")
        .expect("URI should parse");
    let selection = ProviderSelection::named("entry-points")
        .expect("selection should parse");
    for result in [
        registry.resolve_uri(&uri),
        registry.resolve_selected_config(
            &selection,
            &FileSystemConfig::new(uri.clone()),
        ),
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
        ConnectionUri::parse("configured:///resource")
            .expect("URI should parse"),
    )
    .with_selection(
        ProviderSelection::named("configured").expect("selection should parse"),
    );
    let requested =
        ProviderSelection::named("requested").expect("selection should parse");
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
    let selection =
        ProviderSelection::named("matching").expect("selection should parse");
    let matching = FileSystemConfig::new(
        ConnectionUri::parse("matching:///resource").expect("URI should parse"),
    )
    .with_selection(selection.clone());
    assert!(matches!(
        registry.resolve_selected_config(&selection, &matching),
        Err(FileSystemRegistryError::Creation(_))
    ));

    let query_credential = FileSystemConfig::new(
        ConnectionUri::parse("s3://bucket/key?token=secret")
            .expect("URI should parse"),
    )
    .with_credential(CredentialRef::DefaultChain);
    assert!(matches!(
        FileSystemRegistry::default().resolve_config(&query_credential),
        Err(FileSystemRegistryError::InvalidConfiguration { .. })
    ));
}

pub(crate) struct FailingProvider {
    id: &'static str,
}
impl FailingProvider {
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

struct MismatchedProvider;

impl ProviderMetadata for MismatchedProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(
            ProviderId::new("registered-sync").expect("provider id"),
        )
    }
}

impl ServiceProvider<FileSystemSpec> for MismatchedProvider {
    fn create_configured(
        &self,
        _: &FileSystemConfig,
    ) -> Result<FileSystemResolution, ProviderFailure<FsError>> {
        Ok(common::sync_resolution("reported-sync"))
    }
}
