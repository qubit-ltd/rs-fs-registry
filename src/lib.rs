// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provider registration and registry integration for [`qubit_fs`].
//!
//! # Quick start
//!
//! ```no_run
//! use qubit_fs::ConnectionUri;
//! use qubit_fs_registry::{
//!     FileSystemConfig,
//!     FileSystemRegistry,
//! };
//!
//! fn resolve_from_registered_providers(
//!     registry: &FileSystemRegistry,
//! ) -> Result<(), Box<dyn std::error::Error>> {
//!     let config = FileSystemConfig::new(ConnectionUri::parse("file:///tmp/example")?);
//!     let _resolution = registry.resolve_config(&config)?;
//!     Ok(())
//! }
//! ```
//!
//! # Async quick start
//!
//! ```no_run
//! use qubit_fs::ConnectionUri;
//! use qubit_fs_registry::{AsyncFileSystemRegistry, FileSystemConfig};
//!
//! async fn resolve_from_registered_async_providers(
//!     registry: &AsyncFileSystemRegistry,
//! ) -> Result<(), Box<dyn std::error::Error>> {
//!     let config = FileSystemConfig::new(ConnectionUri::parse("file:///tmp/example")?);
//!     let _resolution = registry.resolve_config(config).await?;
//!     Ok(())
//! }
//! ```

#![deny(missing_docs)]

mod async_file_system_provider;
mod async_file_system_registry;
mod async_file_system_resolution;
mod credential_ref;
mod file_system_config;
mod file_system_provider;
mod file_system_registry;
mod file_system_registry_error;
mod file_system_resolution;
mod file_system_spec;
mod internal;

pub use async_file_system_provider::AsyncFileSystemProvider;
pub use async_file_system_registry::AsyncFileSystemRegistry;
pub use async_file_system_resolution::AsyncFileSystemResolution;
pub use credential_ref::CredentialRef;
pub use file_system_config::FileSystemConfig;
pub use file_system_provider::FileSystemProvider;
pub use file_system_registry::FileSystemRegistry;
pub use file_system_registry_error::FileSystemRegistryError;
pub use file_system_registry_error::FileSystemRegistryResult;
pub use file_system_resolution::FileSystemResolution;
pub use file_system_spec::FileSystemSpec;
