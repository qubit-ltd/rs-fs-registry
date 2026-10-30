// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::FsError;
use qubit_fs::error::FsErrorKind;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemRegistry;
use qubit_fs_registry::FileSystemRegistryError;
use qubit_fs_registry::FileSystemResolution;
use qubit_fs_registry::FileSystemSpec;
use qubit_spi::ProviderDescriptor;
use qubit_spi::ProviderId;
use qubit_spi::ProviderMetadata;
use qubit_spi::ServiceProvider;
use qubit_spi::ServiceSpec;
use qubit_spi::SyncServiceSpec;
use qubit_spi::error::ProviderFailure;
use qubit_spi::error::RegistryMutationError;

struct DuplicateProvider;

impl ProviderMetadata for DuplicateProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(ProviderId::new("duplicate").expect("static ID"))
    }
}

impl ServiceProvider<FileSystemSpec> for DuplicateProvider {
    fn create_configured(&self, _config: &FileSystemConfig) -> Result<FileSystemResolution, ProviderFailure<FsError>> {
        unreachable!("registration fails before creation")
    }
}

qubit_spi::submit_sync_provider! {
    inventory_entry = qubit_fs_registry::sync_file_system_providers::Entry;
    spec = FileSystemSpec;
    provider = DuplicateProvider;
}

qubit_spi::submit_sync_provider! {
    inventory_entry = qubit_fs_registry::sync_file_system_providers::Entry;
    spec = FileSystemSpec;
    provider = DuplicateProvider;
}

/// The filesystem error preserves the inventory source and registration
/// conflict.
#[test]
fn test_inventory_conflict_preserves_source_and_reason() {
    let error = FileSystemRegistry::from_inventory().expect_err("duplicate selector should fail");
    assert_eq!(error.reason_code(), "inventory_registration_conflict");
    assert!(std::error::Error::source(&error).is_some());
    let FileSystemRegistryError::InventoryBuild(build_error) = error else {
        panic!("expected inventory build error")
    };
    assert_eq!(build_error.source_location().crate_name(), "qubit-fs-registry");
    assert!(matches!(
        build_error.registration_error(),
        Some(RegistryMutationError::DuplicateSelector { .. })
    ));
    assert_eq!(
        Some("duplicate"),
        build_error.registration_error().and_then(|cause| cause.selector())
    );
    let converted: FsError = FileSystemRegistryError::InventoryBuild(build_error).into();
    assert_eq!(converted.kind(), FsErrorKind::Conflict);
    assert!(std::error::Error::source(&converted).is_some());
}

struct AmbiguousSpec;

impl ServiceSpec for AmbiguousSpec {
    type Config = ();
    type Error = std::convert::Infallible;
}

impl SyncServiceSpec for AmbiguousSpec {
    type Output = ();
}

struct AmbiguousProvider;

impl ProviderMetadata for AmbiguousProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(ProviderId::new("private-provider-secret").expect("valid static provider ID"))
    }
}

impl ServiceProvider<AmbiguousSpec> for AmbiguousProvider {
    fn create_configured(&self, _config: &()) -> Result<(), ProviderFailure<std::convert::Infallible>> {
        Ok(())
    }
}

qubit_spi::declare_sync_provider_inventory! {
    pub mod ambiguous_providers {
        spec = AmbiguousSpec;
    }
}

macro_rules! submit_same_source_providers {
    () => {
        qubit_spi::submit_sync_provider! {
            inventory_entry = ambiguous_providers::Entry;
            spec = AmbiguousSpec;
            provider = AmbiguousProvider;
        }
        qubit_spi::submit_sync_provider! {
            inventory_entry = ambiguous_providers::Entry;
            spec = AmbiguousSpec;
            provider = AmbiguousProvider;
        }
    };
}

submit_same_source_providers!();

/// Ambiguous source errors keep their source and format without selector access.
#[test]
fn test_inventory_ambiguity_preserves_source_and_safe_formatting() {
    let build_error = ambiguous_providers::build_registry().expect_err("duplicate source must fail");
    assert!(build_error.registration_error().is_none());
    assert!(build_error.source_location().line() > 0);

    let error = FileSystemRegistryError::InventoryBuild(build_error);
    assert_eq!(error.reason_code(), "inventory_source_ambiguity");
    assert!(std::error::Error::source(&error).is_some());
    let formatted = error.to_string();
    assert!(formatted.contains("inventory_source_ambiguity"));
    assert!(formatted.contains("<unknown>"));
    assert!(!formatted.contains("private-provider-secret"));
}
