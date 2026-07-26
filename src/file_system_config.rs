// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Complete provider configuration used by filesystem registries.

use std::fmt::{
    Debug,
    Formatter,
    Result as FmtResult,
};

use qubit_spi::ProviderSelection;

use crate::CredentialRef;
use qubit_fs::{
    FsUri,
    UserMetadata,
};

/// Complete non-secret configuration passed through registry and provider SPI.
#[derive(Clone, Eq, PartialEq)]
pub struct FileSystemConfig {
    /// Resource URI used for provider selection and path decoding.
    uri: FsUri,
    /// Optional provider selection overriding URI-scheme selection.
    selection: Option<ProviderSelection>,
    /// Validated non-sensitive options passed to provider creation.
    options: UserMetadata,
    /// Optional reference identifying an external credential source.
    credentials: Option<CredentialRef>,
}

impl FileSystemConfig {
    /// Creates provider configuration from a resource URI.
    ///
    /// # Parameters
    ///
    /// * `uri` - Secret-free resource URI used for provider resolution.
    ///
    /// # Returns
    ///
    /// A configuration with no explicit selection, options, or credentials.
    #[inline(always)]
    #[must_use]
    pub fn new(uri: FsUri) -> Self {
        Self {
            uri,
            selection: None,
            options: UserMetadata::new(),
            credentials: None,
        }
    }

    /// Sets an explicit provider selection and fallback policy.
    ///
    /// # Parameters
    ///
    /// * `selection` - Provider target and creation fallback policy.
    ///
    /// # Returns
    ///
    /// The updated configuration.
    #[inline(always)]
    #[must_use]
    pub fn with_selection(mut self, selection: ProviderSelection) -> Self {
        self.selection = Some(selection);
        self
    }

    /// Sets validated non-sensitive provider options.
    ///
    /// [`UserMetadata`] rejects sensitive option keys when it is built.
    /// Secrets must be referenced through [`CredentialRef`].
    ///
    /// # Parameters
    ///
    /// * `options` - Validated non-sensitive provider options.
    ///
    /// # Returns
    ///
    /// The updated configuration.
    #[inline(always)]
    #[must_use]
    pub fn with_options(mut self, options: UserMetadata) -> Self {
        self.options = options;
        self
    }

    /// Sets a credential source reference without embedding secret material.
    ///
    /// # Parameters
    ///
    /// * `credentials` - Reference identifying an external credential source.
    ///
    /// # Returns
    ///
    /// The updated configuration.
    #[inline(always)]
    #[must_use]
    pub fn with_credentials(mut self, credentials: CredentialRef) -> Self {
        self.credentials = Some(credentials);
        self
    }

    /// Returns the resource URI used for provider resolution.
    ///
    /// # Returns
    ///
    /// The configured secret-free resource URI.
    #[inline(always)]
    #[must_use]
    pub const fn uri(&self) -> &FsUri {
        &self.uri
    }

    /// Returns the optional explicit provider selection.
    ///
    /// # Returns
    ///
    /// `Some` when provider selection is explicit; otherwise `None`.
    #[inline(always)]
    #[must_use]
    pub const fn selection(&self) -> Option<&ProviderSelection> {
        self.selection.as_ref()
    }

    /// Returns validated non-sensitive provider options.
    ///
    /// # Returns
    ///
    /// The provider options, which may be empty.
    #[inline(always)]
    #[must_use]
    pub const fn options(&self) -> &UserMetadata {
        &self.options
    }

    /// Returns the optional credential source reference.
    ///
    /// # Returns
    ///
    /// `Some` when an external credential source is configured; otherwise
    /// `None`.
    #[inline(always)]
    #[must_use]
    pub const fn credentials(&self) -> Option<&CredentialRef> {
        self.credentials.as_ref()
    }
}

impl Debug for FileSystemConfig {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        let option_keys: Vec<_> =
            self.options.iter().map(|(key, _)| key).collect();
        formatter
            .debug_struct("FileSystemConfig")
            .field("uri", &self.uri)
            .field("selection", &self.selection)
            .field("option_keys", &option_keys)
            .field(
                "credentials",
                &self.credentials.as_ref().map(|_| "<credential-ref>"),
            )
            .finish()
    }
}
