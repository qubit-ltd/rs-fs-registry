// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Regression and execution checks for shipped documentation.

use std::path::Path;

use crate::support::markdown_examples::check_documents;
use crate::support::markdown_examples::documentation_manifest;
use crate::support::markdown_examples::manifest;
use crate::support::markdown_examples::snippets;

/// Dependency requirements must follow Cargo metadata, including without
/// siblings.
#[test]
fn test_documentation_versions_follow_manifest() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let input = manifest(root);
    let output = documentation_manifest(root, &input, true, false);
    assert_eq!(
        output["dependencies"]["qubit-fs"]["version"],
        input["dependencies"]["qubit-fs"]["version"]
    );
    assert_eq!(
        output["dependencies"]["qubit-fs-local"]["version"],
        input["package"]["metadata"]["documentation"]["dependencies"]["qubit-fs-local"]["version"]
    );
    assert!(output["dependencies"]["qubit-fs"].get("path").is_none());
    assert!(output["dependencies"]["qubit-fs-local"].get("path").is_none());
    assert!(output["dependencies"].get("futures").is_none());
    assert_eq!(
        output["dependencies"]["qubit-fs-registry"]["features"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    let asynchronous = documentation_manifest(root, &input, true, true);
    assert_eq!(
        asynchronous["dependencies"]["qubit-fs-registry"]["features"][0].as_str(),
        Some("async")
    );
    assert!(asynchronous["dependencies"].get("futures").is_some());
}

/// Missing metadata must fail explicitly instead of silently omitting
/// dependencies.
#[test]
#[should_panic(expected = "documentation dependency metadata is required")]
fn test_missing_documentation_metadata_is_rejected() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut input = manifest(root);
    input["package"].as_table_mut().unwrap().remove("metadata");
    documentation_manifest(root, &input, true, false);
}

/// Invalid documents must fail rather than silently skip malformed examples.
#[test]
fn test_documentation_rejects_malformed_fences() {
    for source in [
        "```rust\nfn main() {}",
        "<!-- registry-example: typo -->",
        "<!-- registry-example: async -->",
        "```rust,ignore\n```",
    ] {
        assert!(snippets(source).is_err(), "must reject {source:?}");
    }
    let result = snippets("```rust\nfn main() {}\n```\n<!-- registry-example: async -->\n```rust\nfn main() {}\n```")
        .expect("valid examples");
    assert_eq!(result.len(), 2);
    assert!(!result[0].asynchronous);
    assert!(result[1].asynchronous);
    assert!(result.iter().all(|s| s.run));
}

/// Each example is compiled and run with its documented minimum features.
#[test]
fn test_shipped_markdown_rust_examples_run() {
    check_documents(Path::new(env!("CARGO_MANIFEST_DIR")), cfg!(feature = "async"));
}
