// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::ConnectionUri;
use qubit_fs::FsError;
use qubit_fs_registry::AsyncFileSystemRegistry;
use qubit_fs_registry::AsyncFileSystemResolution;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemSpec;
use qubit_spi::AsyncServiceProvider;
use qubit_spi::ProviderDescriptor;
use qubit_spi::ProviderFuture;
use qubit_spi::ProviderId;
use qubit_spi::ProviderMetadata;
use qubit_spi::error::ProviderFailure;

use crate::common;

/// An asynchronous validating wrapper preserves a provider result whose
/// identity matches its registered descriptor.
#[test]
fn test_validating_async_file_system_provider_accepts_matching_identity() {
    let registry = AsyncFileSystemRegistry::default();
    registry
        .register(MatchingAsyncProvider)
        .expect("register matching provider");
    let config = FileSystemConfig::new(
        ConnectionUri::parse("registered-async:///resource")
            .expect("valid URI"),
    );

    let resolution = common::block_on(registry.resolve_config(config))
        .expect("matching provider identity must resolve");
    assert_eq!(
        resolution.file_system().properties().info().provider_id(),
        "registered-async"
    );
    assert_eq!(resolution.path().as_str(), "/resource");
    assert_eq!(
        resolution.canonical_uri().as_str(),
        "registry-test:///resource"
    );
}

/// Asynchronous provider fixture whose output identity matches its descriptor.
struct MatchingAsyncProvider;

impl ProviderMetadata for MatchingAsyncProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(
            ProviderId::new("registered-async").expect("provider id"),
        )
    }
}

impl AsyncServiceProvider<FileSystemSpec> for MatchingAsyncProvider {
    fn create_configured<'a>(
        &'a self,
        _: &'a FileSystemConfig,
    ) -> ProviderFuture<
        'a,
        Result<AsyncFileSystemResolution, ProviderFailure<FsError>>,
    > {
        Box::pin(async { Ok(common::async_resolution("registered-async")) })
    }
}
