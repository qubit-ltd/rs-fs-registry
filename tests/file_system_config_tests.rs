// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::{
    FsUri,
    UserMetadata,
};
use qubit_fs_registry::FileSystemConfig;
use qubit_spi::ProviderSelection;

#[test]
fn config_builder_preserves_validated_options_without_a_fallible_step() {
    let selection =
        ProviderSelection::named("mock").expect("selection should parse");
    let options = UserMetadata::new()
        .with("region", "test-1")
        .expect("metadata should accept a non-sensitive key");
    let config = FileSystemConfig::new(
        FsUri::parse("mock:///file.txt").expect("URI should parse"),
    )
    .with_selection(selection.clone())
    .with_options(options.clone());

    assert_eq!(Some(&selection), config.selection());
    assert_eq!(&options, config.options());
}
