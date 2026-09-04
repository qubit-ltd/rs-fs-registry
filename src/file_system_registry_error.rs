// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Error model for filesystem provider registry operations.

use std::error::Error;
use std::fmt;

use qubit_fs::FsError;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_redact::DebugDisplay;
use qubit_redact::RedactionTextOutput;
use qubit_redact::Redactor;
use qubit_spi::ProviderSelection;
use qubit_spi::error::ProviderCreationError;
use qubit_spi::error::ProviderResolutionError;
use qubit_spi::error::ProviderSelectionBuildError;
use qubit_spi::error::RegistrationError;

/// Result returned by filesystem registry operations.
///
/// # Type Parameters
///
/// - `T`: Successful registry operation output.
pub type FileSystemRegistryResult<T> = Result<T, FileSystemRegistryError>;

/// Error returned by filesystem-provider registration, selection, and creation.
#[non_exhaustive]
pub enum FileSystemRegistryError {
    /// Configuration violates a registry-level safety invariant.
    InvalidConfiguration {
        /// Safe description that never contains connection or credential data.
        message: &'static str,
    },
    /// A provider descriptor could not be registered.
    Registration(
        /// Typed SPI registration failure.
        RegistrationError,
    ),
    /// A provider selection could not be constructed from configuration.
    Selection(
        /// Typed SPI selection-construction failure.
        ProviderSelectionBuildError,
    ),
    /// A caller-supplied selection conflicts with the configuration selection.
    SelectionConflict {
        /// Selection requested by the caller or registry default.
        requested: ProviderSelection,
        /// Different selection embedded in the configuration.
        configured: ProviderSelection,
    },
    /// A selection did not resolve to registered providers.
    Resolution(
        /// Typed SPI provider-resolution failure.
        ProviderResolutionError,
    ),
    /// Provider creation terminated without producing a filesystem.
    Creation(
        /// Typed aggregate preserving provider creation attempts and failures.
        ProviderCreationError<FsError>,
    ),
}

impl FileSystemRegistryError {
    /// Builds one bounded diagnostic event with an explicit redactor snapshot.
    fn redacted_output(&self, redactor: &Redactor) -> RedactionTextOutput {
        match self {
            Self::InvalidConfiguration { message } => redactor
                .text_composer()
                .literal("invalid filesystem configuration: ")
                .field("password", message),
            Self::Registration(error) => redactor
                .text_composer()
                .literal("provider registration failed: selector=")
                .field("selector", error.selector())
                .literal(", existing_provider=")
                .field("provider_id", error.existing_provider())
                .literal(", provider=")
                .field("provider_id", error.provider()),
            Self::Selection(_error) => redactor.text_composer().literal("provider selection is invalid"),
            Self::SelectionConflict { requested, configured } => redactor
                .text_composer()
                .literal("configured provider selection conflicts with requested selection: requested=")
                .field("selection", &DebugDisplay::new(requested))
                .literal(", configured=")
                .field("selection", &DebugDisplay::new(configured)),
            Self::Resolution(_error) => redactor.text_composer().literal("provider resolution failed"),
            Self::Creation(_error) => redactor.text_composer().literal("filesystem provider creation failed"),
        }
        .finish()
    }
}

impl fmt::Display for FileSystemRegistryError {
    /// Formats the registry failure with its preserved SPI context.
    ///
    /// # Parameters
    ///
    /// - `formatter`: Destination formatter.
    ///
    /// # Returns
    ///
    /// The formatter result.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let redactor = Redactor::standard();
        let output = self.redacted_output(&redactor);
        formatter.write_str(output.text().as_str())
    }
}

impl fmt::Debug for FileSystemRegistryError {
    /// Formats the policy-redacted diagnostic with a type label.
    ///
    /// # Parameters
    ///
    /// - `formatter`: Destination formatter.
    ///
    /// # Returns
    ///
    /// The formatter result.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "FileSystemRegistryError({self})")
    }
}

impl Error for FileSystemRegistryError {
    /// Returns the underlying SPI error when one exists.
    ///
    /// # Returns
    ///
    /// `Some` with the underlying registration, selection, resolution, or
    /// creation error; `None` for configuration and selection-conflict errors.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidConfiguration { .. } => None,
            Self::Registration(error) => Some(error),
            Self::Selection(error) => Some(error),
            Self::SelectionConflict { .. } => None,
            Self::Resolution(error) => Some(error),
            Self::Creation(error) => Some(error),
        }
    }
}

impl From<RegistrationError> for FileSystemRegistryError {
    /// Wraps an SPI registration failure without losing its type.
    ///
    /// # Parameters
    ///
    /// - `error`: SPI registration error to wrap.
    ///
    /// # Returns
    ///
    /// The registry registration error.
    #[inline(always)]
    fn from(error: RegistrationError) -> Self {
        Self::Registration(error)
    }
}

impl From<ProviderSelectionBuildError> for FileSystemRegistryError {
    /// Wraps an SPI selection-construction failure without losing its type.
    ///
    /// # Parameters
    ///
    /// - `error`: SPI selection-construction error to wrap.
    ///
    /// # Returns
    ///
    /// The registry selection error.
    #[inline(always)]
    fn from(error: ProviderSelectionBuildError) -> Self {
        Self::Selection(error)
    }
}

impl From<ProviderResolutionError> for FileSystemRegistryError {
    /// Wraps an SPI resolution failure without losing its type.
    ///
    /// # Parameters
    ///
    /// - `error`: SPI provider-resolution error to wrap.
    ///
    /// # Returns
    ///
    /// The registry resolution error.
    #[inline(always)]
    fn from(error: ProviderResolutionError) -> Self {
        Self::Resolution(error)
    }
}

impl From<ProviderCreationError<FsError>> for FileSystemRegistryError {
    /// Wraps the typed provider-creation aggregate without losing leaf errors.
    ///
    /// # Parameters
    ///
    /// - `error`: Typed provider-creation aggregate to wrap.
    ///
    /// # Returns
    ///
    /// The registry creation error.
    #[inline(always)]
    fn from(error: ProviderCreationError<FsError>) -> Self {
        Self::Creation(error)
    }
}

impl From<FileSystemRegistryError> for FsError {
    /// Converts a registry failure into a filesystem-operation error.
    ///
    /// The returned error retains the typed registry error as its source. A
    /// creation failure uses the decisive provider error's kind and provider
    /// ID; other registry failures use the closest provider-neutral category.
    ///
    /// # Parameters
    ///
    /// - `error`: Registry error to convert.
    ///
    /// # Returns
    ///
    /// A filesystem provider-operation error retaining `error` as its source.
    fn from(error: FileSystemRegistryError) -> Self {
        let (kind, message, provider) = match &error {
            FileSystemRegistryError::InvalidConfiguration { .. } => {
                (FsErrorKind::InvalidOptions, "filesystem configuration is invalid", None)
            }
            FileSystemRegistryError::Registration(_) => {
                (FsErrorKind::Conflict, "filesystem provider registration failed", None)
            }
            FileSystemRegistryError::Selection(_) => (
                FsErrorKind::InvalidUri,
                "filesystem provider selection is invalid",
                None,
            ),
            FileSystemRegistryError::SelectionConflict { .. } => (
                FsErrorKind::InvalidOptions,
                "filesystem provider selections conflict",
                None,
            ),
            FileSystemRegistryError::Resolution(_) => (
                FsErrorKind::ProviderUnavailable,
                "filesystem provider selection could not be resolved",
                None,
            ),
            FileSystemRegistryError::Creation(creation) => {
                let attempt = creation.decisive_attempt();
                (
                    attempt.failure().error().kind(),
                    "filesystem provider creation failed",
                    Some(attempt.provider_id().as_str().to_owned()),
                )
            }
        };
        let error = FsError::with_source(kind, FsOperation::Provider, message, error);
        match provider {
            Some(provider) => error.with_provider(provider),
            None => error,
        }
    }
}

#[cfg(test)]
mod tests {
    use qubit_redact::RedactionPolicy;
    use qubit_redact::Redactor;
    use qubit_spi::ProviderSelection;

    use super::FileSystemRegistryError;

    /// One error diagnostic must share its output budget across all fields.
    #[test]
    fn redacted_output_uses_one_event_budget() {
        let policy = RedactionPolicy::builder()
            .limits(|limits| {
                limits.max_output_bytes(24);
            })
            .expect("limits should be valid")
            .build()
            .expect("policy should build");
        let error = FileSystemRegistryError::SelectionConflict {
            requested: ProviderSelection::named("requested-provider").expect("valid selector"),
            configured: ProviderSelection::named("configured-provider").expect("valid selector"),
        };

        let output = error.redacted_output(&Redactor::new(policy));

        assert!(output.text().as_str().len() <= 24);
    }
}
