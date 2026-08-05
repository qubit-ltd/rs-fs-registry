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
//! # #[cfg(feature = "async")]
//! use qubit_fs::ConnectionUri;
//! # #[cfg(feature = "async")]
//! use qubit_fs_registry::{AsyncFileSystemRegistry, FileSystemConfig};
//!
//! # #[cfg(feature = "async")]
//! async fn resolve_from_registered_async_providers(
//!     registry: &AsyncFileSystemRegistry,
//! ) -> Result<(), Box<dyn std::error::Error>> {
//!     let config = FileSystemConfig::new(ConnectionUri::parse("file:///tmp/example")?);
//!     let _resolution = registry.resolve_config(config).await?;
//!     Ok(())
//! }
//! ```

#![deny(missing_docs)]

#[cfg(feature = "async")]
mod async_file_system_provider;
#[cfg(feature = "async")]
mod async_file_system_registry;
#[cfg(feature = "async")]
mod async_file_system_resolution;
mod credential_ref;
mod file_system_config;
mod file_system_provider;
mod file_system_registry;
mod file_system_registry_error;
mod file_system_resolution;
mod file_system_spec;
mod internal;

#[cfg(feature = "async")]
pub use async_file_system_provider::AsyncFileSystemProvider;
#[cfg(feature = "async")]
pub use async_file_system_registry::AsyncFileSystemRegistry;
#[cfg(feature = "async")]
pub use async_file_system_resolution::AsyncFileSystemResolution;
pub use credential_ref::CredentialRef;
pub use file_system_config::FileSystemConfig;
pub use file_system_provider::FileSystemProvider;
pub use file_system_registry::FileSystemRegistry;
pub use file_system_registry_error::FileSystemRegistryError;
pub use file_system_registry_error::FileSystemRegistryResult;
pub use file_system_resolution::FileSystemResolution;
pub use file_system_spec::FileSystemSpec;
