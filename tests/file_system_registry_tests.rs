// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::sync::{
    Arc,
    Mutex,
};

use qubit_fs::{
    FileMetadata,
    FileSystem,
    FileSystemCapabilities,
    FileSystemId,
    FileSystemInfo,
    FileSystemLimits,
    FileSystemProperties,
    FsError,
    FsErrorKind,
    FsOperation,
    FsPath,
    FsResult,
    FsUri,
    PathSemantics,
    UserMetadata,
};
use qubit_fs_registry::{
    FileSystemConfig,
    FileSystemProvider,
    FileSystemRegistry,
    FileSystemRegistryError,
    FileSystemResolution,
    FileSystemSpec,
};
use qubit_spi::{
    FallbackPolicy,
    ProviderDescriptor,
    ProviderId,
    ProviderMetadata,
    ProviderSelection,
    ServiceProvider,
    error::{
        ProviderFailure,
        ProviderFailureKind,
        ProviderSelectionBuildError,
    },
};

#[test]
fn test_sync_registry_exposes_catalog_state_and_low_level_resolution() {
    let registry = FileSystemRegistry::default();
    let selection =
        ProviderSelection::named("missing").expect("selection should parse");

    assert!(registry.is_empty());
    assert_eq!(0, registry.len());
    assert!(registry.descriptors().is_empty());
    assert!(registry.resolve_selected(&selection).is_err());
    assert!(registry.resolve().is_err());
}

#[test]
fn test_sync_registry_reports_registered_provider_descriptors() {
    let registry = FileSystemRegistry::default();
    registry
        .register(UnavailableProvider)
        .expect("provider should register");

    assert!(!registry.is_empty());
    assert_eq!(1, registry.len());
    assert_eq!(
        vec![ProviderId::new("unavailable").expect("provider ID should parse"),],
        registry.provider_ids(),
    );
    assert_eq!(
        vec!["unavailable"],
        registry
            .descriptors()
            .iter()
            .map(|descriptor| descriptor.id().as_str())
            .collect::<Vec<_>>(),
    );
}

#[test]
fn test_registry_registers_shared_providers_and_tracks_default_selection() {
    let registry = FileSystemRegistry::default();
    let provider: Arc<FileSystemProvider> = Arc::new(UnavailableProvider);
    registry
        .register_shared(provider)
        .expect("shared provider should register");
    let selection = ProviderSelection::named("unavailable")
        .expect("selection should parse");

    registry.set_default_selection(selection.clone());

    assert_eq!(selection, registry.default_selection());
}

/// Verifies clones share registrations and aliases.
#[test]
fn test_registry_clones_observe_runtime_registrations() {
    let registry = FileSystemRegistry::default();
    let clone = registry.clone();
    registry
        .register(CapturingProvider {
            captured: Arc::new(Mutex::new(None)),
        })
        .expect("provider should register");

    let selection =
        ProviderSelection::named("capture").expect("selection should parse");
    assert!(
        clone.resolve_selected(&selection).is_ok(),
        "the clone should observe the shared provider catalog",
    );
}

/// Verifies registration conflicts retain their SPI diagnostics.
#[test]
fn test_registry_preserves_registration_conflicts() {
    let registry = FileSystemRegistry::default();
    registry
        .register(UnavailableProvider)
        .expect("the first provider should register");
    let error = registry
        .register(UnavailableProvider)
        .expect_err("a duplicate provider ID should conflict");

    assert!(matches!(
        error,
        FileSystemRegistryError::Registration(_)
    ));
}

/// Verifies provider creation failures preserve classification and source.
#[test]
fn test_registry_preserves_provider_creation_failures() {
    let registry = FileSystemRegistry::default();
    registry
        .register(UnavailableProvider)
        .expect("provider should register");
    let config = FileSystemConfig::new(
        FsUri::parse("unavailable:///resource").expect("URI should parse"),
    );
    let error = registry
        .resolve_config(&config)
        .expect_err("the unavailable provider should fail creation");

    let FileSystemRegistryError::Creation(creation) = error else {
        panic!("creation should preserve its typed aggregate");
    };
    assert_eq!("unavailable", creation.decisive_attempt().provider_id().as_str());
    assert_eq!(ProviderFailureKind::Unavailable, creation.decisive_attempt().failure().kind());
    assert_eq!(FsErrorKind::ProviderUnavailable, creation.decisive_attempt().failure().error().kind());
}

#[test]
fn test_registry_preserves_each_provider_failure_class() {
    let cases = [
        (
            "unsupported",
            ProviderFailureKind::Unsupported,
            FsErrorKind::RequirementNotMet,
        ),
        (
            "invalid-configuration",
            ProviderFailureKind::InvalidConfiguration,
            FsErrorKind::InvalidOptions,
        ),
        (
            "initialization-failed",
            ProviderFailureKind::InitializationFailed,
            FsErrorKind::Other,
        ),
    ];

    for (id, provider_kind, filesystem_kind) in cases {
        let registry = FileSystemRegistry::default();
        registry
            .register(FailingProvider { id, provider_kind })
            .expect("provider should register");
        let config = FileSystemConfig::new(
            FsUri::parse(&format!("{id}:///resource"))
                .expect("URI should parse"),
        );

        let error = registry
            .resolve_config(&config)
            .expect_err("provider creation should fail");

        let FileSystemRegistryError::Creation(creation) = error else {
            panic!("creation should preserve its typed aggregate");
        };
        assert_eq!(id, creation.decisive_attempt().provider_id().as_str());
        assert_eq!(provider_kind, creation.decisive_attempt().failure().kind());
        assert_eq!(filesystem_kind, creation.decisive_attempt().failure().error().kind());
    }
}

#[test]
fn test_registry_preserves_invalid_uri_scheme_selection() {
    let registry = FileSystemRegistry::default();
    let config = FileSystemConfig::new(
        FsUri::parse("mock-:///resource").expect("URI should parse"),
    );

    let error = registry
        .resolve_config(&config)
        .expect_err("the URI scheme should not form a provider selector");

    assert!(matches!(
        error,
        FileSystemRegistryError::Selection(ProviderSelectionBuildError::InvalidSelector { .. })
    ));
}

/// Verifies a configured chain falls back after an unavailable provider.
#[test]
fn test_registry_applies_provider_fallback_policy() {
    let registry = FileSystemRegistry::default();
    registry
        .register(UnavailableProvider)
        .expect("the unavailable provider should register");
    registry
        .register(CapturingProvider {
            captured: Arc::new(Mutex::new(None)),
        })
        .expect("the fallback provider should register");
    let selection = ProviderSelection::chain(["unavailable", "capture"])
        .expect("the provider chain should parse")
        .with_fallback_policy(FallbackPolicy::OnAbsence);
    let config = FileSystemConfig::new(
        FsUri::parse("unrelated:///resource").expect("URI should parse"),
    )
    .with_selection(selection.clone());

    let resolution = registry
        .resolve_selected_config(&selection, &config)
        .expect("fallback should reach the capture provider");

    assert_eq!("provider-decoded/%252F", resolution.path().as_str());
}

/// Verifies full configuration reaches the selected provider unchanged.
#[test]
fn test_registry_binds_provider_decoded_paths_from_complete_configuration() {
    let captured = Arc::new(Mutex::new(None));
    let registry = FileSystemRegistry::default();
    registry
        .register(CapturingProvider {
            captured: captured.clone(),
        })
        .expect("provider should register");

    let config = FileSystemConfig::new(
        FsUri::parse("unrelated:///raw%2Fkey").expect("URI should parse"),
    )
    .with_selection(
        ProviderSelection::named("capture").expect("selection should parse"),
    )
    .with_options(
        UserMetadata::new()
            .with("region", "test-1")
            .expect("metadata should accept a non-sensitive key"),
    );

    let resource = registry
        .resource(&config)
        .expect("complete configuration should resolve");

    assert_eq!("provider-decoded/%252F", resource.path().as_str());
    assert_eq!(
        Some(config),
        captured.lock().expect("lock should succeed").clone()
    );
}

#[test]
fn test_registry_exposes_filesystem_and_uri_convenience_paths() {
    let registry = FileSystemRegistry::default();
    registry
        .register(CapturingProvider {
            captured: Arc::new(Mutex::new(None)),
        })
        .expect("provider should register");
    let uri = FsUri::parse("capture:///resource").expect("URI should parse");
    let config = FileSystemConfig::new(uri.clone());

    let filesystem = registry
        .file_system(&config)
        .expect("configuration should create a filesystem");
    let filesystem_from_uri = registry
        .file_system_uri(&uri)
        .expect("URI should create a filesystem");
    let resource = registry
        .resource_uri(&uri)
        .expect("URI should create a resource");

    assert_eq!("capture-instance", filesystem.info().id().as_str());
    assert_eq!("capture-instance", filesystem_from_uri.info().id().as_str());
    assert_eq!("provider-decoded/%252F", resource.path().as_str());
}

/// Verifies explicit and default configured resolution use the supplied
/// provider selection instead of the URI scheme.
#[test]
fn test_registry_resolves_selected_and_default_configurations() {
    let registry = FileSystemRegistry::default();
    registry
        .register(CapturingProvider {
            captured: Arc::new(Mutex::new(None)),
        })
        .expect("provider should register");
    let selection =
        ProviderSelection::named("capture").expect("selection should parse");
    let config = FileSystemConfig::new(
        FsUri::parse("unrelated:///resource").expect("URI should parse"),
    );

    registry
        .resolve_selected_config(&selection, &config)
        .expect("explicit selection should create a filesystem");

    registry.set_default_selection(selection);
    registry
        .resolve_default_config(&config)
        .expect("default selection should create a filesystem");
}

/// Verifies explicit selection rejects a conflicting configuration selection.
#[test]
fn test_registry_selected_configuration_rejects_conflicting_embedded_selection()
{
    let registry = FileSystemRegistry::default();
    let selection =
        ProviderSelection::named("requested").expect("selection should parse");
    let config = FileSystemConfig::new(
        FsUri::parse("unrelated:///resource").expect("URI should parse"),
    )
    .with_selection(
        ProviderSelection::named("embedded").expect("selection should parse"),
    );

    let error = registry
        .resolve_selected_config(&selection, &config)
        .expect_err("conflicting provider selections should be rejected");

    assert!(matches!(error, FileSystemRegistryError::SelectionConflict { .. }));
}

/// Verifies default selection rejects a conflicting configuration selection.
#[test]
fn test_registry_default_configuration_rejects_conflicting_embedded_selection()
{
    let registry = FileSystemRegistry::default();
    registry.set_default_selection(
        ProviderSelection::named("default").expect("selection should parse"),
    );
    let config = FileSystemConfig::new(
        FsUri::parse("unrelated:///resource").expect("URI should parse"),
    )
    .with_selection(
        ProviderSelection::named("embedded").expect("selection should parse"),
    );

    let error = registry
        .resolve_default_config(&config)
        .expect_err("conflicting provider selections should be rejected");

    assert!(matches!(error, FileSystemRegistryError::SelectionConflict { .. }));
}

struct UnavailableProvider;

impl ProviderMetadata for UnavailableProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(
            ProviderId::new("unavailable").expect("provider ID should parse"),
        )
    }
}

impl ServiceProvider<FileSystemSpec> for UnavailableProvider {
    fn create_configured(
        &self,
        _config: &FileSystemConfig,
    ) -> Result<FileSystemResolution<dyn FileSystem>, ProviderFailure<FsError>> {
        Err(ProviderFailure::unavailable(FsError::new(
            FsErrorKind::ProviderUnavailable,
            FsOperation::Provider,
            "provider is unavailable",
        )))
    }
}

struct FailingProvider {
    id: &'static str,
    provider_kind: ProviderFailureKind,
}

impl ProviderMetadata for FailingProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(
            ProviderId::new(self.id).expect("provider ID should parse"),
        )
    }
}

impl ServiceProvider<FileSystemSpec> for FailingProvider {
    fn create_configured(
        &self,
        _config: &FileSystemConfig,
    ) -> Result<FileSystemResolution<dyn FileSystem>, ProviderFailure<FsError>> {
        let error = FsError::new(
            match self.provider_kind {
                ProviderFailureKind::Unsupported => FsErrorKind::RequirementNotMet,
                ProviderFailureKind::InvalidConfiguration => FsErrorKind::InvalidOptions,
                ProviderFailureKind::InitializationFailed => FsErrorKind::Other,
                ProviderFailureKind::Unavailable => FsErrorKind::ProviderUnavailable,
                _ => FsErrorKind::Other,
            },
            FsOperation::Provider,
            "classified provider failure",
        );
        Err(match self.provider_kind {
            ProviderFailureKind::Unsupported => ProviderFailure::unsupported(error),
            ProviderFailureKind::Unavailable => ProviderFailure::unavailable(error),
            ProviderFailureKind::InvalidConfiguration => ProviderFailure::invalid_configuration(error),
            ProviderFailureKind::InitializationFailed => ProviderFailure::initialization_failed(error),
            _ => ProviderFailure::initialization_failed(error),
        })
    }
}

struct CapturingProvider {
    captured: Arc<Mutex<Option<FileSystemConfig>>>,
}

impl ProviderMetadata for CapturingProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(
            ProviderId::new("capture").expect("provider ID should parse"),
        )
    }
}

impl ServiceProvider<FileSystemSpec> for CapturingProvider {
    fn create_configured(
        &self,
        config: &FileSystemConfig,
    ) -> Result<FileSystemResolution<dyn FileSystem>, ProviderFailure<FsError>> {
        *self.captured.lock().expect("lock should succeed") =
            Some(config.clone());
        let filesystem: Arc<dyn FileSystem> = Arc::new(CapturingFileSystem);
        Ok(FileSystemResolution::new(
            filesystem,
            FsPath::parse_literal("provider-decoded/%252F")
                .expect("provider path should parse"),
            config.uri().clone(),
        ))
    }
}

struct CapturingFileSystem;

impl FileSystemProperties for CapturingFileSystem {
    fn info(&self) -> &FileSystemInfo {
        static INFO: std::sync::OnceLock<FileSystemInfo> =
            std::sync::OnceLock::new();
        INFO.get_or_init(|| {
            FileSystemInfo::new(
                FileSystemId::new("capture-instance")
                    .expect("the static filesystem ID should parse"),
                "capture",
                PathSemantics::ProviderSpecific,
            )
        })
    }

    fn capabilities(&self) -> FileSystemCapabilities {
        FileSystemCapabilities::default()
    }

    fn limits(&self) -> &FileSystemLimits {
        static LIMITS: FileSystemLimits = FileSystemLimits::unknown();
        &LIMITS
    }
}

impl FileSystem for CapturingFileSystem {
    fn stat(&self, _path: &FsPath) -> FsResult<FileMetadata> {
        Err(FsError::new(
            FsErrorKind::NotFound,
            FsOperation::Stat,
            "the capturing test filesystem has no resources",
        ))
    }
}
