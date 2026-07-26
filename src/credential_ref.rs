// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Credential references used by filesystem provider configuration.

/// Reference to credentials without storing secret values.
#[derive(Clone, Eq, PartialEq)]
pub enum CredentialRef {
    /// Use the provider's default credential chain.
    DefaultChain,
    /// Use a named credential profile.
    Profile {
        /// Profile name understood by the provider.
        name: String,
    },
    /// Read credentials from named environment variables.
    Environment {
        /// Environment variable containing the access key or username.
        access_key_env: String,
        /// Environment variable containing the secret key or password.
        secret_key_env: String,
    },
    /// Use an external credential provider id.
    Provider {
        /// Provider-specific credential-provider identifier.
        id: String,
    },
}

impl std::fmt::Debug for CredentialRef {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DefaultChain => formatter.write_str("CredentialRef::DefaultChain"),
            Self::Profile { .. } => formatter.write_str("CredentialRef::Profile(<redacted>)"),
            Self::Environment { .. } => formatter.write_str(
                "CredentialRef::Environment { access_key_env: <redacted>, secret_key_env: <redacted> }",
            ),
            Self::Provider { .. } => formatter.write_str("CredentialRef::Provider(<redacted>)"),
        }
    }
}
