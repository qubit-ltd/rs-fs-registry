// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs_registry::CredentialRef;

/// Verifies credential references retain only selection data.
#[test]
fn test_credential_reference_variants_are_comparable() {
    assert_eq!(CredentialRef::DefaultChain, CredentialRef::DefaultChain);
    assert_eq!(
        CredentialRef::Profile {
            name: "production".to_owned(),
        },
        CredentialRef::Profile {
            name: "production".to_owned(),
        },
    );
    assert_eq!(
        CredentialRef::Environment {
            access_key_env: "ACCESS_KEY".to_owned(),
            secret_key_env: "SECRET_KEY".to_owned(),
        },
        CredentialRef::Environment {
            access_key_env: "ACCESS_KEY".to_owned(),
            secret_key_env: "SECRET_KEY".to_owned(),
        },
    );
    assert_eq!(
        CredentialRef::Provider {
            id: "vault".to_owned(),
        },
        CredentialRef::Provider {
            id: "vault".to_owned(),
        },
    );
}

/// Verifies debug output never exposes credential reference payloads.
#[test]
fn test_credential_reference_debug_redacts_payloads() {
    let reference = CredentialRef::Environment {
        access_key_env: "PRODUCTION_ACCESS_KEY".to_owned(),
        secret_key_env: "PRODUCTION_SECRET_KEY".to_owned(),
    };

    let debug = format!("{reference:?}");

    assert!(debug.contains("Environment"));
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("PRODUCTION_ACCESS_KEY"));
    assert!(!debug.contains("PRODUCTION_SECRET_KEY"));
}
