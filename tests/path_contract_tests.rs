// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Regression tests for resolution path validation delegation.

#[path = "common.rs"]
mod common;

use qubit_fs::error::FsErrorKind;
use qubit_fs::metadata::FileSystemLimit;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::path::PathConstraints;

#[test]
fn synchronous_resolution_rejects_paths_outside_the_filesystem_contract() {
    let error = common::sync_resolution_with_path_properties(
        "path-contract",
        "resource",
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
    )
    .expect_err("relative path must be rejected by an absolute-only filesystem");
    assert_eq!(FsErrorKind::InvalidPath, error.kind());

    let error = common::sync_resolution_with_path_properties(
        "path-contract",
        "/long",
        FileSystemLimits::unknown().with_max_path_text_bytes(FileSystemLimit::Maximum(3)),
        PathConstraints::absolute(),
    )
    .expect_err("paths exceeding the configured limit must be rejected");
    assert_eq!(FsErrorKind::ResourceLimitExceeded, error.kind());
}

#[cfg(feature = "async")]
#[test]
fn asynchronous_resolution_rejects_paths_outside_the_filesystem_contract() {
    let error = common::async_resolution_with_path_properties(
        "path-contract",
        "resource",
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
    )
    .expect_err("relative path must be rejected by an absolute-only filesystem");
    assert_eq!(FsErrorKind::InvalidPath, error.kind());

    let error = common::async_resolution_with_path_properties(
        "path-contract",
        "/long",
        FileSystemLimits::unknown().with_max_path_text_bytes(FileSystemLimit::Maximum(3)),
        PathConstraints::absolute(),
    )
    .expect_err("paths exceeding the configured limit must be rejected");
    assert_eq!(FsErrorKind::ResourceLimitExceeded, error.kind());
}
