// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::error::FsErrorKind;
use qubit_fs::path::ConnectionUri;
#[cfg(feature = "async")]
use qubit_fs_registry::AsyncFileSystemRegistry;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemRegistry;
use qubit_fs_registry::FileSystemRegistryError;
use qubit_fs_registry::FileSystemSpec;

#[cfg(feature = "async")]
use crate::support::common;
use crate::support::provider_fixtures::ObservedProvider;

qubit_spi::submit_sync_provider! {
    inventory_entry = qubit_fs_registry::sync_file_system_providers::Entry;
    spec = FileSystemSpec;
    provider = ObservedProvider::new("inventory-sync");
}

/// Inventory construction discovers submitted providers and validates their
/// outputs.
#[test]
fn test_sync_inventory_discovers_and_validates_provider() {
    let registry = FileSystemRegistry::from_inventory().expect("inventory should build");
    assert_eq!(registry.provider_ids()[0].as_str(), "inventory-sync");
    let config = FileSystemConfig::new(ConnectionUri::parse("inventory-sync:///resource").expect("valid URI"));
    let resolution = registry.resolve_config(&config).expect("matching provider identity");
    assert_eq!(
        resolution.file_system().properties().info().provider_id(),
        "inventory-sync"
    );
}

qubit_spi::submit_sync_provider! {
    inventory_entry = qubit_fs_registry::sync_file_system_providers::Entry;
    spec = FileSystemSpec;
    provider = ObservedProvider { output_id: "other", ..ObservedProvider::new("inventory-sync-mismatch") };
}

/// Discovered providers use the same identity-checking adapter as explicit
/// ones.
#[test]
fn test_sync_inventory_rejects_provider_identity_mismatch() {
    let registry = FileSystemRegistry::from_inventory().expect("inventory should build");
    let config = FileSystemConfig::new(ConnectionUri::parse("inventory-sync-mismatch:///resource").expect("valid URI"));
    let error = registry
        .resolve_config(&config)
        .expect_err("identity mismatch should fail");
    let FileSystemRegistryError::Creation(error) = error else {
        panic!("expected creation error")
    };
    assert_eq!(
        error.decisive_attempt().failure().error().kind(),
        FsErrorKind::ProviderContractViolation
    );
}

#[cfg(feature = "async")]
qubit_spi::submit_async_provider! {
    inventory_entry = qubit_fs_registry::async_file_system_providers::Entry;
    spec = FileSystemSpec;
    provider = ObservedProvider::new("inventory-async");
}

/// Async discovery constructs a registry without starting service creation.
#[cfg(feature = "async")]
#[test]
fn test_async_inventory_discovers_provider() {
    let registry = AsyncFileSystemRegistry::from_inventory().expect("async inventory should build");
    assert_eq!(registry.provider_ids()[0].as_str(), "inventory-async");
    let config = FileSystemConfig::new(ConnectionUri::parse("inventory-async:///resource").expect("valid URI"));
    let resolution = common::block_on(registry.resolve_config(config)).expect("matching provider identity");
    assert_eq!(
        resolution.file_system().properties().info().provider_id(),
        "inventory-async"
    );
}

#[cfg(feature = "async")]
qubit_spi::submit_async_provider! {
    inventory_entry = qubit_fs_registry::async_file_system_providers::Entry;
    spec = FileSystemSpec;
    provider = ObservedProvider { output_id: "other", ..ObservedProvider::new("inventory-async-mismatch") };
}

/// Async discovered providers also reject facade identities that contradict
/// metadata.
#[cfg(feature = "async")]
#[test]
fn test_async_inventory_rejects_provider_identity_mismatch() {
    let registry = AsyncFileSystemRegistry::from_inventory().expect("async inventory should build");
    let config =
        FileSystemConfig::new(ConnectionUri::parse("inventory-async-mismatch:///resource").expect("valid URI"));
    let error = common::block_on(registry.resolve_config(config)).expect_err("identity mismatch should fail");
    let FileSystemRegistryError::Creation(error) = error else {
        panic!("expected creation error")
    };
    assert_eq!(
        error.decisive_attempt().failure().error().kind(),
        FsErrorKind::ProviderContractViolation
    );
}
