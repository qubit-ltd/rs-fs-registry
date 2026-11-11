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
use std::path::absolute;
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

/// Builds a minimal example manifest; only the tested package may be patched in
/// published mode.
pub fn documentation_manifest(
    root: &Path,
    input: &Value,
    published: bool,
    asynchronous: bool,
    local: bool,
    inventory: bool,
) -> Value {
    let package = input["package"]["name"].as_str().expect("package name");
    let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let sibling_alias = root
        .parent()
        .map(|parent| parent.join(format!("rs-{}", package.trim_start_matches("qubit-"))))
        .filter(|path| path.is_dir() && path.canonicalize().ok().as_ref() == Some(&canonical_root));
    let package_root = sibling_alias.unwrap_or(canonical_root);
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
            if (name == "qubit-fs-local" && !local)
                || (value.get("async-only").and_then(Value::as_bool) == Some(true) && !asynchronous)
            {
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
    let patch_source = format!(
        "[crates-io.{package}]\npath={}\n",
        Value::String(package_root.to_str().expect("UTF-8 root").into())
    );
    let mut patch: Value = from_str(&patch_source).expect("self patch");
    if !published {
        let sibling_fs = root.parent().expect("registry has a parent directory").join("rs-fs");
        if sibling_fs.join("Cargo.toml").is_file() {
            patch["crates-io"].as_table_mut().expect("patch table").insert(
                "qubit-fs".into(),
                Value::Table(Table::from_iter([(
                    "path".into(),
                    Value::String(sibling_fs.to_str().expect("UTF-8 filesystem path").into()),
                )])),
            );
        }
        let sibling_redact = root
            .parent()
            .expect("registry has a parent directory")
            .join("rs-redact");
        if sibling_redact.join("Cargo.toml").is_file() {
            patch["crates-io"].as_table_mut().expect("patch table").insert(
                "qubit-redact".into(),
                Value::Table(Table::from_iter([(
                    "path".into(),
                    Value::String(sibling_redact.to_str().expect("UTF-8 redaction path").into()),
                )])),
            );
        }
        let sibling_fs_local = root
            .parent()
            .expect("registry has a parent directory")
            .join("rs-fs-local");
        if sibling_fs_local.join("Cargo.toml").is_file() && local_provider_uses_registry(&sibling_fs_local, root) {
            patch["crates-io"].as_table_mut().expect("patch table").insert(
                "qubit-fs-local".into(),
                Value::Table(Table::from_iter([(
                    "path".into(),
                    Value::String(sibling_fs_local.to_str().expect("UTF-8 local filesystem path").into()),
                )])),
            );
        }
        let sibling_io = root.parent().expect("registry has a parent directory").join("rs-io");
        if sibling_io.join("Cargo.toml").is_file() {
            patch["crates-io"].as_table_mut().expect("patch table").insert(
                "qubit-io".into(),
                Value::Table(Table::from_iter([(
                    "path".into(),
                    Value::String(sibling_io.to_str().expect("UTF-8 I/O path").into()),
                )])),
            );
        }
        let sibling_spi = root.parent().expect("registry has a parent directory").join("rs-spi");
        if sibling_spi.join("Cargo.toml").is_file() {
            patch["crates-io"].as_table_mut().expect("patch table").insert(
                "qubit-spi".into(),
                Value::Table(Table::from_iter([(
                    "path".into(),
                    Value::String(sibling_spi.to_str().expect("UTF-8 SPI path").into()),
                )])),
            );
        }
    }
    output
        .as_table_mut()
        .expect("manifest table")
        .insert("patch".into(), patch);
    output
}

pub(crate) fn local_provider_uses_registry(local_provider: &Path, registry: &Path) -> bool {
    let source = match fs::read_to_string(local_provider.join("Cargo.toml")) {
        Ok(source) => source,
        Err(_) => return false,
    };
    let manifest: Value = match from_str(&source) {
        Ok(manifest) => manifest,
        Err(_) => return false,
    };
    let Some(path) = manifest
        .get("dependencies")
        .and_then(|dependencies| dependencies.get("qubit-fs-registry"))
        .and_then(Value::as_table)
        .and_then(|dependency| dependency.get("path"))
        .and_then(Value::as_str)
    else {
        return false;
    };
    let provider_root = local_provider
        .canonicalize()
        .unwrap_or_else(|_| local_provider.to_path_buf());
    let provider_registry = provider_root.join(path).canonicalize();
    let current_registry = registry.canonicalize();
    matches!((provider_registry, current_registry), (Ok(provider), Ok(current)) if provider == current)
}

pub(crate) fn local_provider_manifest(
    local_provider: &Path,
    registry: &Path,
    sibling_root: &Path,
    published: bool,
    destination: &Path,
) -> PathBuf {
    let local_root = local_provider.canonicalize().expect("local provider must exist");
    let source = fs::read_to_string(local_root.join("Cargo.toml")).expect("read local provider manifest");
    let mut manifest: Value = from_str(&source).expect("parse local provider manifest");
    let registry = registry.canonicalize().expect("tested registry must exist");
    let scopes = manifest.as_table_mut().expect("local provider manifest table");
    for name in ["dependencies", "build-dependencies"] {
        if let Some(dependencies) = scopes.get_mut(name).and_then(Value::as_table_mut) {
            rewrite_local_provider_paths(dependencies, &local_root, &registry, sibling_root, published);
        }
    }
    if let Some(targets) = scopes.get_mut("target").and_then(Value::as_table_mut) {
        for (_, target_value) in targets.iter_mut() {
            let Some(target) = target_value.as_table_mut() else {
                continue;
            };
            for name in ["dependencies", "build-dependencies"] {
                if let Some(dependencies) = target.get_mut(name).and_then(Value::as_table_mut) {
                    rewrite_local_provider_paths(dependencies, &local_root, &registry, sibling_root, published);
                }
            }
        }
    }
    scopes.remove("dev-dependencies");
    scopes.remove("patch");
    scopes.remove("bench");
    let library_path = scopes
        .get("lib")
        .and_then(Value::as_table)
        .and_then(|library| library.get("path"))
        .and_then(Value::as_str)
        .unwrap_or("src/lib.rs");
    let library_path = local_root
        .join(library_path)
        .canonicalize()
        .expect("local provider library");
    scopes
        .entry("lib")
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("library table")
        .insert(
            "path".into(),
            Value::String(library_path.to_str().expect("UTF-8 library path").into()),
        );
    fs::create_dir_all(destination).expect("create local provider manifest shim");
    let manifest_path = destination.join("Cargo.toml");
    fs::write(
        &manifest_path,
        to_string(&manifest).expect("serialize local provider manifest"),
    )
    .expect("write local provider manifest shim");
    manifest_path
}

fn rewrite_local_provider_paths(
    dependencies: &mut Table,
    local_root: &Path,
    registry: &Path,
    sibling_root: &Path,
    published: bool,
) {
    for (name, dependency) in dependencies {
        let Some(declaration) = dependency.as_table_mut() else {
            continue;
        };
        let Some(path) = declaration.get("path").and_then(Value::as_str) else {
            continue;
        };
        if published
            && !matches!(
                name.as_str(),
                "qubit-fs-registry" | "qubit-fs" | "qubit-fs-local" | "qubit-local-files"
            )
        {
            assert!(
                declaration.contains_key("version"),
                "published local-provider path dependency `{name}` needs a registry version"
            );
            declaration.remove("path");
            continue;
        }
        let resolved = match name.as_str() {
            "qubit-fs-registry" => registry.to_path_buf(),
            "qubit-fs" => sibling_root.join("rs-fs"),
            "qubit-fs-local" => sibling_root.join("rs-fs-local"),
            "qubit-local-files" => sibling_root.join("rs-local-files"),
            _ => local_root.join(path),
        };
        declaration.insert(
            "path".into(),
            Value::String(resolved.to_str().expect("UTF-8 dependency path").into()),
        );
    }
}

/// Keeps version/feature requirements while choosing local or published
/// sources.
pub(crate) fn dependency(root: &Path, value: &Value, published: bool) -> Value {
    let mut table = match value {
        Value::String(version) => Table::from_iter([("version".into(), Value::String(version.clone()))]),
        Value::Table(table) => table.clone(),
        _ => panic!("dependency must declare a version"),
    };
    if table.get("version").and_then(Value::as_str).is_none() {
        let declared_path = table
            .get("path")
            .and_then(Value::as_str)
            .expect("dependency requires a path or version");
        let sibling = resolve_dependency_path(root, Path::new(declared_path));
        let source = fs::read_to_string(sibling.join("Cargo.toml"))
            .expect("path-only dependency requires an available sibling manifest");
        let manifest: Value = from_str(&source).expect("parse sibling manifest");
        let version = manifest["package"]["version"]
            .as_str()
            .expect("sibling package version");
        table.insert("version".into(), Value::String(version.to_owned()));
    }
    table.remove("optional");
    table.remove("async-only");
    if let Some(path) = table.remove("path") {
        let declared_path = Path::new(path.as_str().expect("dependency path"));
        let path = resolve_dependency_path(root, declared_path);
        if !published && path.join("Cargo.toml").is_file() {
            let path = absolute(path).expect("dependency path must be absolute");
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
        .and_then(|source| from_str::<Value>(&source).ok())
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
#[derive(Clone, Copy, Debug, Default)]
pub struct GraphOptions<'a> {
    pub published: bool,
    pub asynchronous: bool,
    pub local: bool,
    pub inventory: bool,
    pub qubit_fs_override: Option<&'a Path>,
    pub qubit_fs_local_override: Option<&'a Path>,
    pub qubit_local_files_override: Option<&'a Path>,
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
                assert_eq!(source, None, "tested package must be the extracted archive");
                assert_eq!(
                    manifest_path,
                    root.join("Cargo.toml").canonicalize().expect("tested manifest exists")
                );
            } else if name == "filesystem-documentation-check" {
                assert_eq!(source, None, "example consumer must be the generated manifest");
                assert_eq!(
                    Some(manifest_path),
                    expected_example_manifest,
                    "unexpected generated example consumer manifest"
                );
            } else if source.is_none() {
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
                assert!(
                    source.is_some_and(|source| source.starts_with("registry+")),
                    "published dependency uses a non-registry source: {name}"
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

/// Reads the exact `qubit-fs` patch from this check's isolated Cargo home.
fn cargo_qubit_fs_override() -> Option<PathBuf> {
    let cargo_home = std::env::var_os("CARGO_HOME")?;
    let source = fs::read_to_string(PathBuf::from(cargo_home).join("config.toml")).ok()?;
    let config: Value = from_str(&source).ok()?;
    config["patch"]["crates-io"]["qubit-fs"]["path"]
        .as_str()
        .map(PathBuf::from)
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
    assert!(
        output.status.success(),
        "cargo {arguments:?}: {}",
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
    let qubit_fs_override = if published { cargo_qubit_fs_override() } else { None };
    let sibling_root = std::env::var_os("QUBIT_FS_SIBLING_ROOT").map(PathBuf::from);
    let qubit_local_files_override = sibling_root.as_ref().map(|parent| parent.join("rs-local-files"));
    let target = root.join("target/documentation-examples");
    for asynchronous in [false, true] {
        for local in [false, true] {
            for inventory in [false, true] {
                if asynchronous && !include_async {
                    continue;
                }
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
                let mut example_manifest =
                    documentation_manifest(root, &input, published, asynchronous, local, inventory);
                let mut local_provider_alias = None;
                if local {
                    let sibling_local = if published {
                        sibling_root.as_ref().map(|parent| parent.join("rs-fs-local"))
                    } else {
                        root.parent().map(|parent| parent.join("rs-fs-local"))
                    };
                    if let Some(sibling_local) = sibling_local.filter(|path| path.join("Cargo.toml").is_file())
                        && (published || !local_provider_uses_registry(&sibling_local, root))
                    {
                        let shim = local_provider_manifest(
                            &sibling_local,
                            root,
                            sibling_root
                                .as_deref()
                                .or_else(|| root.parent())
                                .expect("sibling root is required for local provider shim"),
                            published,
                            &workspace.path().join("local-provider-shim"),
                        );
                        let shim_dir = shim.parent().expect("shim manifest has a parent");
                        if published {
                            example_manifest["dependencies"]["qubit-fs-local"]
                                .as_table_mut()
                                .expect("local provider dependency table")
                                .insert(
                                    "path".into(),
                                    Value::String(shim_dir.to_str().expect("UTF-8 shim directory").into()),
                                );
                        } else {
                            example_manifest["patch"]["crates-io"]
                                .as_table_mut()
                                .expect("patch table")
                                .insert(
                                    "qubit-fs-local".into(),
                                    Value::Table(Table::from_iter([(
                                        "path".into(),
                                        Value::String(shim_dir.to_str().expect("UTF-8 shim directory").into()),
                                    )])),
                                );
                        }
                        local_provider_alias = Some(shim_dir.to_path_buf());
                    }
                }
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
                        published,
                        asynchronous,
                        local,
                        inventory,
                        qubit_fs_override: qubit_fs_override.as_deref(),
                        qubit_fs_local_override: local_provider_alias.as_deref(),
                        qubit_local_files_override: qubit_local_files_override.as_deref(),
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
