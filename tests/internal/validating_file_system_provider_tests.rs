// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::FsError;
use qubit_fs::path::ConnectionUri;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemRegistry;
use qubit_fs_registry::FileSystemResolution;
use qubit_fs_registry::FileSystemSpec;
use qubit_spi::ProviderDescriptor;
use qubit_spi::ProviderId;
use qubit_spi::ProviderMetadata;
use qubit_spi::ServiceProvider;
use qubit_spi::error::ProviderFailure;

use crate::common;

/// A synchronous validating wrapper preserves a provider result whose identity
/// matches its registered descriptor.
#[test]
fn test_validating_file_system_provider_accepts_matching_identity() {
    let registry = FileSystemRegistry::default();
    registry.register(MatchingProvider).expect("register matching provider");
    let config = FileSystemConfig::new(ConnectionUri::parse("registered-sync:///resource").expect("valid URI"));

    let resolution = registry
        .resolve_config(&config)
        .expect("matching provider identity must resolve");
    assert_eq!(
        resolution.file_system().properties().info().provider_id(),
        "registered-sync"
    );
    assert_eq!(resolution.path().as_str(), "/resource");
    assert_eq!(resolution.canonical_uri().as_str(), "registry-test:///resource");
}

/// Provider fixture whose output identity matches its descriptor.
struct MatchingProvider;

impl ProviderMetadata for MatchingProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(ProviderId::new("registered-sync").expect("provider id"))
    }
}

impl ServiceProvider<FileSystemSpec> for MatchingProvider {
    fn create_configured(&self, _: &FileSystemConfig) -> Result<FileSystemResolution, ProviderFailure<FsError>> {
        Ok(common::sync_resolution("registered-sync"))
    }
}
