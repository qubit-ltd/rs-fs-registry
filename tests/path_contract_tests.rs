// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Regression tests for resolution path validation delegation.

#[path = "common.rs"]
mod common;

use qubit_fs::Path;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::FileSystemLimit;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;

#[test]
fn synchronous_resolution_rejects_paths_outside_the_filesystem_contract() {
    let error = common::sync_resolution_with_path_properties(
        "path-contract",
        "resource",
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
    )
    .expect_err("relative path must be rejected by an absolute-only filesystem");
    assert_eq!(FsErrorKind::InvalidPath, error.kind());
    assert_eq!(FsOperation::ParsePath, error.operation());
    assert_eq!(Some("resource"), error.path().map(Path::as_str));
    assert_eq!(Some("path-contract"), error.provider());

    let error = common::sync_resolution_with_path_properties(
        "path-contract",
        "/long",
        FileSystemLimits::unknown().with_max_path_text_bytes(FileSystemLimit::Maximum(3)),
        PathConstraints::absolute(),
    )
    .expect_err("paths exceeding the configured limit must be rejected");
    assert_eq!(FsErrorKind::ResourceLimitExceeded, error.kind());
    assert_eq!(FsOperation::ParsePath, error.operation());
    assert_eq!(Some("/long"), error.path().map(Path::as_str));
    assert_eq!(Some("path-contract"), error.provider());
}

#[test]
fn synchronous_resolution_accepts_relative_and_literal_object_key_paths() {
    let relative = common::sync_resolution_with_path_semantics(
        "relative-provider",
        "dir/file",
        FileSystemLimits::unknown(),
        PathConstraints::relative(),
        PathSemantics::Hierarchical,
    )
    .expect("relative paths accepted by relative filesystem");
    assert_eq!("dir/file", relative.path().as_str());

    let literal = common::sync_resolution_with_path_semantics(
        "object-provider",
        "bucket//./object",
        FileSystemLimits::unknown(),
        PathConstraints::either(),
        PathSemantics::ObjectKey,
    )
    .expect("literal object-key paths accepted by object filesystem");
    assert_eq!("bucket//./object", literal.path().as_str());
    assert_eq!(PathSemantics::ObjectKey, literal.path().semantics());

    let normalized = Path::parse("bucket//./object").expect("hierarchical path parses");
    let literal = Path::parse_literal("bucket//./object").expect("literal path parses");
    assert_ne!(normalized.as_str(), literal.as_str());
    assert_eq!(PathSemantics::ObjectKey, literal.semantics());
}

#[cfg(feature = "async")]
#[test]
fn asynchronous_resolution_rejects_paths_outside_the_filesystem_contract() {
    let error = common::async_resolution_with_path_properties(
        "path-contract",
        "resource",
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
    )
    .expect_err("relative path must be rejected by an absolute-only filesystem");
    assert_eq!(FsErrorKind::InvalidPath, error.kind());
    assert_eq!(FsOperation::ParsePath, error.operation());
    assert_eq!(Some("resource"), error.path().map(Path::as_str));
    assert_eq!(Some("path-contract"), error.provider());

    let error = common::async_resolution_with_path_properties(
        "path-contract",
        "/long",
        FileSystemLimits::unknown().with_max_path_text_bytes(FileSystemLimit::Maximum(3)),
        PathConstraints::absolute(),
    )
    .expect_err("paths exceeding the configured limit must be rejected");
    assert_eq!(FsErrorKind::ResourceLimitExceeded, error.kind());
    assert_eq!(FsOperation::ParsePath, error.operation());
    assert_eq!(Some("/long"), error.path().map(Path::as_str));
    assert_eq!(Some("path-contract"), error.provider());
}

#[cfg(feature = "async")]
#[test]
fn asynchronous_resolution_accepts_relative_and_literal_object_key_paths() {
    let relative = common::async_resolution_with_path_semantics(
        "relative-provider",
        "dir/file",
        FileSystemLimits::unknown(),
        PathConstraints::relative(),
        PathSemantics::Hierarchical,
    )
    .expect("relative paths accepted by relative filesystem");
    assert_eq!("dir/file", relative.path().as_str());

    let literal = common::async_resolution_with_path_semantics(
        "object-provider",
        "bucket//./object",
        FileSystemLimits::unknown(),
        PathConstraints::either(),
        PathSemantics::ObjectKey,
    )
    .expect("literal object-key paths accepted by object filesystem");
    assert_eq!("bucket//./object", literal.path().as_str());
    assert_eq!(PathSemantics::ObjectKey, literal.path().semantics());
}
