// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::FsErrorKind;

use crate::common::{
    async_resolution,
    async_resolution_with_scheme,
};

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
    assert_eq!(
        resolution.canonical_uri().as_str(),
        "registry-test:///resource"
    );
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

/// Async resolutions reject canonical URI schemes the facade does not
/// advertise.
#[test]
fn test_async_resolution_rejects_unadvertised_canonical_uri_scheme() {
    let error = async_resolution_with_scheme(
        "async-resolution-provider",
        "accepted",
        "rejected:///resource",
    )
    .expect_err("unadvertised scheme must fail");
    assert_eq!(error.kind(), FsErrorKind::InvalidUri);
}
