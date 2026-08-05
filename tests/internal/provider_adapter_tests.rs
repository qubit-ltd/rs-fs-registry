// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::{
    ConnectionUri,
    FsError,
    FsErrorKind,
};
#[cfg(feature = "async")]
use qubit_fs_registry::{
    AsyncFileSystemRegistry,
    AsyncFileSystemResolution,
};
use qubit_fs_registry::{
    FileSystemConfig,
    FileSystemRegistry,
    FileSystemRegistryError,
    FileSystemResolution,
    FileSystemSpec,
};
use qubit_spi::error::ProviderFailure;
#[cfg(feature = "async")]
use qubit_spi::{
    AsyncServiceProvider,
    ProviderFuture,
};
use qubit_spi::{
    ProviderDescriptor,
    ProviderId,
    ProviderMetadata,
    ServiceProvider,
};

use crate::common;

/// Provider-adapter validation rejects a resolution whose filesystem identity
/// differs from the registered descriptor.
#[test]
fn test_provider_adapter_rejects_mismatched_provider_identity() {
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

/// Asynchronous provider-adapter validation rejects a resolution whose
/// filesystem identity differs from the registered descriptor.
#[cfg(feature = "async")]
#[test]
fn test_provider_adapter_rejects_mismatched_async_provider_identity() {
    let registry = AsyncFileSystemRegistry::default();
    registry
        .register(MismatchedAsyncProvider)
        .expect("register mismatched provider");
    let config = FileSystemConfig::new(
        ConnectionUri::parse("registered-async:///resource")
            .expect("valid URI"),
    );

    let error = common::block_on(registry.resolve_config(config))
        .expect_err("mismatched provider identity must fail");
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

/// Asynchronous fixture whose output intentionally contradicts its descriptor.
#[cfg(feature = "async")]
struct MismatchedAsyncProvider;

#[cfg(feature = "async")]
impl ProviderMetadata for MismatchedAsyncProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(
            ProviderId::new("registered-async").expect("provider id"),
        )
    }
}

#[cfg(feature = "async")]
impl AsyncServiceProvider<FileSystemSpec> for MismatchedAsyncProvider {
    fn create_configured<'a>(
        &'a self,
        _: &'a FileSystemConfig,
    ) -> ProviderFuture<
        'a,
        Result<AsyncFileSystemResolution, ProviderFailure<FsError>>,
    > {
        Box::pin(async { Ok(common::async_resolution("reported-async")) })
    }
}
