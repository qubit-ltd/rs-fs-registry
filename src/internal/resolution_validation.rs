// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared validation for provider-decoded resolutions.

use qubit_fs::FsError;
use qubit_fs::Path;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::FileSystemProperties;
use qubit_fs::path::Uri;

/// Validates the path and canonical URI returned by a filesystem provider.
///
/// Path validation runs first so its error takes precedence when both the
/// path and URI are invalid. A canonical URI must use a scheme advertised by
/// the filesystem, and a filesystem with no advertised schemes is rejected.
///
/// # Parameters
///
/// - `properties`: The properties advertised by the filesystem.
/// - `path`: The provider-decoded path to validate.
/// - `canonical_uri`: The canonical URI returned by the provider.
pub(crate) fn validate_resolution(
    properties: &FileSystemProperties,
    path: &Path,
    canonical_uri: &Uri,
) -> Result<(), FsError> {
    properties.validate_path(path, FsOperation::ParsePath)?;
    if !properties
        .info()
        .schemes()
        .iter()
        .any(|scheme| scheme == canonical_uri.scheme())
    {
        return Err(FsError::new(
            FsErrorKind::InvalidUri,
            FsOperation::Provider,
            "canonical URI scheme is not supported by the filesystem",
        )
        .with_provider(properties.info().provider_id().to_owned()));
    }
    Ok(())
}
