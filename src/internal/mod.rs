// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private helpers shared by synchronous and asynchronous registries.

mod registry_support;

pub(super) use registry_support::{
    ensure_selection_matches_config,
    map_provider_creation_error,
    map_provider_resolution_error,
    map_registration_error,
    selection_for_config,
};
