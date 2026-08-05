// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

#[cfg(feature = "async")]
mod async_file_system_registry_tests;
#[cfg(feature = "async")]
mod async_file_system_resolution_tests;
mod common;
mod credential_ref_tests;
mod file_system_config_tests;
mod file_system_registry_error_tests;
mod file_system_registry_tests;
mod file_system_resolution_tests;
mod file_system_spec_tests;
mod internal;
mod readme_examples_tests;
