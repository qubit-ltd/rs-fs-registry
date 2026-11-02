// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use qubit_fs::FsError;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::path::ConnectionUri;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemRegistry;
use qubit_fs_registry::FileSystemRegistryError;
use qubit_fs_registry::FileSystemResolution;
use qubit_fs_registry::FileSystemSpec;
use qubit_redact::RedactionPolicy;
use qubit_redact::Redactor;
use qubit_spi::ProviderDescriptor;
use qubit_spi::ProviderId;
use qubit_spi::ProviderMetadata;
use qubit_spi::ServiceProvider;
use qubit_spi::error::ProviderFailure;

struct DefaultRedactorGuard(Option<Redactor>);

impl Drop for DefaultRedactorGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.0.take() {
            drop(Redactor::replace_application_default(previous));
        }
    }
}

struct ObservedSource(Arc<AtomicUsize>);

impl fmt::Display for ObservedSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fetch_add(1, Ordering::SeqCst);
        formatter.write_str("source-secret-marker")
    }
}

impl fmt::Debug for ObservedSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fetch_add(1, Ordering::SeqCst);
        formatter.write_str("source-secret-marker")
    }
}

impl Error for ObservedSource {}

struct FormattingFailureProvider(Arc<AtomicUsize>);

impl ProviderMetadata for FormattingFailureProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(ProviderId::new("formatting-fixture").expect("fixture ID"))
    }
}

impl ServiceProvider<FileSystemSpec> for FormattingFailureProvider {
    fn create_configured(&self, _: &FileSystemConfig) -> Result<FileSystemResolution, ProviderFailure<FsError>> {
        Err(ProviderFailure::initialization_failed(FsError::with_source(
            FsErrorKind::ProviderContractViolation,
            FsOperation::Provider,
            "leaf-secret-marker",
            ObservedSource(Arc::clone(&self.0)),
        )))
    }
}

/// Public registry formatting is independent of application defaults and
/// sources.
#[test]
fn test_public_formatting_keeps_standard_policy_and_does_not_format_sources() {
    let calls = Arc::new(AtomicUsize::new(0));
    let registry = FileSystemRegistry::default();
    registry
        .register(FormattingFailureProvider(Arc::clone(&calls)))
        .expect("register fixture");
    let config = FileSystemConfig::new(ConnectionUri::parse("formatting-fixture:///resource").expect("fixture URI"));
    let creation = registry.resolve_config(&config).expect_err("fixture fails");
    let invalid = FileSystemRegistryError::InvalidConfiguration {
        message: "token=configuration-secret-marker",
    };
    let before = [
        format!("{invalid}"),
        format!("{invalid:?}"),
        format!("{creation}"),
        format!("{creation:?}"),
    ];
    let guard = DefaultRedactorGuard(Some(Redactor::replace_application_default(Redactor::new(
        RedactionPolicy::disabled(),
    ))));
    let after = [
        format!("{invalid}"),
        format!("{invalid:?}"),
        format!("{creation}"),
        format!("{creation:?}"),
    ];
    assert_eq!(before, after);
    for text in &after[..2] {
        assert!(text.contains("<redacted>"));
        assert!(!text.contains("configuration-secret-marker"));
        assert!(text.contains("invalid_configuration"));
    }
    for text in &after[2..] {
        assert!(text.contains("provider_creation_failed"));
        assert!(text.contains("formatting-fixture"));
        assert!(!text.contains("leaf-secret-marker"));
        assert!(!text.contains("source-secret-marker"));
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let FileSystemRegistryError::Creation(aggregate) = &creation else {
        panic!("creation aggregate expected");
    };
    let source = aggregate
        .decisive_attempt()
        .failure()
        .error()
        .source()
        .and_then(|source| source.downcast_ref::<ObservedSource>())
        .expect("original typed provider source retained");
    assert_eq!(source.to_string(), "source-secret-marker");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    drop(guard);
    let restored = [
        format!("{invalid}"),
        format!("{invalid:?}"),
        format!("{creation}"),
        format!("{creation:?}"),
    ];
    assert_eq!(before, restored);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
