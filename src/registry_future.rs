// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Future aliases for asynchronous filesystem registry operations.

use std::{
    future::Future,
    pin::Pin,
};

use crate::FileSystemRegistryError;

/// Result returned by filesystem registry operations.
pub type FileSystemRegistryResult<T> = Result<T, FileSystemRegistryError>;

/// Sendable future returned by an asynchronous filesystem registry operation.
pub type RegistryFuture<'a, T> = Pin<Box<dyn Future<Output = FileSystemRegistryResult<T>> + Send + 'a>>;
