// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Regression and execution checks for shipped documentation.

use std::path::Path;
use std::path::PathBuf;

use crate::support::markdown_examples::CHECKED_DOCUMENTS;
use crate::support::markdown_examples::check_documents;
use crate::support::markdown_examples::check_graph;
use crate::support::markdown_examples::dependency;
use crate::support::markdown_examples::documentation_manifest;
use crate::support::markdown_examples::executable_artifacts;
use crate::support::markdown_examples::local_provider_manifest;
use crate::support::markdown_examples::local_provider_uses_registry;
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

/// Builds user-facing installation commands from the selected manifest only.
fn installation_commands(root: &Path, input: &toml::Value) -> (String, String, String) {
    let registry_version = release_line(
        input["package"]["version"]
            .as_str()
            .expect("package version must be a string"),
    );
    let filesystem = dependency(root, &input["dependencies"]["qubit-fs"], true);
    let fs_version = filesystem["version"]
        .as_str()
        .expect("qubit-fs version must be a string")
        .trim_start_matches('^');
    let fs_version = fs_version.split('.').take(2).collect::<Vec<_>>().join(".");
    let documentation_dependencies = &input["package"]["metadata"]["documentation"]["dependencies"];
    let local_version = documentation_dependencies["qubit-fs-local"]["version"]
        .as_str()
        .expect("qubit-fs-local version must be a string");
    let sync_command = format!("cargo add qubit-fs@{fs_version} qubit-fs-registry@{registry_version}");
    let local_command = format!("cargo add qubit-fs-local@{local_version} --features registry");
    let async_command = format!("cargo add qubit-fs-registry@{registry_version} --features async");

    (sync_command, local_command, async_command)
}

/// Future release lines change documented installation commands automatically.
#[test]
fn test_installation_commands_follow_changed_manifest_versions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut input = manifest(root);
    input["package"]["version"] = toml::Value::String("0.99.1".into());
    input["dependencies"]["qubit-fs"]
        .as_table_mut()
        .expect("filesystem dependency table")
        .insert("version".into(), toml::Value::String("0.98".into()));
    input["package"]["metadata"]["documentation"]["dependencies"]["qubit-fs-local"]["version"] =
        toml::Value::String("0.97".into());
    assert_eq!(
        installation_commands(root, &input),
        (
            "cargo add qubit-fs@0.98 qubit-fs-registry@0.99".into(),
            "cargo add qubit-fs-local@0.97 --features registry".into(),
            "cargo add qubit-fs-registry@0.99 --features async".into(),
        )
    );
}

/// User-facing installation commands must match the supported release lines.
#[test]
fn test_documentation_commands_follow_manifest() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let input = manifest(root);
    let (sync_command, local_command, async_command) = installation_commands(root, &input);

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

/// Both migration guides keep every SPI release declaration aligned with Cargo.
#[test]
fn test_migration_spi_versions_follow_manifest() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let input = manifest(root);
    let spi_dependency = dependency(root, &input["dependencies"]["qubit-spi"], true);
    let expected = spi_dependency["version"]
        .as_str()
        .expect("SPI dependency version must be a string")
        .trim_start_matches('^');
    for relative in [
        "doc/registry_contract_migration.md",
        "doc/registry_contract_migration.zh_CN.md",
    ] {
        let source = std::fs::read_to_string(root.join(relative)).expect("migration guide must be readable");
        let declarations: Vec<_> = source.split("`qubit-spi` ").skip(1).collect();
        assert_eq!(declarations.len(), 2, "{relative}: expected two SPI declarations");
        for declaration in declarations {
            let version = declaration
                .split(|c: char| !c.is_ascii_digit() && c != '.')
                .next()
                .expect("SPI release declaration")
                .trim_end_matches('.');
            assert_eq!(version, expected, "{relative}: stale SPI release declaration");
        }
    }
}

/// Dependency requirements must follow Cargo metadata, including without
/// siblings.
#[test]
fn test_documentation_versions_follow_manifest() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let input = manifest(root);
    let output = documentation_manifest(root, &input, true, false, false, false);
    assert_eq!(
        output["dependencies"]["qubit-fs"]["version"],
        input["dependencies"]["qubit-fs"]["version"]
    );
    assert!(output["dependencies"].get("qubit-fs-local").is_none());
    assert!(output["dependencies"]["qubit-fs"].get("path").is_none());
    assert!(output["dependencies"].get("futures").is_none());
    assert_eq!(
        output["dependencies"]["qubit-fs-registry"]["features"]
            .as_array()
            .expect("registry feature list must be an array")
            .len(),
        0
    );
    let asynchronous = documentation_manifest(root, &input, true, true, false, false);
    assert_eq!(
        asynchronous["dependencies"]["qubit-fs-registry"]["features"][0].as_str(),
        Some("async")
    );
    assert!(asynchronous["dependencies"].get("futures").is_some());
}

/// A sibling provider that still points at another registry worktree is not
/// patched into the isolated example package.
#[test]
fn test_local_provider_patch_requires_the_tested_registry_path() {
    let workspace = tempfile::tempdir().expect("temporary sibling fixture");
    let current = workspace.path().join("current-registry");
    let previous = workspace.path().join("rs-fs-registry");
    let local = workspace.path().join("rs-fs-local");
    std::fs::create_dir_all(&current).expect("current registry directory");
    std::fs::create_dir_all(&previous).expect("previous registry directory");
    std::fs::create_dir_all(&local).expect("local provider directory");
    std::fs::write(
        current.join("Cargo.toml"),
        "[package]\nname='qubit-fs-registry'\nversion='0.8.0'\n",
    )
    .expect("current registry manifest");
    std::fs::write(
        previous.join("Cargo.toml"),
        "[package]\nname='qubit-fs-registry'\nversion='0.8.0'\n",
    )
    .expect("previous registry manifest");
    std::fs::write(
        local.join("Cargo.toml"),
        "[package]\nname='qubit-fs-local'\nversion='0.11.0'\n[dependencies]\nqubit-fs-registry={path='../rs-fs-registry',version='0.8'}\n",
    )
    .expect("local provider manifest");

    assert!(!local_provider_uses_registry(&local, &current));
    assert!(local_provider_uses_registry(&local, &previous));
}

/// A local-provider shim aligns its registry and sibling package paths with
/// the isolated example manifest while compiling the real provider source.
#[test]
fn test_local_provider_shim_uses_current_registry_and_sibling_aliases() {
    let workspace = tempfile::tempdir().expect("temporary shim fixture");
    let siblings = workspace.path().join("siblings");
    let current = siblings.join("codex-registry");
    let local = workspace.path().join("original/rs-fs-local");
    let original_registry = workspace.path().join("original/rs-fs-registry");
    let original_fs = workspace.path().join("original/rs-fs");
    let sibling_fs = siblings.join("rs-fs");
    for directory in [&current, &local, &original_registry, &original_fs, &sibling_fs] {
        std::fs::create_dir_all(directory).expect("fixture directory");
    }
    for directory in [&current, &original_registry] {
        std::fs::write(
            directory.join("Cargo.toml"),
            "[package]\nname='qubit-fs-registry'\nversion='0.8.0'\n",
        )
        .expect("registry fixture manifest");
    }
    for directory in [&original_fs, &sibling_fs] {
        std::fs::write(
            directory.join("Cargo.toml"),
            "[package]\nname='qubit-fs'\nversion='0.3.0'\n",
        )
        .expect("filesystem fixture manifest");
    }
    std::fs::create_dir_all(local.join("src")).expect("provider source directory");
    std::fs::write(local.join("src/lib.rs"), "pub fn example() {}\n").expect("provider source");
    std::fs::write(
        local.join("Cargo.toml"),
        "[package]\nname='qubit-fs-local'\nversion='0.11.0'\n[lib]\n[dependencies]\nqubit-fs={path='../rs-fs',version='0.3'}\nqubit-fs-registry={path='../rs-fs-registry',version='0.8'}\n",
    )
    .expect("provider manifest");

    let shim = local_provider_manifest(&local, &current, &workspace.path().join("shim"));
    let manifest: toml::Value = toml::from_str(&std::fs::read_to_string(shim).unwrap()).unwrap();
    assert_eq!(
        manifest["dependencies"]["qubit-fs-registry"]["path"].as_str(),
        Some(current.canonicalize().unwrap().to_str().unwrap())
    );
    assert_eq!(
        manifest["dependencies"]["qubit-fs"]["path"].as_str(),
        Some(sibling_fs.to_str().unwrap())
    );
    assert_eq!(
        manifest["lib"]["path"].as_str(),
        Some(local.join("src/lib.rs").canonicalize().unwrap().to_str().unwrap())
    );
}

/// Example directives compose and each dependency group declares only its
/// needs.
#[test]
fn test_example_requirements_are_grouped_explicitly() {
    let parsed = snippets("<!-- registry-example: async -->\n<!-- registry-example: local -->\n<!-- registry-example: inventory -->\n```rust\nfn main() {}\n```\n```rust,no_run\nfn main() {}\n```").expect("combined requirements");
    assert_eq!(parsed.len(), 2);
    assert!(parsed[0].asynchronous && parsed[0].local && parsed[0].inventory && parsed[0].run);
    assert!(!parsed[1].asynchronous && !parsed[1].local && !parsed[1].inventory && !parsed[1].run);

    for invalid in [
        "<!-- registry-example: local -->\n<!-- registry-example: local -->\n```rust\nfn main() {}\n```",
        "<!-- registry-example: typo -->",
        "<!-- registry-example: inventory -->",
    ] {
        assert!(snippets(invalid).is_err(), "must reject {invalid:?}");
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let input = manifest(root);
    let default = documentation_manifest(root, &input, true, false, false, false);
    assert!(default["dependencies"].get("qubit-fs-local").is_none());
    assert_eq!(
        default["dependencies"]["qubit-fs-registry"]["features"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    let asynchronous = documentation_manifest(root, &input, true, true, false, false);
    assert_eq!(
        asynchronous["dependencies"]["qubit-fs-registry"]["features"][0].as_str(),
        Some("async")
    );
    assert!(asynchronous["dependencies"].get("futures").is_some());
    let local = documentation_manifest(root, &input, true, false, true, false);
    assert_eq!(
        local["dependencies"]["qubit-fs-registry"]["features"][0].as_str(),
        Some("inventory")
    );
    assert!(local["dependencies"].get("qubit-fs-local").is_some());
    assert_eq!(
        local["dependencies"]["qubit-fs-local"]["features"][0].as_str(),
        Some("registry")
    );
    let inventory = documentation_manifest(root, &input, true, false, false, true);
    assert_eq!(
        inventory["dependencies"]["qubit-fs-registry"]["features"][0].as_str(),
        Some("inventory")
    );
    assert!(inventory["dependencies"].get("qubit-fs-local").is_none());
}

/// Resolve graph checks verify the tested registry's real feature closure.
#[test]
fn test_graph_checks_registry_features() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let registry_id = "registry 0.8.0 (path+file:///registry)";
    let base = |features: &[&str]| {
        serde_json::json!({
            "packages": [{"name":"qubit-fs-registry", "id":registry_id, "manifest_path":root.join("Cargo.toml").to_str().unwrap(), "source":null}],
            "resolve": {"nodes": [{"id":registry_id, "features":features, "dependencies":[]}]}
        })
    };
    check_graph(&base(&[]), "qubit-fs-registry", root, false, false, false, false);
    assert!(
        std::panic::catch_unwind(|| check_graph(
            &base(&["inventory"]),
            "qubit-fs-registry",
            root,
            false,
            false,
            false,
            false
        ))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| check_graph(
            &base(&["async"]),
            "qubit-fs-registry",
            root,
            false,
            false,
            false,
            false
        ))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| check_graph(&base(&[]), "qubit-fs-registry", root, false, true, false, false))
            .is_err()
    );
    check_graph(
        &base(&["async", "inventory"]),
        "qubit-fs-registry",
        root,
        false,
        true,
        false,
        true,
    );
    let local = serde_json::json!({
        "packages": [
            {"name":"qubit-fs-registry", "id":registry_id, "manifest_path":root.join("Cargo.toml").to_str().unwrap(), "source":null},
            {"name":"qubit-fs-local", "id":"local 0.11.0 (registry+https://example)", "manifest_path":"/local/Cargo.toml", "source":"registry+https://example"}
        ],
        "resolve": {"nodes": [
            {"id":registry_id, "features":["inventory"], "dependencies":[]},
            {"id":"local 0.11.0 (registry+https://example)", "features":[], "dependencies":[registry_id]}
        ]}
    });
    check_graph(&local, "qubit-fs-registry", root, false, false, true, false);
    let duplicate = serde_json::json!({
        "packages": [
            {"name":"qubit-fs-registry", "id":registry_id, "manifest_path":root.join("Cargo.toml").to_str().unwrap(), "source":null},
            {"name":"qubit-fs-registry", "id":"registry duplicate", "manifest_path":root.join("Cargo.toml").to_str().unwrap(), "source":null}
        ],
        "resolve": {"nodes": [{"id":registry_id, "features":[], "dependencies":[]}]}
    });
    assert!(
        std::panic::catch_unwind(|| check_graph(&duplicate, "qubit-fs-registry", root, false, false, false, false))
            .is_err()
    );
}

/// A path-only workspace edge still renders the sibling's release requirement.
#[test]
fn test_path_only_filesystem_dependency_uses_sibling_release() {
    let original_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut input = manifest(original_root);
    let workspace = tempfile::tempdir().expect("temporary fixture");
    let registry = workspace.path().join("registry");
    let sibling = workspace.path().join("fixture-fs");
    std::fs::create_dir_all(&registry).expect("registry fixture");
    std::fs::create_dir_all(&sibling).expect("filesystem fixture");
    std::fs::write(
        sibling.join("Cargo.toml"),
        "[package]\nname = 'qubit-fs'\nversion = '0.98.7'\n",
    )
    .expect("sibling manifest");
    let dependency = input["dependencies"]["qubit-fs"]
        .as_table_mut()
        .expect("filesystem dependency table");
    dependency.remove("version");
    dependency.insert("path".into(), toml::Value::String("../fixture-fs".into()));
    let output = documentation_manifest(&registry, &input, false, false, false, false);

    assert_eq!(output["dependencies"]["qubit-fs"]["version"].as_str(), Some("0.98.7"));
    assert!(output["dependencies"]["qubit-fs"]["path"].as_str().is_some());
}

/// Missing metadata must fail explicitly instead of silently omitting
/// dependencies.
#[test]
#[should_panic(expected = "documentation dependency metadata is required")]
fn test_missing_documentation_metadata_is_rejected() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut input = manifest(root);
    input["package"]
        .as_table_mut()
        .expect("package metadata table")
        .remove("metadata");
    documentation_manifest(root, &input, true, false, false, false);
}

/// Invalid documents must fail rather than silently skip malformed examples.
#[test]
fn test_documentation_rejects_malformed_fences() {
    for source in [
        "```rust\nfn main() {}",
        "<!-- registry-example: typo -->",
        "<!-- registry-example: async -->",
        "<!-- registry-example: local -->\n<!-- registry-example: local -->\n```rust\nfn main() {}\n```",
        "<!-- registry-example: async -->\n<!-- registry-example: async -->\n```rust\nfn main() {}\n```",
        "<!-- registry-example: inventory -->\n<!-- registry-example: inventory -->\n```rust\nfn main() {}\n```",
        "<!-- registry-example: inventory -->",
        "<!-- registry-example: local -->\nnot a fence\n```rust\nfn main() {}\n```",
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

/// Reported executable paths retain target folders and platform spelling.
#[test]
fn test_cargo_executable_artifacts_keep_reported_paths() {
    for executable in [
        "/tmp/target/debug/document_0_0",
        "/tmp/target/x86_64-unknown-linux-gnu/debug/document_0_0",
        "C:/example target/示例/debug/document_0_0.exe",
    ] {
        let message = serde_json::json!({
            "reason": "compiler-artifact",
            "target": {"kind": ["bin"], "name": "document_0_0"},
            "executable": executable,
        });
        let paths = executable_artifacts(message.to_string().as_bytes()).unwrap();
        assert_eq!(paths["document_0_0"], PathBuf::from(executable));
    }
}

/// Irrelevant messages are skipped and ambiguous or invalid output is rejected.
#[test]
fn test_cargo_executable_artifacts_validate_messages() {
    let output = concat!(
        "\n",
        "{\"reason\":\"build-finished\",\"success\":true}\n",
        "{\"reason\":\"compiler-artifact\",\"target\":{\"kind\":[\"lib\"],\"name\":\"dep\"},\"executable\":null}\n"
    );
    assert!(executable_artifacts(output.as_bytes()).unwrap().is_empty());
    assert!(executable_artifacts(b"not-json").is_err());
    assert!(executable_artifacts(&[0xff]).is_err());
    let first = serde_json::json!({
        "reason": "compiler-artifact",
        "target": {"kind": ["bin"], "name": "document_0_0"},
        "executable": "/tmp/first",
    });
    let mut second = first.clone();
    second["executable"] = serde_json::json!("/tmp/second");
    assert!(executable_artifacts(format!("{first}\n{second}\n").as_bytes()).is_err());
    assert!(executable_artifacts(format!("{first}\n{first}\n").as_bytes()).is_ok());
    let mut missing_name = first.clone();
    missing_name["target"].as_object_mut().unwrap().remove("name");
    assert!(executable_artifacts(missing_name.to_string().as_bytes()).is_err());
    second["executable"] = serde_json::Value::Null;
    assert!(executable_artifacts(second.to_string().as_bytes()).unwrap().is_empty());
}

/// Each example is compiled and run with its documented minimum features.
#[test]
fn test_shipped_markdown_rust_examples_run() {
    check_documents(Path::new(env!("CARGO_MANIFEST_DIR")), cfg!(feature = "async"));
}

/// Both language appendices stay in the verifier and retain async execution.
#[test]
fn test_async_resolution_appendices_are_checked_as_async_examples() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for relative in [
        "doc/async_resolution_example.md",
        "doc/async_resolution_example.zh_CN.md",
    ] {
        assert!(CHECKED_DOCUMENTS.contains(&relative), "{relative} must be checked");
        let source = std::fs::read_to_string(root.join(relative)).expect("appendix must be readable");
        let examples = snippets(&source).expect("appendix examples must parse");
        assert!(
            examples.iter().any(|example| example.asynchronous && example.run),
            "{relative} must retain a runnable async example"
        );
    }
    for relative in ["doc/user_guide.md", "doc/user_guide.zh_CN.md"] {
        let source = std::fs::read_to_string(root.join(relative)).expect("guide must be readable");
        let examples = snippets(&source).expect("guide examples must parse");
        assert!(
            examples.iter().any(|example| example.asynchronous && !example.run),
            "{relative} must compile its short async example without running it"
        );
    }
}

/// Direct dependency paths retain the identity used by sibling transitive
/// edges.
#[cfg(unix)]
#[test]
fn test_documentation_manifest_preserves_sibling_symlinks() {
    use std::fs;
    use std::os::unix::fs::symlink;

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut input = manifest(root);
    let workspace = tempfile::tempdir().expect("temporary fixture");
    let target = workspace.path().join("source");
    let alias = workspace.path().join("linked-spi");
    fs::create_dir_all(&target).expect("SPI target directory");
    fs::write(
        target.join("Cargo.toml"),
        "[package]\nname = 'qubit-spi'\nversion = '0.12.0'\n",
    )
    .expect("SPI fixture manifest");
    symlink(&target, &alias).expect("SPI symlink");
    let spi_declaration = dependency(root, &input["dependencies"]["qubit-spi"], true);
    let spi_version = spi_declaration["version"].as_str().expect("SPI dependency version");
    let mut spi_dependency = toml::Table::new();
    spi_dependency.insert("version".into(), toml::Value::String(spi_version.into()));
    input["dependencies"]["qubit-spi"] = toml::Value::Table(spi_dependency);
    input["dependencies"]["qubit-spi"]
        .as_table_mut()
        .expect("SPI dependency table")
        .insert(
            "path".into(),
            toml::Value::String(alias.to_str().expect("UTF-8 fixture path").into()),
        );
    let output = documentation_manifest(root, &input, false, false, false, false);
    assert_eq!(output["dependencies"]["qubit-spi"]["path"].as_str(), alias.to_str());
    assert_ne!(alias, alias.canonicalize().expect("canonical SPI fixture path"));
}
