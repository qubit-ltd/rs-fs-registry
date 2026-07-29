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

struct FailingProvider {
    id: &'static str,
}
impl FailingProvider {
    fn new(id: &'static str) -> Self {
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
