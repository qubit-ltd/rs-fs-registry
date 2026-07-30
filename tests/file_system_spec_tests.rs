// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::FsError;
use qubit_fs_registry::{
    AsyncFileSystemResolution,
    FileSystemConfig,
    FileSystemResolution,
    FileSystemSpec,
};
use qubit_spi::{
    AsyncServiceSpec,
    ServiceSpec,
    SyncServiceSpec,
};

/// The filesystem service specification binds the registry's public config,
/// error, and resolution types.
#[test]
fn test_file_system_spec_associated_types() {
    assert_file_system_spec::<FileSystemSpec>();
}

/// Requires the exact associated-type contract expected by registry providers.
///
/// # Type Parameters
///
/// - `S`: Service specification whose associated types are checked.
fn assert_file_system_spec<S>()
where
    S: ServiceSpec<Config = FileSystemConfig, Error = FsError>
        + SyncServiceSpec<Output = FileSystemResolution>
        + AsyncServiceSpec<Output = AsyncFileSystemResolution>,
{
}
