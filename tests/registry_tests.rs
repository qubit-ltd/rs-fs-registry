// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::{
    sync::{
        Arc,
        Mutex,
    },
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
    AsyncFileSystemRegistry,
    FileSystemConfig,
    FileSystemRegistry,
    FileSystemResolution,
    FileSystemSpec,
};
use qubit_spi::{
    ProviderDescriptor,
    ProviderId,
    ProviderMetadata,
    ProviderSelection,
    ServiceProvider,
    error::ProviderError,
};

#[test]
fn sync_registry_exposes_catalog_state_and_low_level_resolution() {
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
fn sync_registry_reports_registered_provider_descriptors() {
    let registry = FileSystemRegistry::default();
    registry
        .register(UnavailableProvider)
        .expect("provider should register");

    assert!(!registry.is_empty());
    assert_eq!(1, registry.len());
    assert_eq!(vec!["unavailable"], registry.provider_ids());
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
fn async_registry_exposes_catalog_state_and_low_level_resolution() {
    let registry = AsyncFileSystemRegistry::default();
    let selection =
        ProviderSelection::named("missing").expect("selection should parse");

    assert!(registry.is_empty());
    assert_eq!(0, registry.len());
    assert!(registry.descriptors().is_empty());
    assert!(registry.resolve_selected(&selection).is_err());
    assert!(registry.resolve().is_err());
}

/// Verifies full configuration reaches the selected provider unchanged.
#[test]
fn registry_binds_provider_decoded_paths_from_complete_configuration() {
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
    assert_eq!(Some(config), captured.lock().expect("lock should succeed").clone());
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
    ) -> Result<FileSystemResolution<dyn FileSystem>, ProviderError> {
        Err(ProviderError::unavailable("provider is unavailable"))
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
    ) -> Result<FileSystemResolution<dyn FileSystem>, ProviderError> {
        *self.captured.lock().expect("lock should succeed") = Some(config.clone());
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
        static INFO: std::sync::OnceLock<FileSystemInfo> = std::sync::OnceLock::new();
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
