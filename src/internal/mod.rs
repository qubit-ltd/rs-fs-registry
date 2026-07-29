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

pub(super) use provider_adapter::{
    ValidatingAsyncFileSystemProvider,
    ValidatingFileSystemProvider,
};
pub(super) use registry_support::{
    ensure_selection_matches_config,
    selection_for_config,
    validate_credentials,
};
