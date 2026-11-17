// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Executes the shipped examples against manifest-owned dependency versions.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use serde_json::Value as Json;
use serde_json::from_slice;
use tempfile::TempDir;
use toml::Table;
use toml::Value;
use toml::from_str;
use toml::to_string;

use crate::support::process_runner::run_with_timeout;

pub const CHECKED_DOCUMENTS: &[&str] = &[
    "README.md",
    "README.zh_CN.md",
    "doc/user_guide.md",
    "doc/user_guide.zh_CN.md",
    "doc/async_resolution_example.md",
    "doc/async_resolution_example.zh_CN.md",
];

/// One complete Markdown program and its minimum feature requirement.
#[derive(Debug, PartialEq)]
pub struct Snippet {
    pub source: String,
    pub asynchronous: bool,
    pub local: bool,
    pub inventory: bool,
    pub run: bool,
}

/// Extracts complete Rust programs, rejecting malformed or unknown directives.
pub fn snippets(text: &str) -> Result<Vec<Snippet>, String> {
    let mut result = Vec::new();
    let mut fence: Option<(bool, bool)> = None;
    let mut asynchronous = false;
    let mut local = false;
    let mut inventory = false;
    let mut directive_pending = false;
    let mut source = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if fence.is_none() && trimmed.starts_with("<!-- registry-example:") {
            let flag = match trimmed {
                "<!-- registry-example: async -->" => &mut asynchronous,
                "<!-- registry-example: local -->" => &mut local,
                "<!-- registry-example: inventory -->" => &mut inventory,
                _ => return Err("unknown registry-example directive".into()),
            };
            if *flag {
                return Err("repeated registry-example directive".into());
            }
            *flag = true;
            directive_pending = true;
        } else if let Some(language) = trimmed.strip_prefix("```") {
            if let Some((rust, run)) = fence.take() {
                if !language.is_empty() {
                    return Err("nested or malformed code fence".into());
                }
                if rust {
                    result.push(Snippet {
                        source: std::mem::take(&mut source),
                        asynchronous,
                        local,
                        inventory,
                        run,
                    });
                }
                asynchronous = false;
                local = false;
                inventory = false;
                directive_pending = false;
            } else {
                let rust = matches!(language, "rust" | "rs" | "rust,no_run");
                if language.starts_with("rust") && !rust {
                    return Err("unsupported Rust fence".into());
                }
                if asynchronous && !rust {
                    return Err("async directive must precede a Rust example".into());
                }
                if (local || inventory) && !rust {
                    return Err("provider directive must precede a Rust example".into());
                }
                fence = Some((rust, language != "rust,no_run"));
                directive_pending = false;
            }
        } else if directive_pending && !trimmed.is_empty() {
            return Err("registry-example directives must be consecutive and immediately precede Rust code".into());
        } else if matches!(fence, Some((true, _))) {
            source.push_str(line);
            source.push('\n');
        }
    }
    if fence.is_some() || asynchronous || local || inventory {
        return Err("unterminated fence or unused registry-example directive".into());
    }
    Ok(result)
}

/// Reads the package manifest, failing explicitly on missing version metadata.
pub fn manifest(root: &Path) -> Value {
    let source = fs::read_to_string(root.join("Cargo.toml")).expect("read package manifest");
    from_str(&source).expect("parse package manifest")
}

/// Builds a minimal example manifest with sibling source dependencies for the
/// current filesystem, registry, and optional local provider.
pub fn documentation_manifest(root: &Path, input: &Value, asynchronous: bool, local: bool, inventory: bool) -> Value {
    let package = input["package"]["name"].as_str().expect("package name");
    let mut dependencies = Table::new();
    for name in ["qubit-fs", "qubit-fs-registry", "qubit-spi"] {
        if let Some(value) = input.get("dependencies").and_then(|deps| deps.get(name)) {
            let path = match name {
                "qubit-fs" => Some(root.join("../rs-fs")),
                "qubit-fs-registry" => Some(root.to_path_buf()),
                _ => None,
            };
            dependencies.insert(
                name.into(),
                path.map_or_else(|| dependency(value), |path| source_dependency(value, &path)),
            );
        }
    }
    let extra = input["package"]
        .get("metadata")
        .and_then(|m| m.get("documentation"))
        .and_then(|d| d.get("dependencies"));
    assert!(extra.is_some(), "documentation dependency metadata is required");
    if let Some(extra) = extra {
        for (name, value) in extra.as_table().expect("documentation dependencies table") {
            if (name == "qubit-fs-local" && !local)
                || (value.get("async-only").and_then(Value::as_bool) == Some(true) && !asynchronous)
            {
                continue;
            }
            if name == "qubit-fs-local" {
                dependencies.insert(name.clone(), source_dependency(value, &root.join("../rs-fs-local")));
            } else {
                dependencies.insert(name.clone(), dependency(value));
            }
        }
    }
    let mut current = Table::new();
    current.insert(
        "version".into(),
        Value::String(format!(
            "={}",
            input["package"]["version"].as_str().expect("package version")
        )),
    );
    current.insert(
        "path".into(),
        Value::String(
            root.canonicalize()
                .expect("tested source exists")
                .to_string_lossy()
                .into_owned(),
        ),
    );
    current.insert("default-features".into(), Value::Boolean(false));
    let mut features = Vec::new();
    if asynchronous {
        features.push(Value::String("async".into()));
    }
    if inventory || local {
        features.push(Value::String("inventory".into()));
    }
    current.insert("features".into(), Value::Array(features));
    dependencies.insert(package.into(), Value::Table(current));
    let mut output: Value = from_str("[package]\nname='filesystem-documentation-check'\nversion='0.0.0'\nedition='2024'\npublish=false\n[workspace]\n").expect("static manifest");
    output
        .as_table_mut()
        .expect("manifest table")
        .insert("dependencies".into(), Value::Table(dependencies));
    output
}

/// Retains the semver requirement and features while pointing an isolated
/// source example at a sibling package.
fn source_dependency(value: &Value, path: &Path) -> Value {
    let mut table = dependency(value).as_table().expect("dependency table").clone();
    let path = path.canonicalize().expect("sibling source dependency exists");
    table.insert("path".into(), Value::String(path.to_string_lossy().into_owned()));
    Value::Table(table)
}

/// Keeps declared version and feature requirements while removing Cargo path
/// hints.
pub(crate) fn dependency(value: &Value) -> Value {
    let mut table = match value {
        Value::String(version) => Table::from_iter([("version".into(), Value::String(version.clone()))]),
        Value::Table(table) => table.clone(),
        _ => panic!("dependency must declare a version"),
    };
    assert!(
        table.get("version").and_then(Value::as_str).is_some(),
        "registry dependencies need an explicit version"
    );
    table.remove("optional");
    table.remove("async-only");
    table.remove("path");
    Value::Table(table)
}

/// Rejects duplicate filesystem/SPI package identities and a downstream using
/// another registry.
#[derive(Clone, Copy, Debug, Default)]
pub struct GraphOptions<'a> {
    pub published: bool,
    pub registry: bool,
    pub asynchronous: bool,
    pub local: bool,
    pub inventory: bool,
    pub qubit_fs_override: Option<&'a Path>,
    pub qubit_fs_local_override: Option<&'a Path>,
    pub qubit_local_files_override: Option<&'a Path>,
    pub qubit_operation_override: Option<&'a Path>,
    pub example_manifest: Option<&'a Path>,
}

pub fn check_graph(metadata: &Json, package: &str, root: &Path, options: GraphOptions<'_>) {
    let mut ids = BTreeMap::new();
    for entry in metadata["packages"].as_array().expect("metadata packages") {
        let name = entry["name"].as_str().expect("package name");
        if options.published {
            let source = entry["source"].as_str();
            let manifest_path = Path::new(entry["manifest_path"].as_str().expect("manifest path"))
                .canonicalize()
                .expect("manifest exists");
            let expected_example_manifest = (name == "filesystem-documentation-check")
                .then(|| options.example_manifest.and_then(|path| path.canonicalize().ok()))
                .flatten();
            if name == package {
                if options.registry {
                    assert!(
                        source.is_some_and(|source| {
                            source == "registry+https://github.com/rust-lang/crates.io-index"
                                || source == "registry+https://index.crates.io/"
                        }),
                        "tested package dependency must use crates.io in a registry example graph"
                    );
                } else {
                    assert_eq!(source, None, "tested package must be the extracted archive");
                    assert_eq!(
                        manifest_path,
                        root.join("Cargo.toml").canonicalize().expect("tested manifest exists")
                    );
                }
            } else if name == "filesystem-documentation-check" {
                assert_eq!(source, None, "example consumer must be the generated manifest");
                assert_eq!(
                    Some(manifest_path),
                    expected_example_manifest,
                    "unexpected generated example consumer manifest"
                );
            } else if source.is_none() {
                assert!(
                    !options.registry,
                    "strict registry graph contains an unexpected local package: {name}"
                );
                let expected_path = match name {
                    "qubit-fs" => options.qubit_fs_override,
                    "qubit-fs-local" => options.qubit_fs_local_override,
                    "qubit-local-files" => options.qubit_local_files_override,
                    _ => None,
                }
                .and_then(|path| path.join("Cargo.toml").canonicalize().ok());
                assert_eq!(
                    Path::new(entry["manifest_path"].as_str().expect("manifest path"))
                        .canonicalize()
                        .ok(),
                    expected_path,
                    "unexpected local source in published dependency graph: {name}"
                );
            } else {
                let source_is_registry = source.is_some_and(|source| {
                    if options.registry {
                        source == "registry+https://github.com/rust-lang/crates.io-index"
                            || source == "registry+https://index.crates.io/"
                    } else {
                        source.starts_with("registry+")
                    }
                });
                assert!(
                    source_is_registry,
                    "published dependency uses an unexpected registry source: {name}"
                );
            }
        }
        if !options.published
            && let Some(example_manifest) = options.example_manifest
        {
            let expected_path = if name == "filesystem-documentation-check" {
                example_manifest.canonicalize().ok()
            } else {
                match name {
                    name if name == package => Some(root),
                    "qubit-fs" => options.qubit_fs_override,
                    "qubit-fs-local" => options.qubit_fs_local_override,
                    "qubit-local-files" => options.qubit_local_files_override,
                    "qubit-operation" => options.qubit_operation_override,
                    _ => None,
                }
                .and_then(|path| path.join("Cargo.toml").canonicalize().ok())
            };
            if let Some(expected_path) = expected_path {
                assert_eq!(
                    entry["source"],
                    Json::Null,
                    "source integration dependency must use its sibling path: {name}"
                );
                assert_eq!(
                    Path::new(entry["manifest_path"].as_str().expect("manifest path"))
                        .canonicalize()
                        .ok(),
                    Some(expected_path),
                    "unexpected source integration path: {name}"
                );
            } else {
                assert!(
                    entry["source"]
                        .as_str()
                        .is_some_and(|source| source.starts_with("registry+")),
                    "source integration dependency must resolve from a registry or declared sibling: {name}"
                );
            }
        }
        if matches!(
            name,
            "qubit-fs" | "qubit-fs-registry" | "qubit-spi" | "qubit-fs-local" | "qubit-local-files"
        ) {
            assert!(
                ids.insert(name, entry["id"].as_str().expect("package ID")).is_none(),
                "duplicate package identity: {name}"
            );
            if name == package && !options.published {
                assert_eq!(
                    Path::new(entry["manifest_path"].as_str().expect("manifest path"))
                        .canonicalize()
                        .expect("manifest exists"),
                    root.join("Cargo.toml").canonicalize().expect("tested manifest exists")
                );
            }
        }
    }
    let registry_id = ids.get(package).expect("tested package identity");
    let registry_node = metadata["resolve"]["nodes"]
        .as_array()
        .expect("resolved nodes")
        .iter()
        .find(|node| node["id"].as_str() == Some(registry_id))
        .expect("tested registry resolve node");
    let features = registry_node["features"].as_array().expect("resolve features");
    assert_eq!(
        features.iter().any(|feature| feature.as_str() == Some("async")),
        options.asynchronous,
        "tested registry async feature does not match example requirements"
    );
    assert_eq!(
        features.iter().any(|feature| feature.as_str() == Some("inventory")),
        options.inventory || options.local,
        "tested registry inventory feature does not match example requirements"
    );
    assert_eq!(
        ids.contains_key(&"qubit-fs-local"),
        options.local,
        "local provider dependency does not match example requirements"
    );
    if let (Some(local), Some(registry)) = (ids.get("qubit-fs-local"), ids.get("qubit-fs-registry")) {
        let node = metadata["resolve"]["nodes"]
            .as_array()
            .expect("resolved nodes")
            .iter()
            .find(|n| n["id"].as_str() == Some(local))
            .expect("local node");
        assert!(
            node["dependencies"]
                .as_array()
                .expect("dependency IDs")
                .iter()
                .any(|id| id.as_str() == Some(registry)),
            "local provider must use the tested registry identity"
        );
    }
}

/// Runs one Cargo operation and includes the command in a failed assertion.
fn cargo(root: &Path, target: &Path, arguments: &[&str], context: &str) -> std::process::Output {
    let mut command = Command::new(env!("CARGO"));
    command
        .args(arguments)
        .current_dir(root)
        .env("CARGO_TARGET_DIR", target);
    let output =
        run_with_timeout(&mut command, Duration::from_secs(900), context).unwrap_or_else(|error| panic!("{error}"));
    let stdout = if output.status.success() {
        String::new()
    } else {
        let raw = String::from_utf8_lossy(&output.stdout);
        let rendered = raw
            .lines()
            .filter_map(|line| from_str::<Json>(line).ok())
            .filter(|message| message["reason"] == "compiler-message")
            .filter_map(|message| message["message"]["rendered"].as_str().map(str::to_owned))
            .collect::<Vec<_>>()
            .join("\n");
        if rendered.is_empty() {
            raw.into_owned()
        } else {
            rendered
        }
    };
    assert!(
        output.status.success(),
        "cargo {arguments:?} failed\nstdout:\n{}\nstderr:\n{}",
        stdout,
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// Collects executable paths reported by Cargo for generated binary targets.
pub fn executable_artifacts(output: &[u8]) -> Result<BTreeMap<String, PathBuf>, String> {
    let text = std::str::from_utf8(output).map_err(|error| error.to_string())?;
    let mut artifacts = BTreeMap::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let message: Json = from_slice(line.as_bytes()).map_err(|error| error.to_string())?;
        if message["reason"].as_str() != Some("compiler-artifact") {
            continue;
        }
        let binary = message["target"]["kind"]
            .as_array()
            .is_some_and(|kinds| kinds.iter().any(|kind| kind.as_str() == Some("bin")));
        if !binary {
            continue;
        }
        let Some(executable) = message["executable"].as_str() else {
            continue;
        };
        let name = message["target"]["name"]
            .as_str()
            .ok_or_else(|| "executable artifact has no target name".to_owned())?;
        let path = PathBuf::from(executable);
        if let Some(previous) = artifacts.insert(name.to_owned(), path.clone())
            && previous != path
        {
            return Err(format!("conflicting executable artifacts for {name}"));
        }
    }
    Ok(artifacts)
}

/// Checks source-integrated programs in isolated directories with an explicit
/// feature matrix.
pub fn check_documents(root: &Path) {
    let input = manifest(root);
    let package = input["package"]["name"].as_str().expect("package name");
    let workspace = TempDir::new().expect("isolated documentation workspace");
    let target = workspace.path().join("target");
    for asynchronous in [false, true] {
        for local in [false, true] {
            for inventory in [false, true] {
                let group = format!("{}-{}-{}", asynchronous as u8, local as u8, inventory as u8);
                let project = workspace.path().join(&group);
                fs::create_dir_all(project.join("src/bin")).expect("create example sources");
                let mut programs = Vec::new();
                for (document_index, document) in CHECKED_DOCUMENTS.iter().enumerate() {
                    let source = fs::read_to_string(root.join(document)).expect("read document");
                    let examples = snippets(&source).unwrap_or_else(|e| panic!("{document}: {e}"));
                    assert!(!examples.is_empty(), "{document} needs examples");
                    for (index, snippet) in examples
                        .into_iter()
                        .enumerate()
                        .filter(|(_, s)| (s.asynchronous, s.local, s.inventory) == (asynchronous, local, inventory))
                    {
                        let name = format!("document_{document_index}_{index}");
                        fs::write(project.join("src/bin").join(format!("{name}.rs")), snippet.source)
                            .expect("write program");
                        programs.push((
                            name,
                            snippet.run,
                            format!("{document} example {index} in group {group}"),
                        ));
                    }
                }
                if programs.is_empty() {
                    continue;
                }
                let label = match (asynchronous, local, inventory) {
                    (false, false, false) => "default-only",
                    (true, false, false) => "async-only",
                    _ => "combined requirements",
                };
                eprintln!(
                    "checking documentation example group {label} (async={asynchronous}, local={local}, inventory={inventory})"
                );
                let example_manifest = documentation_manifest(root, &input, asynchronous, local, inventory);
                fs::write(
                    project.join("Cargo.toml"),
                    to_string(&example_manifest).expect("serialize manifest"),
                )
                .expect("write example manifest");
                cargo(
                    &project,
                    &target,
                    &["generate-lockfile"],
                    &format!("Cargo generate-lockfile in group {label}"),
                );
                let metadata = cargo(
                    &project,
                    &target,
                    &["metadata", "--locked", "--format-version", "1"],
                    &format!("Cargo metadata in group {label}"),
                );
                check_graph(
                    &from_slice(&metadata.stdout).expect("parse Cargo metadata"),
                    package,
                    root,
                    GraphOptions {
                        published: false,
                        registry: false,
                        asynchronous,
                        local,
                        inventory,
                        qubit_fs_override: Some(&root.join("../rs-fs")),
                        qubit_fs_local_override: Some(&root.join("../rs-fs-local")),
                        qubit_local_files_override: None,
                        qubit_operation_override: Some(&root.join("../rs-operation")),
                        example_manifest: Some(&project.join("Cargo.toml")),
                    },
                );
                let build = cargo(
                    &project,
                    &target,
                    &["build", "--locked", "--bins", "--message-format=json"],
                    &format!("Cargo build in group {label}"),
                );
                let executables = executable_artifacts(&build.stdout).expect("parse Cargo executable artifacts");
                for (name, run, context) in programs {
                    let executable = executables
                        .get(&name)
                        .unwrap_or_else(|| panic!("Cargo did not report an executable for {name}"));
                    if !run {
                        continue;
                    }
                    let execution = TempDir::new().expect("isolated example working directory");
                    let mut command = Command::new(executable);
                    command.current_dir(execution.path());
                    let output = run_with_timeout(&mut command, Duration::from_secs(30), &context)
                        .unwrap_or_else(|error| panic!("{error}"));
                    assert!(
                        output.status.success(),
                        "{name}: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
            }
        }
    }
}
