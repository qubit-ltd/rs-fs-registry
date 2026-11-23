// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use qubit_fs::FsError;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemRegistry;
use qubit_fs_registry::FileSystemRegistryError;
use qubit_fs_registry::FileSystemResolution;
use qubit_fs_registry::FileSystemSpec;
use qubit_spi::ProviderDescriptor;
use qubit_spi::ProviderId;
use qubit_spi::ProviderMetadata;
use qubit_spi::ServiceProvider;
use qubit_spi::error::ProviderFailure;

static SYNC_FACTORY_CALLS: AtomicUsize = AtomicUsize::new(0);

struct AmbiguousSyncProvider;

impl ProviderMetadata for AmbiguousSyncProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(ProviderId::new("ambiguous-sync").expect("static ID"))
    }
}

impl ServiceProvider<FileSystemSpec> for AmbiguousSyncProvider {
    fn create_configured(&self, _config: &FileSystemConfig) -> Result<FileSystemResolution, ProviderFailure<FsError>> {
        unreachable!("inventory source ambiguity fails before provider creation")
    }
}

fn make_sync_provider() -> AmbiguousSyncProvider {
    SYNC_FACTORY_CALLS.fetch_add(1, Ordering::SeqCst);
    AmbiguousSyncProvider
}

macro_rules! submit_same_source_sync_providers {
    () => {
        qubit_spi::submit_sync_provider! {
            inventory_entry = qubit_fs_registry::sync_file_system_providers::Entry;
            spec = FileSystemSpec;
            provider = make_sync_provider();
        }
        qubit_spi::submit_sync_provider! {
            inventory_entry = qubit_fs_registry::sync_file_system_providers::Entry;
            spec = FileSystemSpec;
            provider = make_sync_provider();
        }
    };
}

submit_same_source_sync_providers!();

/// The sync registry API reports ambiguous inventory sources before factories
/// run.
#[test]
fn test_sync_inventory_ambiguity_precedes_provider_factories() {
    SYNC_FACTORY_CALLS.store(0, Ordering::SeqCst);
    let error = FileSystemRegistry::from_inventory().expect_err("duplicate source must fail");
    assert_eq!(error.reason_code(), "inventory_source_ambiguity");
    let FileSystemRegistryError::InventoryBuild(build_error) = error else {
        panic!("expected inventory build error")
    };
    assert!(build_error.registration_error().is_none());
    assert_eq!(build_error.source_location().crate_name(), "qubit-fs-registry");
    assert!(build_error.source_location().line() > 0);
    assert_eq!(SYNC_FACTORY_CALLS.load(Ordering::SeqCst), 0);
}

#[cfg(feature = "async")]
mod asynchronous {
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use qubit_fs::FsError;
    use qubit_fs_registry::AsyncFileSystemRegistry;
    use qubit_fs_registry::AsyncFileSystemResolution;
    use qubit_fs_registry::FileSystemConfig;
    use qubit_fs_registry::FileSystemRegistryError;
    use qubit_fs_registry::FileSystemSpec;
    use qubit_spi::AsyncServiceProvider;
    use qubit_spi::ProviderDescriptor;
    use qubit_spi::ProviderFuture;
    use qubit_spi::ProviderId;
    use qubit_spi::ProviderMetadata;
    use qubit_spi::error::ProviderFailure;

    static ASYNC_FACTORY_CALLS: AtomicUsize = AtomicUsize::new(0);

    struct AmbiguousAsyncProvider;

    impl ProviderMetadata for AmbiguousAsyncProvider {
        fn descriptor(&self) -> ProviderDescriptor {
            ProviderDescriptor::new(ProviderId::new("ambiguous-async").expect("static ID"))
        }
    }

    impl AsyncServiceProvider<FileSystemSpec> for AmbiguousAsyncProvider {
        fn create_configured<'a>(
            &'a self,
            _config: &'a FileSystemConfig,
        ) -> ProviderFuture<'a, Result<AsyncFileSystemResolution, ProviderFailure<FsError>>> {
            unreachable!("inventory source ambiguity fails before provider creation")
        }
    }

    fn make_async_provider() -> AmbiguousAsyncProvider {
        ASYNC_FACTORY_CALLS.fetch_add(1, Ordering::SeqCst);
        AmbiguousAsyncProvider
    }

    macro_rules! submit_same_source_async_providers {
        () => {
            qubit_spi::submit_async_provider! {
                inventory_entry = qubit_fs_registry::async_file_system_providers::Entry;
                spec = FileSystemSpec;
                provider = make_async_provider();
            }
            qubit_spi::submit_async_provider! {
                inventory_entry = qubit_fs_registry::async_file_system_providers::Entry;
                spec = FileSystemSpec;
                provider = make_async_provider();
            }
        };
    }

    submit_same_source_async_providers!();

    /// The async registry API reports ambiguous sources before factories run.
    #[test]
    fn test_async_inventory_ambiguity_precedes_provider_factories() {
        ASYNC_FACTORY_CALLS.store(0, Ordering::SeqCst);
        let error = AsyncFileSystemRegistry::from_inventory().expect_err("duplicate source must fail");
        assert_eq!(error.reason_code(), "inventory_source_ambiguity");
        let FileSystemRegistryError::InventoryBuild(build_error) = error else {
            panic!("expected inventory build error")
        };
        assert!(build_error.registration_error().is_none());
        assert_eq!(build_error.source_location().crate_name(), "qubit-fs-registry");
        assert!(build_error.source_location().line() > 0);
        assert_eq!(ASYNC_FACTORY_CALLS.load(Ordering::SeqCst), 0);
    }
}
