// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private helpers shared by synchronous and asynchronous registries.

mod provider_adapter;
mod registry_support;
#[cfg(feature = "async")]
mod validating_async_file_system_provider;
mod validating_file_system_provider;

pub(super) use registry_support::{
    ensure_selection_matches_config,
    selection_for_config,
    validate_credentials,
};
#[cfg(feature = "async")]
pub(super) use validating_async_file_system_provider::ValidatingAsyncFileSystemProvider;
pub(super) use validating_file_system_provider::ValidatingFileSystemProvider;
