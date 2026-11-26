// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Link-time inventories for filesystem provider factories.
//!
//! The generated `build_registry` and `build_registry_with` functions return
//! raw SPI registries. They do not apply filesystem facade validation.
//! [`crate::FileSystemRegistry::from_inventory`] and the corresponding async
//! constructor adapt discovered providers to filesystem guarantees; facade
//! resolution applies the same guarantees to providers registered directly.

qubit_spi::declare_sync_provider_inventory! {
    pub mod sync_file_system_providers {
        spec = crate::FileSystemSpec;
    }
}

#[cfg(feature = "async")]
qubit_spi::declare_async_provider_inventory! {
    pub mod async_file_system_providers {
        spec = crate::FileSystemSpec;
    }
}
