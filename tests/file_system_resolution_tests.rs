// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::Uri;
#[test]
fn test_resolution_boundary_uses_secret_free_uri() {
    assert!(Uri::parse("s3://bucket/key").is_ok());
    assert!(Uri::parse("s3://user:password@bucket/key").is_err());
}
