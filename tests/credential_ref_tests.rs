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
        CredentialRef::Provider { id: "vault".to_owned() },
        CredentialRef::Provider { id: "vault".to_owned() },
    );
}

/// Verifies debug output never exposes credential reference payloads.
#[test]
fn test_credential_reference_debug_redacts_payloads() {
    let cases = [
        (
            CredentialRef::DefaultChain,
            "CredentialRef::DefaultChain",
            &[] as &[&str],
        ),
        (
            CredentialRef::Profile {
                name: "production-profile".to_owned(),
            },
            "CredentialRef::Profile(<redacted>)",
            &["production-profile"] as &[&str],
        ),
        (
            CredentialRef::Environment {
                access_key_env: "PRODUCTION_ACCESS_KEY".to_owned(),
                secret_key_env: "PRODUCTION_SECRET_KEY".to_owned(),
            },
            concat!(
                "CredentialRef::Environment { ",
                "access_key_env: <redacted>, ",
                "secret_key_env: <redacted> }",
            ),
            &["PRODUCTION_ACCESS_KEY", "PRODUCTION_SECRET_KEY"] as &[&str],
        ),
        (
            CredentialRef::Provider {
                id: "production-vault".to_owned(),
            },
            "CredentialRef::Provider(<redacted>)",
            &["production-vault"] as &[&str],
        ),
    ];

    for (reference, expected, payloads) in cases {
        let debug = format!("{reference:?}");
        assert_eq!(debug, expected);
        for payload in payloads {
            assert!(!debug.contains(payload));
        }
    }
}
