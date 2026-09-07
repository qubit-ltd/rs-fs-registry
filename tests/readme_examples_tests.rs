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
<<<<<<< HEAD
fn test_documentation_versions_follow_manifest() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let input = manifest(root);
    let output = documentation_manifest(root, &input, true, false);
    assert_eq!(
        output["dependencies"]["qubit-fs"]["version"],
        input["dependencies"]["qubit-fs"]["version"]
=======
fn test_shipped_markdown_rust_examples_compile() {
    // An isolated checkout may keep sibling crates in a separate workspace view.
    let manifest_dir = std::env::var_os("QUBIT_FS_SIBLING_ROOT")
        .map(|root| PathBuf::from(root).join("rs-fs-registry"))
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    let workspace = MarkdownDoctestWorkspace::new(&manifest_dir);

    for (name, path) in [
        ("readme_en", manifest_dir.join("README.md")),
        ("readme_zh_cn", manifest_dir.join("README.zh_CN.md")),
        ("user_guide_en", manifest_dir.join("doc/user_guide.md")),
        ("user_guide_zh_cn", manifest_dir.join("doc/user_guide.zh_CN.md")),
    ] {
        let snippets = extract_rust_snippets(&path);
        assert!(!snippets.is_empty(), "{} should contain Rust snippets", path.display(),);
        compile_snippets(
            &manifest_dir,
            &workspace.output_dir,
            &workspace.target_dir,
            name,
            &snippets,
        );
    }
}

/// Owns generated Markdown example sources and their shared Cargo cache.
struct MarkdownDoctestWorkspace {
    /// Process-scoped source directory removed when the test finishes.
    output_dir: PathBuf,
    /// Shared Cargo target directory reused across test processes.
    target_dir: PathBuf,
}

impl MarkdownDoctestWorkspace {
    /// Creates a clean process-scoped source directory.
    ///
    /// # Parameters
    ///
    /// - `manifest_dir`: Registry crate manifest directory.
    ///
    /// # Returns
    ///
    /// A workspace whose generated sources are removed on drop.
    fn new(manifest_dir: &Path) -> Self {
        let output_dir = markdown_doctest_output_dir(manifest_dir);
        recreate_dir(&output_dir);
        Self {
            output_dir,
            target_dir: manifest_dir.join("target/markdown-doctest-target"),
        }
    }
}

impl Drop for MarkdownDoctestWorkspace {
    /// Removes generated process-scoped Markdown example sources.
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.output_dir);
    }
}

/// Returns the current process's isolated Markdown example output directory.
///
/// # Parameters
///
/// - `manifest_dir`: Registry crate manifest directory.
///
/// # Returns
///
/// A process-scoped directory below the crate's `target` directory.
fn markdown_doctest_output_dir(manifest_dir: &Path) -> PathBuf {
    manifest_dir.join(format!("target/markdown-doctest-{}", std::process::id()))
}

/// Verifies concurrent test processes receive independent temporary outputs.
#[test]
fn test_markdown_doctest_output_dir_scopes_to_current_process() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output_dir = markdown_doctest_output_dir(&manifest_dir);

    assert!(
        output_dir.ends_with(format!("markdown-doctest-{}", std::process::id())),
        "the output directory should be isolated to the current test process",
>>>>>>> 907037e (test(docs): resolve current sibling dependencies)
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
<<<<<<< HEAD
#[should_panic(expected = "documentation dependency metadata is required")]
fn test_missing_documentation_metadata_is_rejected() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut input = manifest(root);
    input["package"].as_table_mut().unwrap().remove("metadata");
    documentation_manifest(root, &input, true, false);
=======
fn test_markdown_doctest_manifest_uses_published_dependencies_without_siblings() {
    let manifest_dir = Path::new("/nonexistent/qubit-fs-registry");
    let manifest = build_markdown_doctest_manifest("packaged", manifest_dir);

    assert!(manifest.contains("qubit-fs = \"0.4\""));
    assert!(manifest.contains("qubit-fs-local = { version = \"0.3\", features = [\"registry\"] }",));
    assert!(!manifest.contains("../rs-fs"));
    assert!(!manifest.contains("../rs-fs-local"));
>>>>>>> 907037e (test(docs): resolve current sibling dependencies)
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

<<<<<<< HEAD
/// Each example is compiled and run with its documented minimum features.
#[test]
fn test_shipped_markdown_rust_examples_run() {
    check_documents(Path::new(env!("CARGO_MANIFEST_DIR")), cfg!(feature = "async"));
=======
/// Extracts fenced Rust snippets from one Markdown document.
///
/// # Parameters
///
/// - `path`: Markdown document to read.
///
/// # Returns
///
/// Rust code blocks in document order.
///
/// # Panics
///
/// Panics when the Markdown document cannot be read.
fn extract_rust_snippets(path: &Path) -> Vec<String> {
    let content = fs::read_to_string(path).expect("failed to read Markdown file");
    let mut snippets = Vec::new();
    let mut in_rust = false;
    let mut current = String::new();

    for line in content.lines() {
        if let Some(language) = line.trim_start().strip_prefix("```") {
            if in_rust {
                snippets.push(current.trim().to_owned());
                current.clear();
                in_rust = false;
                continue;
            }
            in_rust = is_rust_fence(language);
            continue;
        }

        if in_rust {
            current.push_str(line);
            current.push('\n');
        }
    }

    snippets
}

/// Returns whether a Markdown fence declares Rust source code.
///
/// # Parameters
///
/// - `language`: Fence info string following the opening backticks.
///
/// # Returns
///
/// `true` when the first fence tag is `rust` or `rs`.
fn is_rust_fence(language: &str) -> bool {
    let tag = language
        .trim()
        .split(|character: char| character == ',' || character.is_whitespace())
        .next()
        .unwrap_or_default();
    matches!(tag, "rust" | "rs")
}

/// Compiles every extracted snippet as an independent binary crate.
///
/// # Parameters
///
/// - `manifest_dir`: Registry crate manifest directory.
/// - `output_dir`: Test-owned root for generated crates and Cargo output.
/// - `target_dir`: Shared Cargo target directory for generated crates.
/// - `name`: Stable generated crate name suffix.
/// - `snippets`: Rust snippets to compile.
///
/// # Panics
///
/// Panics when fixture files cannot be written, Cargo cannot run, or any
/// snippet fails to compile.
fn compile_snippets(manifest_dir: &Path, output_dir: &Path, target_dir: &Path, name: &str, snippets: &[String]) {
    let crate_dir = output_dir.join(name);
    let bin_dir = crate_dir.join("src/bin");
    fs::create_dir_all(&bin_dir).expect("failed to create snippet binary directory");

    fs::write(
        crate_dir.join("Cargo.toml"),
        build_markdown_doctest_manifest(name, manifest_dir),
    )
    .expect("failed to write snippet Cargo manifest");

    for (index, snippet) in snippets.iter().enumerate() {
        fs::write(bin_dir.join(format!("snippet_{index}.rs")), normalize_snippet(snippet))
            .expect("failed to write snippet source");
    }

    let status = Command::new("cargo")
        .arg("check")
        .arg("--quiet")
        .arg("--bins")
        .current_dir(&crate_dir)
        .env("CARGO_TARGET_DIR", target_dir)
        .status()
        .expect("failed to compile Markdown snippets");
    assert!(status.success(), "Markdown Rust snippets failed for {name}");
}

/// Builds a temporary manifest with the dependencies used by README examples.
///
/// # Parameters
///
/// - `name`: Stable generated crate name suffix.
/// - `manifest_dir`: Registry crate manifest directory.
///
/// # Returns
///
/// A Cargo manifest referencing the registry crate and either local sibling or
/// published filesystem and provider crates.
fn build_markdown_doctest_manifest(name: &str, manifest_dir: &Path) -> String {
    let registry_path = manifest_dir
        .canonicalize()
        .unwrap_or_else(|_| manifest_dir.to_path_buf());
    let registry = toml_basic_string(&registry_path.display().to_string());
    let sibling_root = manifest_dir.parent().expect("crate directory must have a parent");
    let filesystem_path = sibling_root.join("rs-fs");
    let local_path = sibling_root.join("rs-fs-local");
    let (filesystem, local) = if filesystem_path.join("Cargo.toml").is_file() && local_path.join("Cargo.toml").is_file()
    {
        (
            format!(
                "{{ path = \"{}\" }}",
                toml_basic_string(
                    &filesystem_path
                        .canonicalize()
                        .expect("filesystem crate must exist")
                        .display()
                        .to_string()
                ),
            ),
            format!(
                "{{ path = \"{}\", features = [\"registry\"] }}",
                toml_basic_string(
                    &local_path
                        .canonicalize()
                        .expect("local crate must exist")
                        .display()
                        .to_string()
                ),
            ),
        )
    } else {
        (
            "\"0.4\"".to_owned(),
            "{ version = \"0.3\", features = [\"registry\"] }".to_owned(),
        )
    };

    format!(
        r#"[package]
name = "qubit-fs-registry-{name}-markdown-doctest"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
qubit-fs = {filesystem}
qubit-fs-local = {local}
qubit-fs-registry = {{ path = "{registry}" }}
"#,
    )
}

/// Escapes a filesystem path for a TOML basic string.
///
/// # Parameters
///
/// - `value`: Unescaped path text.
///
/// # Returns
///
/// Text escaped according to TOML basic-string rules.
fn toml_basic_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\u{0008}' => escaped.push_str("\\b"),
            '\t' => escaped.push_str("\\t"),
            '\n' => escaped.push_str("\\n"),
            '\u{000C}' => escaped.push_str("\\f"),
            '\r' => escaped.push_str("\\r"),
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\u{0000}'..='\u{001F}' | '\u{007F}' => {
                write!(escaped, "\\u{:04X}", character as u32).expect("writing to a string should not fail");
            }
            _ => escaped.push(character),
        }
    }
    escaped
}

/// Wraps item-only snippets in a minimal binary entry point.
///
/// # Parameters
///
/// - `snippet`: Extracted Rust source.
///
/// # Returns
///
/// A compilable binary source file preserving snippets that already define
/// `main`.
fn normalize_snippet(snippet: &str) -> String {
    let allow_example_noise = "#![allow(dead_code, unused_imports, unused_variables)]\n";
    if snippet.contains("fn main") {
        format!("{allow_example_noise}{snippet}\n")
    } else {
        format!("{allow_example_noise}fn main() {{\n{snippet}\n}}\n")
    }
>>>>>>> 907037e (test(docs): resolve current sibling dependencies)
}
