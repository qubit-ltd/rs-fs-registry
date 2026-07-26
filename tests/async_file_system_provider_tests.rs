// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::{
    error::Error,
    sync::Arc,
};

use qubit_fs::{
    FsError,
    FsErrorKind,
    FsOperation,
};
use qubit_fs_registry::map_async_provider_error;
use qubit_spi::error::ProviderErrorKind;

/// Verifies async provider failures retain filesystem classifications.
#[test]
fn async_provider_error_mapping_preserves_classification_and_source() {
    let error = map_async_provider_error(FsError::new(
        FsErrorKind::ProviderUnavailable,
        FsOperation::Provider,
        "test provider failure",
    ));

    assert_eq!(ProviderErrorKind::Unavailable, error.kind());
    let mut source = Error::source(&error);
    let mut retains_filesystem_error = false;
    while let Some(cause) = source {
        retains_filesystem_error |= cause.downcast_ref::<FsError>().is_some();
        retains_filesystem_error |= cause
            .downcast_ref::<Arc<dyn Error + Send + Sync>>()
            .and_then(|shared| shared.as_ref().downcast_ref::<FsError>())
            .is_some();
        source = cause.source();
    }
    assert!(
        retains_filesystem_error,
        "the SPI error should retain its filesystem source",
    );
}
