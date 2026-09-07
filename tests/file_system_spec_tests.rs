// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::FsError;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemResolution;
use qubit_fs_registry::FileSystemSpec;
use qubit_spi::ServiceSpec;
use qubit_spi::SyncServiceSpec;

/// The filesystem service specification binds the registry's public config,
/// error, and resolution types.
#[test]
fn test_file_system_spec_associated_types() {
    assert_sync_file_system_spec::<FileSystemSpec>();
}

/// Requires the exact associated-type contract expected by registry providers.
///
/// # Type Parameters
///
/// - `S`: Service specification whose associated types are checked.
fn assert_sync_file_system_spec<S>()
where
    S: ServiceSpec<Config = FileSystemConfig, Error = FsError> + SyncServiceSpec<Output = FileSystemResolution>,
{
}

#[cfg(feature = "async")]
use qubit_fs_registry::AsyncFileSystemResolution;
#[cfg(feature = "async")]
use qubit_spi::AsyncServiceSpec;

/// Requires the exact asynchronous associated-type contract expected by
/// registry providers.
#[cfg(feature = "async")]
#[test]
fn test_file_system_spec_async_associated_type() {
    assert_async_file_system_spec::<FileSystemSpec>();
}

/// Requires the exact asynchronous associated-type contract expected by
/// registry providers.
///
/// # Type Parameters
///
/// - `S`: Service specification whose asynchronous associated type is checked.
#[cfg(feature = "async")]
fn assert_async_file_system_spec<S>()
where
    S: ServiceSpec<Config = FileSystemConfig, Error = FsError> + AsyncServiceSpec<Output = AsyncFileSystemResolution>,
{
}
