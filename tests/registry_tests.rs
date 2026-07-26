// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs_registry::{
    AsyncFileSystemRegistry,
    FileSystemConfig,
    FileSystemRegistry,
    FileSystemResolution,
    FileSystemSpec,
};
use qubit_fs::FileSystem;
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
