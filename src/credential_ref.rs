// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Credential references used by filesystem provider configuration.

/// Reference to credentials without storing secret values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CredentialRef {
    /// Use the provider's default credential chain.
    DefaultChain,
    /// Use a named credential profile.
    Profile(String),
    /// Read credentials from named environment variables.
    Environment {
        /// Environment variable containing the access key or username.
        access_key: String,
        /// Environment variable containing the secret key or password.
        secret_key: String,
    },
    /// Use an external credential provider id.
    Provider(String),
}
