// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Verifies every Rust README example compiles with its documented dependencies.
#[test]
fn test_readme_rust_examples_compile() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output_dir = manifest_dir.join("target/markdown-doctest");
    recreate_dir(&output_dir);

    for (name, path) in [
        ("readme_en", manifest_dir.join("README.md")),
        ("readme_zh_cn", manifest_dir.join("README.zh_CN.md")),
    ] {
        let snippets = extract_rust_snippets(&path);
        assert!(
            !snippets.is_empty(),
            "{} should contain Rust snippets",
            path.display(),
        );
        compile_snippets(&manifest_dir, &output_dir, name, &snippets);
    }
}

/// Recreates a test-owned output directory.
fn recreate_dir(path: &Path) {
    if path.exists() {
        fs::remove_dir_all(path).expect("failed to remove old markdown doctest directory");
    }
    fs::create_dir_all(path).expect("failed to create markdown doctest directory");
}

/// Extracts fenced Rust snippets from one Markdown document.
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
fn is_rust_fence(language: &str) -> bool {
    let tag = language
        .trim()
        .split(|character: char| character == ',' || character.is_whitespace())
        .next()
        .unwrap_or_default();
    matches!(tag, "rust" | "rs")
}

/// Compiles every extracted snippet as an independent binary crate.
fn compile_snippets(manifest_dir: &Path, output_dir: &Path, name: &str, snippets: &[String]) {
    let crate_dir = output_dir.join(name);
    let bin_dir = crate_dir.join("src/bin");
    fs::create_dir_all(&bin_dir).expect("failed to create snippet binary directory");

    fs::write(
        crate_dir.join("Cargo.toml"),
        build_markdown_doctest_manifest(name, manifest_dir),
    )
    .expect("failed to write snippet Cargo manifest");

    for (index, snippet) in snippets.iter().enumerate() {
        fs::write(
            bin_dir.join(format!("snippet_{index}.rs")),
            normalize_snippet(snippet),
        )
        .expect("failed to write snippet source");
    }

    let status = Command::new("cargo")
        .arg("check")
        .arg("--quiet")
        .arg("--bins")
        .current_dir(&crate_dir)
        .env("CARGO_TARGET_DIR", output_dir.join("target"))
        .status()
        .expect("failed to compile Markdown snippets");
    assert!(status.success(), "Markdown Rust snippets failed for {name}");
}

/// Builds a temporary manifest with the dependencies used by README examples.
fn build_markdown_doctest_manifest(name: &str, manifest_dir: &Path) -> String {
    let registry = toml_basic_string(&manifest_dir.display().to_string());
    let filesystem = toml_basic_string(&manifest_dir.join("../rs-fs").display().to_string());
    let local = toml_basic_string(&manifest_dir.join("../rs-fs-local").display().to_string());

    format!(
        r#"[package]
name = "qubit-fs-registry-{name}-markdown-doctest"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
qubit-fs = {{ path = "{filesystem}" }}
qubit-fs-local = {{ path = "{local}", features = ["registry"] }}
qubit-fs-registry = {{ path = "{registry}" }}
"#,
    )
}

/// Escapes a filesystem path for a TOML basic string.
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
                write!(escaped, "\\u{:04X}", character as u32)
                    .expect("writing to a string should not fail");
            }
            _ => escaped.push(character),
        }
    }
    escaped
}

/// Wraps item-only snippets in a minimal binary entry point.
fn normalize_snippet(snippet: &str) -> String {
    let allow_example_noise = "#![allow(dead_code, unused_imports, unused_variables)]\n";
    if snippet.contains("fn main") {
        format!("{allow_example_noise}{snippet}\n")
    } else {
        format!("{allow_example_noise}fn main() {{\n{snippet}\n}}\n")
    }
}
