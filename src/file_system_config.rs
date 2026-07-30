// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Complete secret-safe provider configuration.

use std::fmt::{
    Debug,
    Display,
    Formatter,
    Result as FmtResult,
};

use qubit_fs::{
    ConnectionUri,
    NonSensitiveMetadata,
};
use qubit_spi::ProviderSelection;

use crate::CredentialRef;

/// Complete configuration passed to a filesystem provider factory.
#[derive(Clone, PartialEq)]
#[must_use]
pub struct FileSystemConfig {
    /// Redacting connection URI that identifies the target filesystem.
    uri: ConnectionUri,
    /// Explicit provider selection, when configuration overrides URI routing.
    selection: Option<ProviderSelection>,
    /// Validated non-sensitive options passed to the provider factory.
    options: NonSensitiveMetadata,
    /// External reference used to obtain credentials, when configured.
    credential: Option<CredentialRef>,
    /// Validated non-sensitive metadata passed to the provider.
    metadata: NonSensitiveMetadata,
}

impl FileSystemConfig {
    /// Creates an empty configuration for `uri`.
    ///
    /// # Parameters
    ///
    /// - `uri`: Redacting connection URI to configure.
    ///
    /// # Returns
    ///
    /// A configuration with no explicit selection, options, credentials, or
    /// metadata.
    #[inline]
    pub fn new(uri: ConnectionUri) -> Self {
        Self {
            uri,
            selection: None,
            options: NonSensitiveMetadata::new(),
            credential: None,
            metadata: NonSensitiveMetadata::new(),
        }
    }
    /// Returns the redacting connection URI.
    ///
    /// # Returns
    ///
    /// The URI owned by this configuration.
    #[inline(always)]
    #[must_use]
    pub const fn uri(&self) -> &ConnectionUri {
        &self.uri
    }
    /// Returns the explicit provider selection, when present.
    ///
    /// # Returns
    ///
    /// `Some` with the configured selection, or `None` when resolution should
    /// derive a selection from the URI.
    #[inline(always)]
    #[must_use]
    pub const fn selection(&self) -> Option<&ProviderSelection> {
        self.selection.as_ref()
    }
    /// Replaces the provider selection.
    ///
    /// # Parameters
    ///
    /// - `selection`: Explicit provider selection to store.
    ///
    /// # Returns
    ///
    /// The updated configuration.
    #[inline(always)]
    pub fn with_selection(mut self, selection: ProviderSelection) -> Self {
        self.selection = Some(selection);
        self
    }
    /// Returns validated non-sensitive factory options.
    ///
    /// # Returns
    ///
    /// The provider factory options.
    #[inline(always)]
    #[must_use]
    pub const fn options(&self) -> &NonSensitiveMetadata {
        &self.options
    }
    /// Replaces validated factory options.
    ///
    /// # Parameters
    ///
    /// - `options`: Validated non-sensitive options to store.
    ///
    /// # Returns
    ///
    /// The updated configuration.
    #[inline(always)]
    pub fn with_options(mut self, options: NonSensitiveMetadata) -> Self {
        self.options = options;
        self
    }
    /// Returns the external credential reference, when configured.
    ///
    /// # Returns
    ///
    /// `Some` with the external credential reference, or `None` when the
    /// configuration does not select an external credential source.
    #[inline(always)]
    #[must_use]
    pub const fn credential(&self) -> Option<&CredentialRef> {
        self.credential.as_ref()
    }
    /// Sets an external credential reference.
    ///
    /// # Parameters
    ///
    /// - `credential`: External credential reference to store.
    ///
    /// # Returns
    ///
    /// The updated configuration.
    #[inline(always)]
    pub fn with_credential(mut self, credential: CredentialRef) -> Self {
        self.credential = Some(credential);
        self
    }
    /// Returns validated non-sensitive provider metadata.
    ///
    /// # Returns
    ///
    /// The provider metadata.
    #[inline(always)]
    #[must_use]
    pub const fn metadata(&self) -> &NonSensitiveMetadata {
        &self.metadata
    }
    /// Replaces validated provider metadata.
    ///
    /// # Parameters
    ///
    /// - `metadata`: Validated non-sensitive metadata to store.
    ///
    /// # Returns
    ///
    /// The updated configuration.
    #[inline(always)]
    pub fn with_metadata(mut self, metadata: NonSensitiveMetadata) -> Self {
        self.metadata = metadata;
        self
    }
}

impl Display for FileSystemConfig {
    /// Formats only safe configuration structure and the redacted URI.
    ///
    /// # Parameters
    ///
    /// - `f`: Destination formatter.
    ///
    /// # Returns
    ///
    /// The formatter result.
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "FileSystemConfig({})", self.uri)
    }
}

impl Debug for FileSystemConfig {
    /// Formats safe configuration structure without metadata values or
    /// reference payloads.
    ///
    /// # Parameters
    ///
    /// - `f`: Destination formatter.
    ///
    /// # Returns
    ///
    /// The formatter result.
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("FileSystemConfig")
            .field("uri", &self.uri)
            .field("selection", &self.selection)
            .field("options", &self.options)
            .field("credential", &self.credential)
            .field("metadata", &self.metadata)
            .finish()
    }
}
