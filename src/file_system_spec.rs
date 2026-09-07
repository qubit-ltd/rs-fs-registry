// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! SPI service specification for filesystems.

use qubit_fs::FsError;
#[cfg(feature = "async")]
use qubit_spi::AsyncServiceSpec;
use qubit_spi::ServiceSpec;
use qubit_spi::SyncServiceSpec;

#[cfg(feature = "async")]
use crate::AsyncFileSystemResolution;
use crate::FileSystemConfig;
use crate::FileSystemResolution;

/// Service specification for filesystem providers.
///
/// # Examples
///
/// ```
/// use qubit_fs::path::ConnectionUri;
/// use qubit_fs_registry::FileSystemConfig;
/// use qubit_fs_registry::FileSystemSpec;
/// use qubit_spi::ServiceSpec;
/// let _specification = FileSystemSpec;
/// let config: <FileSystemSpec as ServiceSpec>::Config =
///     FileSystemConfig::new(ConnectionUri::parse("file:///report.csv")?);
/// assert_eq!(config.uri().scheme(), "file");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct FileSystemSpec;

impl ServiceSpec for FileSystemSpec {
    type Config = FileSystemConfig;
    type Error = FsError;
}

impl SyncServiceSpec for FileSystemSpec {
    type Output = FileSystemResolution;
}

#[cfg(feature = "async")]
impl AsyncServiceSpec for FileSystemSpec {
    type Output = AsyncFileSystemResolution;
}
