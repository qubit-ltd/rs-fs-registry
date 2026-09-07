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

use serde_json::Value as Json;
use serde_json::from_slice;
use tempfile::TempDir;
use toml::Table;
use toml::Value;
use toml::to_string;

/// One complete Markdown program and its minimum feature requirement.
#[derive(Debug, PartialEq)]
pub struct Snippet {
    pub source: String,
    pub asynchronous: bool,
    pub run: bool,
}

/// Extracts complete Rust programs, rejecting malformed or unknown directives.
pub fn snippets(text: &str) -> Result<Vec<Snippet>, String> {
    let mut result = Vec::new();
    let mut fence: Option<(bool, bool)> = None;
    let mut asynchronous = false;
    let mut source = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if fence.is_none() && trimmed.starts_with("<!-- registry-example:") {
            if trimmed != "<!-- registry-example: async -->" || asynchronous {
                return Err("unknown or repeated registry-example directive".into());
            }
            asynchronous = true;
        } else if let Some(language) = trimmed.strip_prefix("```") {
            if let Some((rust, run)) = fence.take() {
                if !language.is_empty() {
                    return Err("nested or malformed code fence".into());
                }
                if rust {
                    result.push(Snippet {
                        source: std::mem::take(&mut source),
                        asynchronous,
                        run,
                    });
                }
                asynchronous = false;
            } else {
                let rust = matches!(language, "rust" | "rs" | "rust,no_run");
                if language.starts_with("rust") && !rust {
                    return Err("unsupported Rust fence".into());
                }
                if asynchronous && !rust {
                    return Err("async directive must precede a Rust example".into());
                }
                fence = Some((rust, language != "rust,no_run"));
            }
        } else if matches!(fence, Some((true, _))) {
            source.push_str(line);
            source.push('\n');
        }
    }
    if fence.is_some() || asynchronous {
        return Err("unterminated fence or unused async directive".into());
    }
    Ok(result)
}

/// Reads the package manifest, failing explicitly on missing version metadata.
pub fn manifest(root: &Path) -> Value {
    fs::read_to_string(root.join("Cargo.toml"))
        .expect("read package manifest")
        .parse()
        .expect("parse package manifest")
}

/// Builds a minimal example manifest; only the tested package may be patched in
/// published mode.
pub fn documentation_manifest(root: &Path, input: &Value, published: bool, asynchronous: bool) -> Value {
    let package_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let package = input["package"]["name"].as_str().expect("package name");
    let mut dependencies = Table::new();
    for name in ["qubit-fs", "qubit-fs-registry", "qubit-spi"] {
        if let Some(value) = input.get("dependencies").and_then(|deps| deps.get(name)) {
            dependencies.insert(name.into(), dependency(root, value, published));
        }
    }
    let extra = input["package"]
        .get("metadata")
        .and_then(|m| m.get("documentation"))
        .and_then(|d| d.get("dependencies"));
    assert!(extra.is_some(), "documentation dependency metadata is required");
    if let Some(extra) = extra {
        for (name, value) in extra.as_table().expect("documentation dependencies table") {
            if value.get("async-only").and_then(Value::as_bool) == Some(true) && !asynchronous {
                continue;
            }
            dependencies.insert(name.clone(), dependency(root, value, published));
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
        Value::String(package_root.to_str().expect("UTF-8 root").into()),
    );
    current.insert("default-features".into(), Value::Boolean(false));
    let features = if package == "qubit-fs-local" {
        vec![Value::String("registry".into())]
    } else if asynchronous {
        vec![Value::String("async".into())]
    } else {
        vec![]
    };
    current.insert("features".into(), Value::Array(features));
    dependencies.insert(package.into(), Value::Table(current));
    let mut output: Value = "[package]\nname='filesystem-documentation-check'\nversion='0.0.0'\nedition='2024'\npublish=false\n[workspace]\n".parse().expect("static manifest");
    output
        .as_table_mut()
        .expect("manifest table")
        .insert("dependencies".into(), Value::Table(dependencies));
    let patch: Value = format!(
        "[crates-io.{package}]\npath={}\n",
        Value::String(package_root.to_str().expect("UTF-8 root").into())
    )
    .parse()
    .expect("self patch");
    output
        .as_table_mut()
        .expect("manifest table")
        .insert("patch".into(), patch);
    output
}

/// Keeps version/feature requirements while choosing local or published
/// sources.
fn dependency(root: &Path, value: &Value, published: bool) -> Value {
    let mut table = match value {
        Value::String(version) => Table::from_iter([("version".into(), Value::String(version.clone()))]),
        Value::Table(table) => table.clone(),
        _ => panic!("dependency must declare a version"),
    };
    assert!(
        table.get("version").and_then(Value::as_str).is_some(),
        "dependency version is required"
    );
    table.remove("optional");
    table.remove("async-only");
    if let Some(path) = table.remove("path") {
        let declared_path = Path::new(path.as_str().expect("dependency path"));
        let path = resolve_dependency_path(root, declared_path);
        if !published && path.join("Cargo.toml").is_file() {
            let path = path.canonicalize().expect("dependency path must resolve");
            table.insert(
                "path".into(),
                Value::String(path.to_str().expect("UTF-8 dependency path").into()),
            );
        }
    }
    Value::Table(table)
}

/// Resolves a dependency against the isolated sibling view when Cargo has
/// rewritten its manifest path to an absolute checkout path.
fn resolve_dependency_path(root: &Path, declared_path: &Path) -> PathBuf {
    let Some(sibling_root) = std::env::var_os("QUBIT_FS_SIBLING_ROOT") else {
        return root.join(declared_path);
    };
    let sibling_root = PathBuf::from(sibling_root);
    let direct = sibling_root.join(
        declared_path
            .file_name()
            .expect("dependency path must name a sibling crate"),
    );
    if direct.join("Cargo.toml").is_file() {
        return direct;
    }
    let package = fs::read_to_string(declared_path.join("Cargo.toml"))
        .ok()
        .and_then(|source| source.parse::<Value>().ok())
        .and_then(|value| value["package"]["name"].as_str().map(str::to_owned));
    if let Some(package) = package
        && let Some(suffix) = package.strip_prefix("qubit-")
    {
        let mapped = sibling_root.join(format!("rs-{suffix}"));
        if mapped.join("Cargo.toml").is_file() {
            return mapped;
        }
    }
    root.join(declared_path)
}

/// Rejects duplicate filesystem/SPI package identities and a downstream using
/// another registry.
pub fn check_graph(metadata: &Json, package: &str, root: &Path, published: bool) {
    let mut ids = BTreeMap::new();
    for entry in metadata["packages"].as_array().expect("metadata packages") {
        let name = entry["name"].as_str().expect("package name");
        if matches!(name, "qubit-fs" | "qubit-fs-registry" | "qubit-spi" | "qubit-fs-local") {
            assert!(
                ids.insert(name, entry["id"].as_str().expect("package ID")).is_none(),
                "duplicate package identity: {name}"
            );
            if name == package {
                assert_eq!(
                    Path::new(entry["manifest_path"].as_str().expect("manifest path"))
                        .canonicalize()
                        .expect("manifest exists"),
                    root.join("Cargo.toml").canonicalize().expect("tested manifest exists")
                );
            } else if published {
                assert!(
                    entry["source"].as_str().is_some_and(|s| s.starts_with("registry+")),
                    "published dependency uses a local source: {name}"
                );
            }
        }
    }
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
fn cargo(root: &Path, target: &Path, arguments: &[&str]) -> std::process::Output {
    let output = Command::new(env!("CARGO"))
        .args(arguments)
        .current_dir(root)
        .env("CARGO_TARGET_DIR", target)
        .output()
        .expect("run Cargo");
    assert!(
        output.status.success(),
        "cargo {arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// Checks the shipped programs in isolated directories with an explicit feature
/// matrix.
pub fn check_documents(root: &Path, include_async: bool) {
    let published = match std::env::var("QUBIT_FS_REGISTRY_DOC_DEPS").as_deref() {
        Ok("published") => true,
        Ok("local") | Err(_) => false,
        Ok(value) => panic!("unknown documentation dependency mode: {value}"),
    };
    let input = manifest(root);
    let package = input["package"]["name"].as_str().expect("package name");
    let workspace = TempDir::new().expect("isolated documentation workspace");
    let target = root.join("target/documentation-examples");
    for asynchronous in [false, true] {
        if asynchronous && !include_async {
            continue;
        }
        let project = workspace.path().join(if asynchronous { "async" } else { "sync" });
        fs::create_dir_all(project.join("src/bin")).expect("create example sources");
        let mut programs = Vec::new();
        for (document_index, document) in [
            "README.md",
            "README.zh_CN.md",
            "doc/user_guide.md",
            "doc/user_guide.zh_CN.md",
        ]
        .iter()
        .enumerate()
        {
            let source = fs::read_to_string(root.join(document)).expect("read document");
            let examples = snippets(&source).unwrap_or_else(|e| panic!("{document}: {e}"));
            assert!(!examples.is_empty(), "{document} needs examples");
            for (index, snippet) in examples
                .into_iter()
                .enumerate()
                .filter(|(_, s)| s.asynchronous == asynchronous)
            {
                let name = format!("document_{document_index}_{index}");
                fs::write(project.join("src/bin").join(format!("{name}.rs")), snippet.source).expect("write program");
                programs.push((name, snippet.run));
            }
        }
        if programs.is_empty() {
            continue;
        }
        fs::write(
            project.join("Cargo.toml"),
            to_string(&documentation_manifest(root, &input, published, asynchronous)).expect("serialize manifest"),
        )
        .expect("write example manifest");
        cargo(&project, &target, &["generate-lockfile"]);
        let metadata = cargo(&project, &target, &["metadata", "--locked", "--format-version", "1"]);
        check_graph(
            &from_slice(&metadata.stdout).expect("parse Cargo metadata"),
            package,
            root,
            published,
        );
        cargo(&project, &target, &["build", "--locked", "--bins", "--quiet"]);
        for (name, run) in programs {
            if !run {
                continue;
            }
            let execution = TempDir::new().expect("isolated example working directory");
            let executable = target
                .join("debug")
                .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
            let output = Command::new(executable)
                .current_dir(execution.path())
                .output()
                .expect("execute example");
            assert!(
                output.status.success(),
                "{name}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}
