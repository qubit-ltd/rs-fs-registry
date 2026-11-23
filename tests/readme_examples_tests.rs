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
use crate::support::markdown_examples::DependencySource;
use crate::support::markdown_examples::ExternalMetadataOptions;
use crate::support::markdown_examples::GraphOptions;
use crate::support::markdown_examples::check_documents_for_source;
use crate::support::markdown_examples::check_graph;
use crate::support::markdown_examples::dependency;
use crate::support::markdown_examples::documentation_manifest;
use crate::support::markdown_examples::documentation_manifest_for_source;
use crate::support::markdown_examples::executable_artifacts;
use crate::support::markdown_examples::manifest;
use crate::support::markdown_examples::prepare_documents_for_external;
use crate::support::markdown_examples::snippets;
use crate::support::markdown_examples::validate_external_metadata;

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
fn installation_commands(_root: &Path, input: &toml::Value) -> (String, String, String) {
    let registry_version = release_line(
        input["package"]["version"]
            .as_str()
            .expect("package version must be a string"),
    );
    let filesystem = dependency(&input["dependencies"]["qubit-fs"]);
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

fn async_appendix_requirements(input: &toml::Value) -> Vec<String> {
    let version = |name: &str| {
        dependency(&input["dependencies"][name])["version"]
            .as_str()
            .expect("dependency version must be a string")
            .to_owned()
    };
    let futures_version = input["package"]["metadata"]["documentation"]["dependencies"]["futures"]["version"]
        .as_str()
        .expect("futures documentation version must be a string")
        .to_owned();
    vec![
        format!("`qubit-fs {}`", version("qubit-fs")),
        format!("`qubit-spi {}`", version("qubit-spi")),
        format!(
            "`qubit-fs-registry {}`",
            input["package"]["version"].as_str().expect("package version")
        ),
        "`async`".to_owned(),
        format!("`futures {futures_version}`"),
    ]
}

fn validate_async_appendix_contract(source: &str, expected: &[String], document: &str) -> Result<(), String> {
    for requirement in expected {
        if !source.contains(requirement) {
            return Err(format!("{document}: missing candidate requirement {requirement}"));
        }
    }
    let command = "cargo test --locked --all-features --test integration_tests readme_examples_tests::test_shipped_markdown_rust_examples_run -- --nocapture";
    if !source.contains(command) {
        return Err(format!("{document}: missing source-workspace validation command"));
    }
    let source_only = if document.ends_with(".zh_CN.md") {
        "源码工作区运行"
    } else {
        "source-workspace-only"
    };
    if !source.contains(source_only) {
        return Err(format!(
            "{document}: must state that this candidate is source-workspace-only"
        ));
    }
    Ok(())
}

#[test]
fn test_async_example_installation_dependencies() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let input = manifest(root);
    let expected = async_appendix_requirements(&input);
    for relative in [
        "doc/async_resolution_example.md",
        "doc/async_resolution_example.zh_CN.md",
    ] {
        let source = std::fs::read_to_string(root.join(relative)).expect("appendix must be readable");
        validate_async_appendix_contract(&source, &expected, relative).unwrap();
    }

    let source = std::fs::read_to_string(root.join("doc/async_resolution_example.md")).expect("appendix");
    for stale in [
        "`qubit-fs ^0.4`",
        "`qubit-fs-registry 0.8.3`",
        "`async`",
        "test_shipped_markdown_rust_examples_run",
        "source-workspace-only",
    ] {
        let changed = source.replace(stale, "removed contract");
        assert!(
            validate_async_appendix_contract(&changed, &expected, "synthetic.md").is_err(),
            "appendix must reject a missing version, feature, source note, or command: {stale}"
        );
    }
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
    let spi_dependency = dependency(&input["dependencies"]["qubit-spi"]);
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

/// Dependency requirements must follow Cargo metadata.
#[test]
fn test_documentation_versions_follow_manifest() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let input = manifest(root);
    let output = documentation_manifest(root, &input, false, false, false);
    assert_eq!(
        output["dependencies"]["qubit-fs"]["version"],
        input["dependencies"]["qubit-fs"]["version"]
    );
    assert!(output["dependencies"].get("qubit-fs-local").is_none());
    assert_eq!(
        PathBuf::from(output["dependencies"]["qubit-fs"]["path"].as_str().unwrap()),
        root.join("../rs-fs").canonicalize().unwrap()
    );
    assert!(output["dependencies"].get("futures").is_none());
    assert_eq!(
        output["dependencies"]["qubit-fs-registry"]["features"]
            .as_array()
            .expect("registry feature list must be an array")
            .len(),
        0
    );
    let asynchronous = documentation_manifest(root, &input, true, false, false);
    assert_eq!(
        asynchronous["dependencies"]["qubit-fs-registry"]["features"][0].as_str(),
        Some("async")
    );
    assert!(asynchronous["dependencies"].get("futures").is_some());
}

#[test]
fn test_documentation_manifest_uses_sibling_sources_and_manifest_features() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let input = manifest(root);
    for (asynchronous, local, inventory, expected) in [
        (false, false, false, Vec::<&str>::new()),
        (true, false, false, vec!["async"]),
        (false, false, true, vec!["inventory"]),
        (true, false, true, vec!["async", "inventory"]),
    ] {
        let output = documentation_manifest(root, &input, asynchronous, local, inventory);
        for (name, relative) in [("qubit-fs", "../rs-fs"), ("qubit-fs-registry", ".")] {
            assert_eq!(
                PathBuf::from(output["dependencies"][name]["path"].as_str().unwrap()),
                root.join(relative).canonicalize().unwrap(),
                "{name} must resolve to the intended sibling source"
            );
        }
        let tested_package_version = format!("={}", input["package"]["version"].as_str().expect("package version"));
        assert_eq!(
            output["dependencies"]["qubit-fs-registry"]["version"].as_str(),
            Some(tested_package_version.as_str()),
            "the local registry path must pin the tested package revision"
        );
        if local {
            assert_eq!(
                PathBuf::from(output["dependencies"]["qubit-fs-local"]["path"].as_str().unwrap()),
                root.join("../rs-fs-local").canonicalize().unwrap(),
            );
        }
        assert!(output["dependencies"]["qubit-spi"].get("path").is_none());
        assert!(output.get("patch").is_none(), "examples must not use Cargo patches");
        let mut actual = output["dependencies"]["qubit-fs-registry"]["features"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect::<Vec<_>>();
        actual.sort_unstable();
        assert_eq!(actual, expected);
    }
}

#[test]
fn test_candidate_and_published_manifests_do_not_probe_sibling_sources() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let input = manifest(root);
    let candidate = documentation_manifest_for_source(
        root,
        &input,
        false,
        false,
        false,
        DependencySource::CandidateArchive,
        None,
    );
    assert!(candidate["dependencies"]["qubit-fs"].get("path").is_none());
    assert!(candidate["dependencies"]["qubit-fs-registry"].get("path").is_some());

    let published = documentation_manifest_for_source(
        root,
        &input,
        false,
        true,
        false,
        DependencySource::PublishedConsumer,
        Some("0.8.2"),
    );
    assert!(published["dependencies"]["qubit-fs"].get("path").is_none());
    assert!(published["dependencies"]["qubit-fs-registry"].get("path").is_none());
    assert_eq!(
        published["dependencies"]["qubit-fs-registry"]["version"].as_str(),
        Some("=0.8.2")
    );
    assert!(published["dependencies"]["qubit-fs-local"].get("path").is_none());
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
    let default = documentation_manifest(root, &input, false, false, false);
    assert!(default["dependencies"].get("qubit-fs-local").is_none());
    assert_eq!(
        default["dependencies"]["qubit-fs-registry"]["features"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    let asynchronous = documentation_manifest(root, &input, true, false, false);
    assert_eq!(
        asynchronous["dependencies"]["qubit-fs-registry"]["features"][0].as_str(),
        Some("async")
    );
    assert!(asynchronous["dependencies"].get("futures").is_some());
    let local = documentation_manifest(root, &input, false, true, false);
    assert_eq!(
        local["dependencies"]["qubit-fs-registry"]["features"][0].as_str(),
        Some("inventory")
    );
    assert!(local["dependencies"].get("qubit-fs-local").is_some());
    assert_eq!(
        local["dependencies"]["qubit-fs-local"]["features"][0].as_str(),
        Some("registry")
    );
    let inventory = documentation_manifest(root, &input, false, false, true);
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
    check_graph(&base(&[]), "qubit-fs-registry", root, GraphOptions::default());
    let alternate_registry = serde_json::json!({
        "packages": [
            {"name":"qubit-fs-registry", "id":registry_id, "manifest_path":root.join("Cargo.toml").to_str().unwrap(), "source":null},
            {"name":"qubit-fs", "id":"qubit-fs 0.3.0 (registry+https://example.invalid/index)", "manifest_path":"/registry/qubit-fs/Cargo.toml", "source":"registry+https://example.invalid/index"}
        ],
        "resolve": {"nodes": [{"id":registry_id, "features":[], "dependencies":[]}]}
    });
    assert!(
        std::panic::catch_unwind(|| check_graph(
            &alternate_registry,
            "qubit-fs-registry",
            root,
            GraphOptions {
                published: true,
                registry: true,
                ..GraphOptions::default()
            }
        ))
        .is_err()
    );
    let registry_manifest = tempfile::tempdir().expect("registry package directory");
    let registry_manifest_path = registry_manifest.path().join("Cargo.toml");
    std::fs::write(
        &registry_manifest_path,
        "[package]\nname = 'qubit-fs-registry'\nversion = '0.8.2'\n",
    )
    .unwrap();
    let registry_example = serde_json::json!({
        "packages": [{
            "name":"qubit-fs-registry",
            "id":"qubit-fs-registry 0.8.2 (registry+https://github.com/rust-lang/crates.io-index)",
            "manifest_path":registry_manifest_path.to_str().unwrap(),
            "source":"registry+https://github.com/rust-lang/crates.io-index"
        }],
        "resolve": {"nodes": [{
            "id":"qubit-fs-registry 0.8.2 (registry+https://github.com/rust-lang/crates.io-index)",
            "features":[],
            "dependencies":[]
        }]}
    });
    check_graph(
        &registry_example,
        "qubit-fs-registry",
        root,
        GraphOptions {
            published: true,
            registry: true,
            ..GraphOptions::default()
        },
    );
    assert!(
        std::panic::catch_unwind(|| check_graph(
            &base(&["inventory"]),
            "qubit-fs-registry",
            root,
            GraphOptions::default()
        ))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| check_graph(&base(&["async"]), "qubit-fs-registry", root, GraphOptions::default()))
            .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| check_graph(
            &base(&[]),
            "qubit-fs-registry",
            root,
            GraphOptions {
                asynchronous: true,
                ..GraphOptions::default()
            }
        ))
        .is_err()
    );
    check_graph(
        &base(&["async", "inventory"]),
        "qubit-fs-registry",
        root,
        GraphOptions {
            asynchronous: true,
            inventory: true,
            ..GraphOptions::default()
        },
    );
    let local = serde_json::json!({
        "packages": [
            {"name":"qubit-fs-registry", "id":registry_id, "manifest_path":root.join("Cargo.toml").to_str().unwrap(), "source":null},
            {"name":"qubit-fs-local", "id":"qubit-fs-local 0.11.0 (registry+https://github.com/rust-lang/crates.io-index)", "manifest_path":"/local/Cargo.toml", "source":"registry+https://github.com/rust-lang/crates.io-index"},
            {"name":"qubit-local-files", "id":"qubit-local-files 0.6.0 (registry+https://github.com/rust-lang/crates.io-index)", "manifest_path":"/local-files/Cargo.toml", "source":"registry+https://github.com/rust-lang/crates.io-index"}
        ],
        "resolve": {"nodes": [
            {"id":registry_id, "features":["inventory"], "dependencies":[]},
            {"id":"qubit-fs-local 0.11.0 (registry+https://github.com/rust-lang/crates.io-index)", "features":["registry"], "dependencies":[registry_id, "qubit-local-files 0.6.0 (registry+https://github.com/rust-lang/crates.io-index)"]},
            {"id":"qubit-local-files 0.6.0 (registry+https://github.com/rust-lang/crates.io-index)", "features":[], "dependencies":[]}
        ]}
    });
    check_graph(
        &local,
        "qubit-fs-registry",
        root,
        GraphOptions {
            local: true,
            ..GraphOptions::default()
        },
    );
    let mut missing_registry_edge = local.clone();
    missing_registry_edge["resolve"]["nodes"][1]["dependencies"][0] = serde_json::json!("wrong-registry-id");
    assert!(
        std::panic::catch_unwind(|| check_graph(
            &missing_registry_edge,
            "qubit-fs-registry",
            root,
            GraphOptions {
                local: true,
                ..GraphOptions::default()
            }
        ))
        .is_err()
    );
    let mut missing_local_registry_feature = local.clone();
    missing_local_registry_feature["resolve"]["nodes"][1]["features"] = serde_json::json!([]);
    assert!(
        std::panic::catch_unwind(|| check_graph(
            &missing_local_registry_feature,
            "qubit-fs-registry",
            root,
            GraphOptions {
                local: true,
                ..GraphOptions::default()
            }
        ))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| check_graph(&local, "qubit-fs-registry", root, GraphOptions::default())).is_err()
    );
    let mut duplicate_local_files = local.clone();
    duplicate_local_files["packages"]
        .as_array_mut()
        .unwrap()
        .push(local["packages"][2].clone());
    assert!(
        std::panic::catch_unwind(|| check_graph(
            &duplicate_local_files,
            "qubit-fs-registry",
            root,
            GraphOptions {
                local: true,
                ..GraphOptions::default()
            }
        ))
        .is_err()
    );
    let missing_local_files = serde_json::json!({
        "packages": local["packages"].as_array().unwrap()[..2].to_vec(),
        "resolve": {"nodes": local["resolve"]["nodes"].as_array().unwrap()[..2].to_vec()}
    });
    assert!(
        std::panic::catch_unwind(|| check_graph(
            &missing_local_files,
            "qubit-fs-registry",
            root,
            GraphOptions {
                local: true,
                ..GraphOptions::default()
            }
        ))
        .is_err()
    );
    let mut alternate_local_files = local.clone();
    alternate_local_files["packages"][2]["source"] = serde_json::json!("registry+https://example.invalid/index");
    alternate_local_files["packages"][2]["id"] =
        serde_json::json!("qubit-local-files 0.6.0 (registry+https://example.invalid/index)");
    alternate_local_files["resolve"]["nodes"][2]["id"] = alternate_local_files["packages"][2]["id"].clone();
    alternate_local_files["resolve"]["nodes"][1]["dependencies"][1] =
        alternate_local_files["packages"][2]["id"].clone();
    assert!(
        std::panic::catch_unwind(|| check_graph(
            &alternate_local_files,
            "qubit-fs-registry",
            root,
            GraphOptions {
                local: true,
                ..GraphOptions::default()
            }
        ))
        .is_err()
    );
    let sibling = tempfile::tempdir().expect("local-files sibling");
    std::fs::write(
        sibling.path().join("Cargo.toml"),
        "[package]\nname='qubit-local-files'\nversion='0.6.0'\n",
    )
    .unwrap();
    let mut source_local_files = local.clone();
    let sibling_id = "qubit-local-files 0.6.0 (path+file:///local-files)";
    source_local_files["packages"][2]["source"] = serde_json::Value::Null;
    source_local_files["packages"][2]["id"] = serde_json::json!(sibling_id);
    source_local_files["packages"][2]["manifest_path"] =
        serde_json::json!(sibling.path().join("Cargo.toml").to_str().unwrap());
    source_local_files["resolve"]["nodes"][2]["id"] = serde_json::json!(sibling_id);
    source_local_files["resolve"]["nodes"][1]["dependencies"][1] = serde_json::json!(sibling_id);
    check_graph(
        &source_local_files,
        "qubit-fs-registry",
        root,
        GraphOptions {
            local: true,
            qubit_local_files_override: Some(sibling.path()),
            ..GraphOptions::default()
        },
    );
    source_local_files["packages"][2]["manifest_path"] = serde_json::json!("/wrong/Cargo.toml");
    assert!(
        std::panic::catch_unwind(|| check_graph(
            &source_local_files,
            "qubit-fs-registry",
            root,
            GraphOptions {
                local: true,
                qubit_local_files_override: Some(sibling.path()),
                ..GraphOptions::default()
            }
        ))
        .is_err()
    );
    let duplicate = serde_json::json!({
        "packages": [
            {"name":"qubit-fs-registry", "id":registry_id, "manifest_path":root.join("Cargo.toml").to_str().unwrap(), "source":null},
            {"name":"qubit-fs-registry", "id":"registry duplicate", "manifest_path":root.join("Cargo.toml").to_str().unwrap(), "source":null}
        ],
        "resolve": {"nodes": [{"id":registry_id, "features":[], "dependencies":[]}]}
    });
    assert!(
        std::panic::catch_unwind(|| check_graph(&duplicate, "qubit-fs-registry", root, GraphOptions::default()))
            .is_err()
    );
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
    documentation_manifest(root, &input, false, false, false);
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
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = std::env::var("REGISTRY_DOCS_SOURCE")
        .ok()
        .map(|value| DependencySource::parse(&value).unwrap_or_else(|error| panic!("{error}")))
        .unwrap_or(DependencySource::SiblingSource);
    let version = std::env::var("REGISTRY_PUBLISHED_VERSION").ok();
    check_documents_for_source(root, source, version.as_deref());
}

/// Python-supervised publication steps use this entry only for Rust manifest,
/// graph, and artifact validation. It never starts Cargo itself.
#[test]
fn test_registry_docs_helper_step() {
    let Ok(action) = std::env::var("REGISTRY_DOCS_HELPER_ACTION") else {
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output_path = PathBuf::from(std::env::var_os("REGISTRY_DOCS_HELPER_OUTPUT").expect("helper output path"));
    let result = match action.as_str() {
        "prepare" => {
            let source = std::env::var("REGISTRY_DOCS_SOURCE")
                .ok()
                .map(|value| DependencySource::parse(&value).unwrap_or_else(|error| panic!("{error}")))
                .unwrap_or(DependencySource::SiblingSource);
            let workspace = PathBuf::from(std::env::var_os("REGISTRY_DOCS_WORKSPACE").expect("workspace path"));
            let version = std::env::var("REGISTRY_PUBLISHED_VERSION").ok();
            prepare_documents_for_external(root, source, version.as_deref(), &workspace)
        }
        "validate" => {
            let project = PathBuf::from(std::env::var_os("REGISTRY_DOCS_PROJECT").expect("project path"));
            let metadata_path = PathBuf::from(std::env::var_os("REGISTRY_DOCS_HELPER_INPUT").expect("metadata path"));
            let metadata: serde_json::Value =
                serde_json::from_slice(&std::fs::read(metadata_path).expect("metadata file"))
                    .expect("Cargo metadata JSON");
            let source = DependencySource::parse(&std::env::var("REGISTRY_DOCS_SOURCE").expect("source mode"))
                .unwrap_or_else(|error| panic!("{error}"));
            let package = manifest(root)["package"]["name"]
                .as_str()
                .expect("package name")
                .to_owned();
            let parse_feature = |name: &str| std::env::var(name).expect("feature value") == "1";
            validate_external_metadata(
                &metadata,
                &package,
                root,
                ExternalMetadataOptions {
                    source,
                    asynchronous: parse_feature("REGISTRY_DOCS_ASYNC"),
                    local: parse_feature("REGISTRY_DOCS_LOCAL"),
                    inventory: parse_feature("REGISTRY_DOCS_INVENTORY"),
                    example_manifest: &project.join("Cargo.toml"),
                },
            )
        }
        "artifacts" => {
            let input_path = PathBuf::from(std::env::var_os("REGISTRY_DOCS_HELPER_INPUT").expect("build output path"));
            let build_output = std::fs::read(input_path).expect("Cargo build message output");
            let artifacts = executable_artifacts(&build_output).unwrap_or_else(|error| panic!("{error}"));
            serde_json::to_value(artifacts.into_iter().collect::<std::collections::BTreeMap<_, _>>())
                .expect("artifact JSON")
        }
        _ => panic!("unknown publication helper action: {action}"),
    };
    std::fs::write(
        output_path,
        serde_json::to_vec_pretty(&result).expect("serialize helper output"),
    )
    .expect("write helper output");
}

#[test]
fn test_documentation_dependency_source_parser_rejects_unknown_input() {
    assert_eq!(DependencySource::parse("source"), Ok(DependencySource::SiblingSource));
    assert_eq!(
        DependencySource::parse("candidate"),
        Ok(DependencySource::CandidateArchive)
    );
    assert_eq!(
        DependencySource::parse("published"),
        Ok(DependencySource::PublishedConsumer)
    );
    assert!(
        DependencySource::parse("siblings")
            .unwrap_err()
            .contains("unknown documentation dependency source")
    );
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
