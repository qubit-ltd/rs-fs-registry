// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::FsUri;
use qubit_fs_registry::FileSystemConfig;

/// Verifies registry configuration remains outside the filesystem core.
#[test]
fn test_file_system_config_owns_provider_selection_context() {
    let uri = FsUri::parse("memory:/object").expect("the URI should parse");
    let config = FileSystemConfig::new(uri.clone());
    assert_eq!(&uri, config.uri());
    assert!(config.selection().is_none());
}
