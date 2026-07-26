// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! SPI service specification for filesystems.

use qubit_spi::{
    AsyncServiceSpec,
    ServiceSpec,
    SyncServiceSpec,
};

use crate::{
    FileSystemConfig,
    FileSystemResolution,
};
use qubit_fs::{
    AsyncFileSystem,
    FileSystem,
};

/// Service specification for filesystem providers.
#[derive(Debug)]
pub struct FileSystemSpec;

impl ServiceSpec for FileSystemSpec {
    type Config = FileSystemConfig;
}

impl SyncServiceSpec for FileSystemSpec {
    type Output = FileSystemResolution<dyn FileSystem>;
}

impl AsyncServiceSpec for FileSystemSpec {
    type Output = FileSystemResolution<dyn AsyncFileSystem>;
}
