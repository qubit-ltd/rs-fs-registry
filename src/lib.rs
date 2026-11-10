// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provider registration and registry integration for [`qubit_fs`].
//!
//! # Register providers first
//!
//! [`FileSystemRegistry::default()`] creates an empty registry. Register a
//! provider with [`FileSystemRegistry::register`] or
//! [`FileSystemRegistry::register_shared`] during startup before resolving
//! connections. URI-based resolution selects by scheme, so the provider ID
//! or one of its aliases must match that selection.
//!
//! The quick-start functions below accept registries whose providers have
//! already been registered. See the user guides below for complete provider
//! registration examples.
//!
//! # Features
//!
//! | Feature | Available integration |
//! | --- | --- |
//! | Default (empty set) | Synchronous registry with explicit registration. |
//! | `async` | Asynchronous registry with explicit registration; no implicit blocking adapter. |
//! | `inventory` | Synchronous `from_inventory()` discovery of linked provider submissions. |
//! | `async` + `inventory` | Asynchronous discovery is also available. |
//!
//! Inventory discovery still requires linking the provider crate that submits
//! the factories.
//!
//! # Quick start
//!
//! ```no_run
//! use qubit_fs::path::ConnectionUri;
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
//! use qubit_fs::path::ConnectionUri;
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
//!
//! # Learn more
//!
//! - [English user guide](https://github.com/qubit-ltd/rs-fs-registry/blob/main/doc/user_guide.md)
//!   / [中文用户指南](https://github.com/qubit-ltd/rs-fs-registry/blob/main/doc/user_guide.zh_CN.md)
//! - [English design](https://github.com/qubit-ltd/rs-fs-registry/blob/main/doc/file_system_registry_design.md)
//!   / [中文设计](https://github.com/qubit-ltd/rs-fs-registry/blob/main/doc/file_system_registry_design.zh_CN.md)
//! - [English migration guide](https://github.com/qubit-ltd/rs-fs-registry/blob/main/doc/registry_contract_migration.md)
//!   / [中文迁移说明](https://github.com/qubit-ltd/rs-fs-registry/blob/main/doc/registry_contract_migration.zh_CN.md)

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
#[cfg(feature = "inventory")]
mod inventory;

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
#[cfg(all(feature = "inventory", feature = "async"))]
pub use inventory::async_file_system_providers;
#[cfg(feature = "inventory")]
pub use inventory::sync_file_system_providers;
