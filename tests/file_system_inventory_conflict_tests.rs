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
use qubit_spi::error::ProviderFailure;

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
    assert_eq!(build_error.registration_error().selector(), Some("duplicate"));
    let converted: FsError = FileSystemRegistryError::InventoryBuild(build_error).into();
    assert_eq!(converted.kind(), FsErrorKind::Conflict);
    assert!(std::error::Error::source(&converted).is_some());
}
