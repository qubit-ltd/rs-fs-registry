// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::FsError;
use qubit_fs::error::FsErrorKind;
use qubit_fs::path::ConnectionUri;
#[cfg(feature = "async")]
use qubit_fs_registry::AsyncFileSystemRegistry;
#[cfg(feature = "async")]
use qubit_fs_registry::AsyncFileSystemResolution;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemRegistry;
use qubit_fs_registry::FileSystemRegistryError;
use qubit_fs_registry::FileSystemResolution;
use qubit_fs_registry::FileSystemSpec;
#[cfg(feature = "async")]
use qubit_spi::AsyncServiceProvider;
use qubit_spi::ProviderDescriptor;
#[cfg(feature = "async")]
use qubit_spi::ProviderFuture;
use qubit_spi::ProviderId;
use qubit_spi::ProviderMetadata;
use qubit_spi::ServiceProvider;
use qubit_spi::error::ProviderFailure;

use crate::common;

/// Provider-adapter validation rejects a resolution whose filesystem identity
/// differs from the registered descriptor.
#[test]
fn test_provider_adapter_rejects_mismatched_provider_identity() {
    let registry = FileSystemRegistry::default();
    registry
        .register(MismatchedProvider)
        .expect("register mismatched provider");
    let config = FileSystemConfig::new(ConnectionUri::parse("registered-sync:///resource").expect("valid URI"));

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

/// Asynchronous provider-adapter validation rejects a resolution whose
/// filesystem identity differs from the registered descriptor.
#[cfg(feature = "async")]
#[test]
fn test_provider_adapter_rejects_mismatched_async_provider_identity() {
    let registry = AsyncFileSystemRegistry::default();
    registry
        .register(MismatchedAsyncProvider)
        .expect("register mismatched provider");
    let config = FileSystemConfig::new(ConnectionUri::parse("registered-async:///resource").expect("valid URI"));

    let error = common::block_on(registry.resolve_config(config)).expect_err("mismatched provider identity must fail");
    let FileSystemRegistryError::Creation(creation) = error else {
        panic!("expected provider creation error")
    };
    assert_eq!(
        creation.decisive_attempt().failure().error().kind(),
        FsErrorKind::ProviderContractViolation
    );
}

/// Provider fixture whose output intentionally contradicts its descriptor.
struct MismatchedProvider;

impl ProviderMetadata for MismatchedProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(ProviderId::new("registered-sync").expect("provider id"))
    }
}

impl ServiceProvider<FileSystemSpec> for MismatchedProvider {
    fn create_configured(&self, _: &FileSystemConfig) -> Result<FileSystemResolution, ProviderFailure<FsError>> {
        Ok(common::sync_resolution("reported-sync"))
    }
}

/// Asynchronous fixture whose output intentionally contradicts its descriptor.
#[cfg(feature = "async")]
struct MismatchedAsyncProvider;

#[cfg(feature = "async")]
impl ProviderMetadata for MismatchedAsyncProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(ProviderId::new("registered-async").expect("provider id"))
    }
}

#[cfg(feature = "async")]
impl AsyncServiceProvider<FileSystemSpec> for MismatchedAsyncProvider {
    fn create_configured<'a>(
        &'a self,
        _: &'a FileSystemConfig,
    ) -> ProviderFuture<'a, Result<AsyncFileSystemResolution, ProviderFailure<FsError>>> {
        Box::pin(async { Ok(common::async_resolution("reported-async")) })
    }
}
