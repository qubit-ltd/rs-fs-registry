// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Complete secret-safe provider configuration.

use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

use qubit_fs::metadata::NonSensitiveMetadata;
use qubit_fs::path::ConnectionUri;
use qubit_spi::ProviderSelection;

use crate::CredentialRef;

/// Complete configuration passed to a filesystem provider factory.
///
/// The [`ConnectionUri`] retains the redaction-policy snapshot captured when
/// it was parsed. Credential validation inspects URI components against that
/// snapshot and the immutable standard sensitivity floor. Username-only
/// userinfo (for example, `s3://reports-user@bucket/key`) is not a secret and
/// may accompany a [`CredentialRef`]. If inspection is invalid or cannot
/// complete within its safety budget, the URI is conservatively treated as
/// containing a secret.
///
/// Options and metadata use [`NonSensitiveMetadata`], whose keys are checked
/// against the standard sensitivity policy when inserted. Values remain
/// unchanged and are available only through explicit reads such as `get`;
/// their presence in this wrapper does not make them safe to log. Providers
/// must sanitize provider-controlled location details before returning a
/// credential-free canonical URI.
///
/// # Examples
///
/// ```
/// use qubit_fs::path::ConnectionUri;
/// use qubit_fs_registry::CredentialRef;
/// use qubit_fs_registry::FileSystemConfig;
/// use qubit_spi::ProviderSelection;
/// let config = FileSystemConfig::new(ConnectionUri::parse("s3://bucket/report.csv")?)
///     .with_selection(ProviderSelection::named("archive")?)
///     .with_credential(CredentialRef::DefaultChain);
/// assert_eq!(config.uri().scheme(), "s3");
/// assert_eq!(config.credential(), Some(&CredentialRef::DefaultChain));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, PartialEq)]
#[must_use]
pub struct FileSystemConfig {
    /// Connection URI carrying the redaction-policy snapshot captured at parse
    /// time; credential validation also applies the standard sensitivity floor.
    uri: ConnectionUri,
    /// Explicit provider selection, when configuration overrides URI routing.
    selection: Option<ProviderSelection>,
    /// Options whose keys passed standard sensitivity validation; stored
    /// values are retained and exposed only by explicit reads.
    options: NonSensitiveMetadata,
    /// External reference used to obtain credentials, when configured.
    credential: Option<CredentialRef>,
    /// Metadata whose keys passed standard sensitivity validation; stored
    /// values are retained and exposed only by explicit reads.
    metadata: NonSensitiveMetadata,
}

impl FileSystemConfig {
    /// Creates an empty configuration for `uri`.
    ///
    /// The URI keeps the policy snapshot captured by its parser. Registry
    /// credential checks combine that snapshot with the standard sensitivity
    /// floor and treat failed or incomplete inspection as secret-bearing.
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
    #[inline]
    #[must_use]
    pub const fn uri(&self) -> &ConnectionUri {
        &self.uri
    }

    /// Returns the explicit provider selection, when present.
    ///
    /// # Returns
    ///
    /// `Some` with the configured selection, or `None` when this configuration
    /// carries no embedded selection. Resolution through
    /// [`crate::FileSystemRegistry::resolve_config`] or its async counterpart,
    /// `AsyncFileSystemRegistry::resolve_config`, derives selection from the
    /// URI. [`crate::FileSystemRegistry::resolve_selected_config`] and
    /// [`crate::FileSystemRegistry::resolve_default_config`] (and their async
    /// counterparts) use the requested or current default selection and reject
    /// a conflicting embedded selection.
    #[inline]
    pub const fn selection(&self) -> Option<&ProviderSelection> {
        self.selection.as_ref()
    }

    /// Returns validated non-sensitive factory options.
    ///
    /// # Returns
    ///
    /// The provider factory options.
    #[inline]
    #[must_use]
    pub const fn options(&self) -> &NonSensitiveMetadata {
        &self.options
    }

    /// Returns the external credential reference, when configured.
    ///
    /// # Returns
    ///
    /// `Some` with the external credential reference, or `None` when the
    /// configuration does not select an external credential source.
    #[inline]
    pub const fn credential(&self) -> Option<&CredentialRef> {
        self.credential.as_ref()
    }

    /// Returns validated non-sensitive provider metadata.
    ///
    /// # Returns
    ///
    /// The provider metadata.
    #[inline]
    #[must_use]
    pub const fn metadata(&self) -> &NonSensitiveMetadata {
        &self.metadata
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
    #[inline]
    pub fn with_selection(mut self, selection: ProviderSelection) -> Self {
        self.selection = Some(selection);
        self
    }

    /// Replaces factory options after their keys have passed sensitivity checks.
    ///
    /// Values are retained unchanged. Reading them explicitly does not make
    /// them safe to log or expose.
    ///
    /// # Parameters
    ///
    /// - `options`: Validated non-sensitive options to store.
    ///
    /// # Returns
    ///
    /// The updated configuration.
    #[inline]
    pub fn with_options(mut self, options: NonSensitiveMetadata) -> Self {
        self.options = options;
        self
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
    #[inline]
    pub fn with_credential(mut self, credential: CredentialRef) -> Self {
        self.credential = Some(credential);
        self
    }

    /// Replaces provider metadata after its keys have passed sensitivity checks.
    ///
    /// Values are retained unchanged. Reading them explicitly does not make
    /// them safe to log or expose.
    ///
    /// # Parameters
    ///
    /// - `metadata`: Validated non-sensitive metadata to store.
    ///
    /// # Returns
    ///
    /// The updated configuration.
    #[inline]
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
    #[inline]
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
