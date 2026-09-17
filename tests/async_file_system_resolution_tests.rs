// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::error::FsErrorKind;
use qubit_fs::metadata::FileSystemLimit;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::path::PathConstraints;

use crate::common::async_resolution;
use crate::common::async_resolution_with_path_properties;
use crate::common::async_resolution_with_scheme;

/// An asynchronous resolution retains its facade, decoded path, and canonical
/// URI across access and ownership transfer.
#[test]
fn test_async_resolution_exposes_and_transfers_components() {
    let resolution = async_resolution("async-resolution-provider");
    assert_eq!(
        resolution.file_system().properties().info().provider_id(),
        "async-resolution-provider"
    );
    assert_eq!(resolution.path().as_str(), "/resource");
    assert_eq!(resolution.canonical_uri().as_str(), "registry-test:///resource");
    let debug = format!("{resolution:?}");
    assert!(debug.contains("/resource"));
    let (file_system, path, uri) = resolution.into_parts();
    assert_eq!(
        file_system.properties().info().provider_id(),
        "async-resolution-provider"
    );
    assert_eq!(path.as_str(), "/resource");
    assert_eq!(uri.as_str(), "registry-test:///resource");
}

/// Async resolution construction enforces the configured path form before
/// exposing provider results to registry callers.
#[test]
fn test_async_resolution_rejects_path_outside_constraints() {
    let error = async_resolution_with_path_properties(
        "async-resolution-provider",
        "relative",
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
    )
    .expect_err("relative path must violate absolute constraints");
    assert_eq!(error.kind(), FsErrorKind::InvalidPath);
}

/// Async resolution construction enforces finite provider path-size limits.
#[test]
fn test_async_resolution_rejects_path_exceeding_limits() {
    let limits = FileSystemLimits::unknown().with_max_path_text_bytes(FileSystemLimit::Maximum(4));
    let error = async_resolution_with_path_properties(
        "async-resolution-provider",
        "/large",
        limits,
        PathConstraints::absolute(),
    )
    .expect_err("oversized path must violate the provider limit");
    assert_eq!(error.kind(), FsErrorKind::ResourceLimitExceeded);
}

/// Async resolutions reject canonical URI schemes the facade does not
/// advertise.
#[test]
fn test_async_resolution_rejects_unadvertised_canonical_uri_scheme() {
    let error = async_resolution_with_scheme("async-resolution-provider", "accepted", "rejected:///resource")
        .expect_err("unadvertised scheme must fail");
    assert_eq!(error.kind(), FsErrorKind::InvalidUri);
}

/// An asynchronous resolution must come from a filesystem that declares its
/// URI scheme.
#[test]
fn test_async_resolution_rejects_canonical_uri_when_no_scheme_is_advertised() {
    let error = async_resolution_with_path_properties(
        "async-resolution-provider",
        "/resource",
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
    )
    .expect_err("a scheme-less filesystem cannot publish a URI resolution");
    assert_eq!(error.kind(), FsErrorKind::InvalidUri);
}
