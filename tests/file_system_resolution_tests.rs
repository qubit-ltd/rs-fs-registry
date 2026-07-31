// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::{
    FileSystemLimit,
    FileSystemLimits,
    FsErrorKind,
    PathConstraints,
    Uri,
};

use crate::common::{
    sync_resolution,
    sync_resolution_with_path_properties,
    sync_resolution_with_scheme,
};
/// Canonical resolution URIs accept no embedded credential material.
#[test]
fn test_resolution_boundary_uses_secret_free_uri() {
    assert!(Uri::parse("s3://bucket/key").is_ok());
    assert!(Uri::parse("s3://user:password@bucket/key").is_err());
}

/// Resolution construction enforces the configured path form before exposing
/// provider results to registry callers.
#[test]
fn test_resolution_rejects_path_outside_constraints() {
    let error = sync_resolution_with_path_properties(
        "resolution-provider",
        "relative",
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
    )
    .expect_err("relative path must violate absolute constraints");
    assert_eq!(error.kind(), FsErrorKind::InvalidPath);
}

/// Resolution construction enforces finite provider path-size limits.
#[test]
fn test_resolution_rejects_path_exceeding_limits() {
    let limits = FileSystemLimits::unknown()
        .with_max_path_text_bytes(FileSystemLimit::Maximum(4));
    let error = sync_resolution_with_path_properties(
        "resolution-provider",
        "/large",
        limits,
        PathConstraints::absolute(),
    )
    .expect_err("oversized path must violate the provider limit");
    assert_eq!(error.kind(), FsErrorKind::ResourceLimitExceeded);
}

/// A synchronous resolution retains its facade, decoded path, and canonical
/// URI across access and ownership transfer.
#[test]
fn test_resolution_exposes_and_transfers_components() {
    let resolution = sync_resolution("resolution-provider");
    assert_eq!(
        resolution.file_system().properties().info().provider_id(),
        "resolution-provider"
    );
    assert_eq!(resolution.path().as_str(), "/resource");
    assert_eq!(
        resolution.canonical_uri().as_str(),
        "registry-test:///resource"
    );
    let debug = format!("{resolution:?}");
    assert!(debug.contains("/resource"));
    let (file_system, path, uri) = resolution.into_parts();
    assert_eq!(
        file_system.properties().info().provider_id(),
        "resolution-provider"
    );
    assert_eq!(path.as_str(), "/resource");
    assert_eq!(uri.as_str(), "registry-test:///resource");
}

/// A facade that advertises URI schemes rejects a provider canonical URI from
/// another scheme before the resolution is published.
#[test]
fn test_resolution_rejects_unadvertised_canonical_uri_scheme() {
    let error = sync_resolution_with_scheme(
        "resolution-provider",
        "accepted",
        "rejected:///resource",
    )
    .expect_err("unadvertised scheme must fail");
    assert_eq!(error.kind(), FsErrorKind::InvalidUri);
}

/// A resolution must come from a filesystem that declares its URI scheme.
#[test]
fn test_resolution_rejects_canonical_uri_when_no_scheme_is_advertised() {
    let error = sync_resolution_with_path_properties(
        "resolution-provider",
        "/resource",
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
    )
    .expect_err("a scheme-less filesystem cannot publish a URI resolution");
    assert_eq!(error.kind(), FsErrorKind::InvalidUri);
}
