// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::sync::Arc;

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
use qubit_spi::FallbackPolicy;
use qubit_spi::ProviderDescriptor;
#[cfg(feature = "async")]
use qubit_spi::ProviderFuture;
use qubit_spi::ProviderId;
use qubit_spi::ProviderMetadata;
use qubit_spi::ProviderSelection;
use qubit_spi::ServiceProvider;
use qubit_spi::error::ProviderFailure;
use qubit_spi::error::ProviderFailureKind;

use crate::common;
use crate::support::provider_fixtures::ObservedProvider;

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
        ConnectionUri::parse("registered-async:///resource").expect("valid URI"),
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
        ProviderDescriptor::new(ProviderId::new("registered-sync").expect("provider id"))
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

/// Filesystem identity validation retains its failure classification across SPI
/// fallback policies.

#[test]
fn test_identity_failure_respects_fallback_policy() {
    for policy in [FallbackPolicy::OnAbsence, FallbackPolicy::OnAnyError] {
        for second_fails in [false, true] {
            let registry = FileSystemRegistry::default();
            let mut first = ObservedProvider::new("first");
            first.output_id = "wrong";
            let first_calls = Arc::clone(&first.calls);
            let mut second = ObservedProvider::new("second");
            second.fail = second_fails;
            let second_calls = Arc::clone(&second.calls);
            registry.register(first).expect("first");
            registry.register(second).expect("second");
            let selection = ProviderSelection::chain(["first", "second"])
                .expect("chain")
                .with_fallback_policy(policy);
            let config =
                FileSystemConfig::new(ConnectionUri::parse("file:///resource").expect("URI"));
            let result = registry.resolve_selected_config(&selection, &config);
            assert_eq!(first_calls.lock().expect("calls").len(), 1);
            if policy == FallbackPolicy::OnAbsence {
                let FileSystemRegistryError::Creation(error) =
                    result.expect_err("identity failure stops absence fallback")
                else {
                    panic!("creation error")
                };
                assert_eq!(error.attempts().len(), 1);
                assert_eq!(error.decisive_attempt().provider_id().as_str(), "first");
                assert_eq!(
                    error.decisive_attempt().failure().kind(),
                    ProviderFailureKind::InitializationFailed
                );
                assert_eq!(
                    error.decisive_attempt().failure().error().kind(),
                    FsErrorKind::ProviderContractViolation
                );
                assert!(second_calls.lock().expect("calls").is_empty());
            } else {
                assert_eq!(second_calls.lock().expect("calls").len(), 1);
                if second_fails {
                    let error = result.expect_err("all providers fail");
                    for text in [format!("{error}"), format!("{error:?}")] {
                        assert!(!text.contains("leaf-secret"));
                        assert!(!text.contains("source-secret"));
                    }
                    let FileSystemRegistryError::Creation(error) = error else {
                        panic!("creation error")
                    };
                    assert_eq!(
                        error
                            .attempts()
                            .iter()
                            .map(|a| a.provider_id().as_str())
                            .collect::<Vec<_>>(),
                        ["first", "second"]
                    );
                    assert_eq!(
                        error.attempts()[0].failure().error().kind(),
                        FsErrorKind::ProviderContractViolation
                    );
                    assert_eq!(
                        error.decisive_attempt().failure().kind(),
                        ProviderFailureKind::Unavailable
                    );
                    assert_eq!(error.decisive_attempt().provider_id().as_str(), "second");
                    assert!(
                        std::error::Error::source(error.decisive_attempt().failure().error())
                            .is_some()
                    );
                } else {
                    let resolution = result.expect("any-error reaches second provider");
                    assert_eq!(
                        resolution.file_system().properties().info().provider_id(),
                        "second"
                    );
                }
            }
        }
    }
}

/// Filesystem identity validation retains its failure classification across SPI
/// fallback policies.
#[cfg(feature = "async")]
#[test]
fn test_async_identity_failure_respects_fallback_policy() {
    for policy in [FallbackPolicy::OnAbsence, FallbackPolicy::OnAnyError] {
        for second_fails in [false, true] {
            let registry = AsyncFileSystemRegistry::default();
            let mut first = ObservedProvider::new("first");
            first.output_id = "wrong";
            let first_calls = Arc::clone(&first.calls);
            let mut second = ObservedProvider::new("second");
            second.fail = second_fails;
            let second_calls = Arc::clone(&second.calls);
            registry.register(first).expect("first");
            registry.register(second).expect("second");
            let selection = ProviderSelection::chain(["first", "second"])
                .expect("chain")
                .with_fallback_policy(policy);
            let config =
                FileSystemConfig::new(ConnectionUri::parse("file:///resource").expect("URI"));
            let result = common::block_on(registry.resolve_selected_config(selection, config));
            assert_eq!(first_calls.lock().expect("calls").len(), 1);
            if policy == FallbackPolicy::OnAbsence {
                let FileSystemRegistryError::Creation(error) =
                    result.expect_err("identity failure stops absence fallback")
                else {
                    panic!("creation error")
                };
                assert_eq!(error.attempts().len(), 1);
                assert_eq!(error.decisive_attempt().provider_id().as_str(), "first");
                assert_eq!(
                    error.decisive_attempt().failure().kind(),
                    ProviderFailureKind::InitializationFailed
                );
                assert_eq!(
                    error.decisive_attempt().failure().error().kind(),
                    FsErrorKind::ProviderContractViolation
                );
                assert!(second_calls.lock().expect("calls").is_empty());
            } else {
                assert_eq!(second_calls.lock().expect("calls").len(), 1);
                if second_fails {
                    let error = result.expect_err("all providers fail");
                    for text in [format!("{error}"), format!("{error:?}")] {
                        assert!(!text.contains("leaf-secret"));
                        assert!(!text.contains("source-secret"));
                    }
                    let FileSystemRegistryError::Creation(error) = error else {
                        panic!("creation error")
                    };
                    assert_eq!(
                        error
                            .attempts()
                            .iter()
                            .map(|a| a.provider_id().as_str())
                            .collect::<Vec<_>>(),
                        ["first", "second"]
                    );
                    assert_eq!(
                        error.attempts()[0].failure().error().kind(),
                        FsErrorKind::ProviderContractViolation
                    );
                    assert_eq!(
                        error.decisive_attempt().failure().kind(),
                        ProviderFailureKind::Unavailable
                    );
                    assert_eq!(error.decisive_attempt().provider_id().as_str(), "second");
                    assert!(
                        std::error::Error::source(error.decisive_attempt().failure().error())
                            .is_some()
                    );
                } else {
                    let resolution = result.expect("any-error reaches second provider");
                    assert_eq!(
                        resolution.file_system().properties().info().provider_id(),
                        "second"
                    );
                }
            }
        }
    }
}
