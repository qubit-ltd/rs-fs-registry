// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Regression and execution checks for shipped documentation.

use std::path::Path;

use crate::support::markdown_examples::check_documents;
use crate::support::markdown_examples::documentation_manifest;
use crate::support::markdown_examples::manifest;
use crate::support::markdown_examples::snippets;

/// Returns Cargo's major/minor release line for a package version.
fn release_line(version: &str) -> String {
    let mut components = version.split('.');
    let major = components
        .next()
        .expect("package version must contain a major component");
    let minor = components
        .next()
        .expect("package version must contain a minor component");
    assert!(
        components.next().is_some(),
        "package version must contain a patch component"
    );
    format!("{major}.{minor}")
}

/// User-facing installation commands must match the supported release lines.
#[test]
fn test_documentation_commands_follow_manifest() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let input = manifest(root);
    let registry_version = release_line(
        input["package"]["version"]
            .as_str()
            .expect("package version must be a string"),
    );
    let fs_version = input["dependencies"]["qubit-fs"]["version"]
        .as_str()
        .expect("qubit-fs version must be a string");
    let local_version = input["package"]["metadata"]["documentation"]["dependencies"]["qubit-fs-local"]["version"]
        .as_str()
        .expect("qubit-fs-local version must be a string");
    assert_eq!(fs_version, "0.4", "qubit-fs release line must remain explicit");
    assert_eq!(registry_version, "0.4", "registry release line must follow the package");
    assert_eq!(local_version, "0.6", "local provider release line must remain explicit");
    let sync_command = format!("cargo add qubit-fs@{fs_version} qubit-fs-registry@{registry_version}");
    let local_command = format!("cargo add qubit-fs-local@{local_version} --features registry");
    let async_command = format!("cargo add qubit-fs-registry@{registry_version} --features async");

    for relative in [
        "README.md",
        "README.zh_CN.md",
        "doc/user_guide.md",
        "doc/user_guide.zh_CN.md",
    ] {
        let source = std::fs::read_to_string(root.join(relative)).expect("documentation must be readable");
        assert!(
            source.contains(&sync_command),
            "{relative} must contain `{sync_command}`"
        );
        assert!(
            source.contains(&local_command),
            "{relative} must contain `{local_command}`"
        );
    }
    for relative in ["doc/user_guide.md", "doc/user_guide.zh_CN.md"] {
        let source = std::fs::read_to_string(root.join(relative)).expect("user guide must be readable");
        assert!(
            source.contains(&async_command),
            "{relative} must contain `{async_command}`"
        );
    }
}

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
