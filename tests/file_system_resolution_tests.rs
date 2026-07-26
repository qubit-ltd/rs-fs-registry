// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::sync::Arc;

use qubit_fs::{
    FileMetadata,
    FileSystem,
    FileSystemCapabilities,
    FileSystemId,
    FileSystemInfo,
    FileSystemLimits,
    FileSystemProperties,
    FsError,
    FsErrorKind,
    FsOperation,
    FsPath,
    FsResult,
    FsUri,
    PathSemantics,
};
use qubit_fs_registry::FileSystemResolution;

/// Verifies provider-decoded paths and canonical URIs remain paired.
#[test]
fn resolution_preserves_decoded_path_and_canonical_uri() {
    let filesystem: Arc<dyn FileSystem> = Arc::new(ResolutionFileSystem);
    let resolution = FileSystemResolution::new(
        filesystem,
        FsPath::parse_literal("bucket/a%252Fb")
            .expect("the provider path should parse"),
        FsUri::parse("mock:///bucket/a%25252Fb")
            .expect("the canonical URI should parse"),
    );

    assert_eq!("bucket/a%252Fb", resolution.path().as_str());
    assert_eq!(
        "/bucket/a%25252Fb",
        resolution.canonical_uri().path().as_encoded(),
    );
    assert_eq!("resolution-test", resolution.file_system().info().id().as_str());

    let cloned = resolution.clone();
    let (filesystem, path, uri) = cloned.into_parts();
    assert!(Arc::ptr_eq(resolution.file_system(), &filesystem));
    assert_eq!(resolution.path(), &path);
    assert_eq!(resolution.canonical_uri(), &uri);
}

struct ResolutionFileSystem;

impl FileSystemProperties for ResolutionFileSystem {
    fn info(&self) -> &FileSystemInfo {
        static INFO: std::sync::OnceLock<FileSystemInfo> = std::sync::OnceLock::new();
        INFO.get_or_init(|| {
            FileSystemInfo::new(
                FileSystemId::new("resolution-test")
                    .expect("the static filesystem ID should parse"),
                "resolution",
                PathSemantics::ObjectKey,
            )
        })
    }

    fn capabilities(&self) -> FileSystemCapabilities {
        FileSystemCapabilities::default()
    }

    fn limits(&self) -> &FileSystemLimits {
        static LIMITS: FileSystemLimits = FileSystemLimits::unknown();
        &LIMITS
    }
}

impl FileSystem for ResolutionFileSystem {
    fn stat(&self, _path: &FsPath) -> FsResult<FileMetadata> {
        Err(FsError::new(
            FsErrorKind::NotFound,
            FsOperation::Stat,
            "the resolution test filesystem has no resources",
        ))
    }
}
