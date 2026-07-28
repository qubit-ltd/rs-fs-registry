//! Complete secret-safe provider configuration.

use std::fmt::{Debug, Display, Formatter, Result as FmtResult};

use qubit_fs::{ConnectionUri, NonSensitiveMetadata};
use qubit_spi::ProviderSelection;

use crate::CredentialRef;

/// Complete configuration passed to a filesystem provider factory.
#[derive(Clone, PartialEq)]
pub struct FileSystemConfig {
    uri: ConnectionUri,
    selection: Option<ProviderSelection>,
    options: NonSensitiveMetadata,
    credential: Option<CredentialRef>,
    metadata: NonSensitiveMetadata,
}

impl FileSystemConfig {
    /// Creates an empty configuration for `uri`.
    #[must_use]
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
    #[must_use]
    pub const fn uri(&self) -> &ConnectionUri {
        &self.uri
    }
    /// Returns the explicit provider selection, when present.
    #[must_use]
    pub const fn selection(&self) -> Option<&ProviderSelection> {
        self.selection.as_ref()
    }
    /// Returns validated non-sensitive factory options.
    #[must_use]
    pub const fn options(&self) -> &NonSensitiveMetadata {
        &self.options
    }
    /// Returns the external credential reference, when configured.
    #[must_use]
    pub const fn credential(&self) -> Option<&CredentialRef> {
        self.credential.as_ref()
    }
    /// Returns validated non-sensitive provider metadata.
    #[must_use]
    pub const fn metadata(&self) -> &NonSensitiveMetadata {
        &self.metadata
    }
    /// Replaces the provider selection.
    #[must_use]
    pub fn with_selection(mut self, selection: ProviderSelection) -> Self {
        self.selection = Some(selection);
        self
    }
    /// Replaces validated factory options.
    #[must_use]
    pub fn with_options(mut self, options: NonSensitiveMetadata) -> Self {
        self.options = options;
        self
    }
    /// Sets an external credential reference.
    #[must_use]
    pub fn with_credential(mut self, credential: CredentialRef) -> Self {
        self.credential = Some(credential);
        self
    }
    /// Replaces validated provider metadata.
    #[must_use]
    pub fn with_metadata(mut self, metadata: NonSensitiveMetadata) -> Self {
        self.metadata = metadata;
        self
    }
}

impl Display for FileSystemConfig {
    /// Formats only safe configuration structure and the redacted URI.
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "FileSystemConfig({})", self.uri)
    }
}
impl Debug for FileSystemConfig {
    /// Formats safe configuration structure without metadata values or reference payloads.
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
